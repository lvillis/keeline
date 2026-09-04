use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::{Context, Result, ensure};
use toml_edit::{DocumentMut, value};

use super::PlannedToolUpdate;
use crate::domain::ImageCatalog;
use crate::file_batch::replace_files;
use crate::render::{self, SyncSummary};

static NEXT_STAGING_ID: AtomicU64 = AtomicU64::new(0);

pub(super) fn apply_updates(
    catalog: &ImageCatalog,
    updates: &[PlannedToolUpdate],
) -> Result<SyncSummary> {
    let updated_definitions = prepare_definition_updates(updates)?;
    let staging = StagingDirectory::new()?;
    stage_catalog(catalog, &updated_definitions, staging.path())?;

    let staged_catalog = ImageCatalog::discover(staging.path())?;
    staged_catalog.validate()?;

    let mut replacements = updated_definitions;
    let mut summary = SyncSummary::default();
    for target in &staged_catalog.targets {
        let relative = target
            .dockerfile
            .strip_prefix(&staged_catalog.root)
            .with_context(|| {
                format!(
                    "staged dockerfile escaped the image catalog: {}",
                    target.dockerfile.display()
                )
            })?;
        let destination = catalog.root.join(relative);
        let rendered = render::render(target)?;

        let changed = match fs::read_to_string(&destination) {
            Ok(existing) if existing == rendered => {
                summary.unchanged += 1;
                false
            }
            Ok(_) => {
                summary.updated += 1;
                true
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                summary.generated += 1;
                true
            }
            Err(error) => {
                return Err(error).with_context(|| {
                    format!("failed to read generated file {}", destination.display())
                });
            }
        };
        if changed {
            ensure!(
                replacements.insert(destination.clone(), rendered).is_none(),
                "multiple staged targets resolve to {}",
                destination.display()
            );
        }
    }

    replace_files(replacements)?;
    Ok(summary)
}

fn prepare_definition_updates(updates: &[PlannedToolUpdate]) -> Result<BTreeMap<PathBuf, String>> {
    let files = updates
        .iter()
        .flat_map(|update| update.definition_files.iter().cloned())
        .collect::<BTreeSet<_>>();
    let mut prepared = BTreeMap::new();

    for file in files {
        let mut contents = fs::read_to_string(&file)
            .with_context(|| format!("failed to read {}", file.display()))?;

        for update in updates
            .iter()
            .filter(|update| update.definition_files.contains(&file))
        {
            contents = update_tool_block(&contents, update)?;
        }

        prepared.insert(file, contents);
    }

    Ok(prepared)
}

fn stage_catalog(
    catalog: &ImageCatalog,
    updated_definitions: &BTreeMap<PathBuf, String>,
    staging_root: &Path,
) -> Result<()> {
    let definitions = catalog
        .targets
        .iter()
        .map(|target| target.definition_file.clone())
        .collect::<BTreeSet<_>>();

    for source in definitions {
        let relative = source.strip_prefix(&catalog.root).with_context(|| {
            format!(
                "image definition escaped the catalog root: {}",
                source.display()
            )
        })?;
        let destination = staging_root.join(relative);
        let parent = destination
            .parent()
            .context("staged image definition has no parent directory")?;
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
        let contents = match updated_definitions.get(&source) {
            Some(contents) => contents.clone(),
            None => fs::read_to_string(&source)
                .with_context(|| format!("failed to read {}", source.display()))?,
        };
        fs::write(&destination, contents)
            .with_context(|| format!("failed to stage {}", source.display()))?;
    }

    Ok(())
}

fn update_tool_block(contents: &str, update: &PlannedToolUpdate) -> Result<String> {
    let mut document = contents
        .parse::<DocumentMut>()
        .context("failed to parse image definition as editable TOML")?;
    let tool = document
        .get_mut("tools")
        .and_then(|tools| tools.get_mut(&update.name))
        .with_context(|| format!("tool block `[tools.{}]` was not found", update.name))?;
    let release = tool
        .get("release")
        .and_then(|item| item.as_str())
        .with_context(|| format!("tool `{}` block is missing string `release`", update.name))?;
    let image = tool
        .get("image")
        .and_then(|item| item.as_str())
        .with_context(|| format!("tool `{}` block is missing string `image`", update.name))?;

    ensure!(
        release == update.old_release,
        "tool `{}` block does not contain expected release `{}`",
        update.name,
        update.old_release
    );
    ensure!(
        image == update.old_image,
        "tool `{}` block does not contain expected image `{}`",
        update.name,
        update.old_image
    );

    tool["release"] = value(&update.new_release);
    tool["image"] = value(&update.new_image);

    Ok(document.to_string())
}

struct StagingDirectory {
    path: PathBuf,
}

impl StagingDirectory {
    fn new() -> Result<Self> {
        for _ in 0..100 {
            let id = NEXT_STAGING_ID.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir()
                .join(format!("keeline-tool-update-{}-{id}", std::process::id()));
            match fs::create_dir(&path) {
                Ok(()) => return Ok(Self { path }),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => {
                    return Err(error)
                        .with_context(|| format!("failed to create {}", path.display()));
                }
            }
        }

        anyhow::bail!("failed to allocate a unique tool update staging directory")
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for StagingDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{PlannedToolUpdate, update_tool_block};
    use crate::domain::ImageCatalog;
    use crate::render;

    #[test]
    fn updates_only_the_selected_tool_block() {
        let contents = r#"[tools.tino]
release = "0.1.26"
image = "ghcr.io/lvillis/tino:0.1.26@sha256:old"

[tools.salus]
release = "0.1.8"
image = "ghcr.io/lvillis/salus:0.1.8@sha256:keep"
"#;
        let update = PlannedToolUpdate {
            name: "tino".to_string(),
            old_release: "0.1.26".to_string(),
            new_release: "0.1.27".to_string(),
            old_image: "ghcr.io/lvillis/tino:0.1.26@sha256:old".to_string(),
            new_image: "ghcr.io/lvillis/tino:0.1.27@sha256:new".to_string(),
            definition_files: Default::default(),
        };

        let updated = update_tool_block(contents, &update).unwrap();

        assert!(updated.contains("release = \"0.1.27\""));
        assert!(updated.contains("image = \"ghcr.io/lvillis/tino:0.1.27@sha256:new\""));
        assert!(updated.contains("image = \"ghcr.io/lvillis/salus:0.1.8@sha256:keep\""));
    }

    #[test]
    fn validates_and_commits_metadata_with_generated_dockerfiles() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("keeline-tool-update-test-{unique}"));
        let image_dir = root.join("scratch/1");
        fs::create_dir_all(&image_dir).unwrap();
        let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("images/scratch/1");
        fs::copy(fixture.join("image.toml"), image_dir.join("image.toml")).unwrap();
        fs::copy(fixture.join("Dockerfile"), image_dir.join("Dockerfile")).unwrap();

        let catalog = ImageCatalog::discover(&root).unwrap();
        catalog.validate().unwrap();
        let old_image = catalog
            .target("scratch-1")
            .unwrap()
            .tool("tino")
            .unwrap()
            .image
            .clone()
            .unwrap();
        let update = PlannedToolUpdate {
            name: "tino".to_string(),
            old_release: "0.1.33".to_string(),
            new_release: "0.1.34".to_string(),
            old_image,
            new_image: format!("ghcr.io/lvillis/tino:0.1.34@sha256:{}", "a".repeat(64)),
            definition_files: BTreeSet::from([image_dir.join("image.toml")]),
        };

        let summary = super::apply_updates(&catalog, &[update]).unwrap();

        assert_eq!(summary.updated, 1);
        let updated = ImageCatalog::discover(&root).unwrap();
        updated.validate().unwrap();
        render::check_catalog(&updated).unwrap();
        assert_eq!(
            updated
                .target("scratch-1")
                .unwrap()
                .tool("tino")
                .unwrap()
                .release,
            "0.1.34"
        );
        assert!(
            fs::read_to_string(image_dir.join("Dockerfile"))
                .unwrap()
                .contains("ghcr.io/lvillis/tino:0.1.34@sha256:")
        );

        fs::remove_dir_all(root).unwrap();
    }
}
