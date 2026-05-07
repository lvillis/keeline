use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::ops::Range;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, Result, bail, ensure};
use semver::Version;
use serde::{Deserialize, Serialize};

use crate::cli::{ToolArgs, ToolCommand, ToolListArgs, ToolOutdatedArgs, ToolUpdateArgs};
use crate::domain::{ImageCatalog, ToolRole};
use crate::render;

const OCI_ACCEPT: &str = concat!(
    "application/vnd.oci.image.index.v1+json, ",
    "application/vnd.docker.distribution.manifest.list.v2+json, ",
    "application/vnd.oci.image.manifest.v1+json, ",
    "application/vnd.docker.distribution.manifest.v2+json"
);

pub fn run(catalog: &ImageCatalog, args: &ToolArgs) -> Result<()> {
    catalog.validate()?;

    match &args.command {
        ToolCommand::List(args) => list(catalog, args),
        ToolCommand::Outdated(args) => outdated(catalog, args),
        ToolCommand::Update(args) => update(catalog, args),
    }
}

fn list(catalog: &ImageCatalog, args: &ToolListArgs) -> Result<()> {
    let declarations = tool_declarations(catalog)?;

    if args.json {
        println!("{}", serde_json::to_string_pretty(&declarations)?);
        return Ok(());
    }

    let rows = declarations
        .iter()
        .map(|tool| {
            vec![
                tool.name.clone(),
                tool.role.as_str().to_string(),
                tool.release.clone(),
                tool.image_tag_reference()
                    .unwrap_or_else(|| "-".to_string()),
                tool.image_ref
                    .as_ref()
                    .map(|image| short_digest(&image.digest))
                    .unwrap_or_else(|| "-".to_string()),
                tool.definition_files.len().to_string(),
                tool.target_ids.len().to_string(),
            ]
        })
        .collect::<Vec<_>>();

    print_table(
        &[
            "TOOL", "ROLE", "RELEASE", "IMAGE", "DIGEST", "DEFS", "TARGETS",
        ],
        &rows,
    );

    Ok(())
}

fn outdated(catalog: &ImageCatalog, args: &ToolOutdatedArgs) -> Result<()> {
    let declarations = selected_declarations(catalog, &args.names)?;
    let client = RegistryClient::new();
    let checks = check_declarations(&client, &declarations, args.allow_major)?;

    if args.json {
        println!("{}", serde_json::to_string_pretty(&checks)?);
    } else {
        print_checks(&checks);
    }

    if args.check && checks.iter().any(ToolCheck::has_newer_version) {
        bail!("tool updates are available");
    }

    Ok(())
}

fn update(catalog: &ImageCatalog, args: &ToolUpdateArgs) -> Result<()> {
    let declarations = selected_declarations(catalog, &args.names)?;
    let client = RegistryClient::new();
    let checks = check_declarations(&client, &declarations, args.allow_major)?;
    let updates = checks
        .iter()
        .filter_map(PlannedToolUpdate::from_check)
        .collect::<Vec<_>>();

    if args.dry_run {
        print_checks(&checks);
        return Ok(());
    }

    if updates.is_empty() {
        print_checks(&checks);
        if checks.iter().any(ToolCheck::has_newer_version) {
            println!("no tool updates applied; use --allow-major to include major updates");
        } else {
            println!("all tools are current");
        }
        return Ok(());
    }

    apply_updates(&updates)?;

    let refreshed = ImageCatalog::discover(&catalog.root)?;
    refreshed.validate()?;
    let summary = render::sync_catalog(&refreshed)?;
    render::check_catalog(&refreshed)?;

    println!(
        "updated {} tool declaration{} across {} image definition file{}; {}",
        updates.len(),
        plural(updates.len()),
        updated_file_count(&updates),
        plural(updated_file_count(&updates)),
        render_summary(summary)
    );

    Ok(())
}

#[derive(Debug, Clone, Serialize)]
struct ToolDeclaration {
    name: String,
    role: ToolRole,
    release: String,
    image: Option<String>,
    source_path: String,
    target_path: String,
    tag_suffix: Option<String>,
    image_ref: Option<ImageReference>,
    definition_files: BTreeSet<PathBuf>,
    target_ids: BTreeSet<String>,
}

impl ToolDeclaration {
    fn image_tag_reference(&self) -> Option<String> {
        self.image_ref
            .as_ref()
            .map(ImageReference::tag_reference)
            .or_else(|| self.image.clone())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct ToolKey {
    name: String,
    role: String,
    release: String,
    image: Option<String>,
    source_path: String,
    target_path: String,
}

#[derive(Debug, Clone, Serialize)]
struct ImageReference {
    registry: String,
    repository: String,
    tag: String,
    digest: String,
}

impl ImageReference {
    fn tag_reference(&self) -> String {
        format!("{}/{}:{}", self.registry, self.repository, self.tag)
    }

    fn with_tag_and_digest(&self, tag: &str, digest: &str) -> String {
        format!("{}/{}:{tag}@{digest}", self.registry, self.repository)
    }
}

#[derive(Debug, Clone, Serialize)]
struct ToolCheck {
    name: String,
    status: ToolStatus,
    current_release: String,
    latest_release: String,
    current_image: String,
    latest_image: String,
    current_digest: String,
    latest_digest: String,
    update_release: Option<String>,
    update_image: Option<String>,
    definition_files: BTreeSet<PathBuf>,
}

impl ToolCheck {
    fn has_newer_version(&self) -> bool {
        self.status != ToolStatus::Current
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
enum ToolStatus {
    Current,
    UpdateAvailable,
    MajorSkipped,
}

impl ToolStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::Current => "current",
            Self::UpdateAvailable => "update-available",
            Self::MajorSkipped => "major-skipped",
        }
    }
}

#[derive(Debug, Clone)]
struct PlannedToolUpdate {
    name: String,
    old_release: String,
    new_release: String,
    old_image: String,
    new_image: String,
    definition_files: BTreeSet<PathBuf>,
}

impl PlannedToolUpdate {
    fn from_check(check: &ToolCheck) -> Option<Self> {
        Some(Self {
            name: check.name.clone(),
            old_release: check.current_release.clone(),
            new_release: check.update_release.clone()?,
            old_image: check.current_image.clone(),
            new_image: check.update_image.clone()?,
            definition_files: check.definition_files.clone(),
        })
    }
}

fn tool_declarations(catalog: &ImageCatalog) -> Result<Vec<ToolDeclaration>> {
    let mut declarations = BTreeMap::<ToolKey, ToolDeclaration>::new();

    for target in &catalog.targets {
        for tool in &target.tools {
            let image_ref = tool
                .image
                .as_deref()
                .map(parse_pinned_image_reference)
                .transpose()
                .with_context(|| {
                    format!(
                        "image `{}` tool `{}` declares an unsupported image reference",
                        target.id, tool.name
                    )
                })?;
            let tag_suffix = image_ref
                .as_ref()
                .and_then(|image| derive_tag_suffix(&tool.release, &image.tag).ok());
            let key = ToolKey {
                name: tool.name.clone(),
                role: tool.role.as_str().to_string(),
                release: tool.release.clone(),
                image: tool.image.clone(),
                source_path: tool.source_path.clone(),
                target_path: tool.target_path.clone(),
            };

            declarations
                .entry(key)
                .and_modify(|declaration| {
                    declaration
                        .definition_files
                        .insert(target.definition_file.clone());
                    declaration.target_ids.insert(target.id.clone());
                })
                .or_insert_with(|| ToolDeclaration {
                    name: tool.name.clone(),
                    role: tool.role,
                    release: tool.release.clone(),
                    image: tool.image.clone(),
                    source_path: tool.source_path.clone(),
                    target_path: tool.target_path.clone(),
                    tag_suffix,
                    image_ref,
                    definition_files: BTreeSet::from([target.definition_file.clone()]),
                    target_ids: BTreeSet::from([target.id.clone()]),
                });
        }
    }

    Ok(declarations.into_values().collect())
}

fn selected_declarations(catalog: &ImageCatalog, names: &[String]) -> Result<Vec<ToolDeclaration>> {
    let declarations = tool_declarations(catalog)?;

    if names.is_empty() {
        return Ok(declarations);
    }

    let requested = names.iter().map(String::as_str).collect::<BTreeSet<_>>();
    let known = declarations
        .iter()
        .map(|declaration| declaration.name.as_str())
        .collect::<BTreeSet<_>>();

    for name in &requested {
        ensure!(known.contains(name), "unknown tool `{name}`");
    }

    Ok(declarations
        .into_iter()
        .filter(|declaration| requested.contains(declaration.name.as_str()))
        .collect())
}

fn check_declarations(
    client: &RegistryClient,
    declarations: &[ToolDeclaration],
    allow_major: bool,
) -> Result<Vec<ToolCheck>> {
    declarations
        .iter()
        .map(|declaration| check_declaration(client, declaration, allow_major))
        .collect()
}

fn check_declaration(
    client: &RegistryClient,
    declaration: &ToolDeclaration,
    allow_major: bool,
) -> Result<ToolCheck> {
    let image = declaration
        .image_ref
        .as_ref()
        .with_context(|| format!("tool `{}` is not image-backed", declaration.name))?;
    ensure!(
        image.registry == "ghcr.io",
        "tool `{}` image registry `{}` is not supported by `tool update`",
        declaration.name,
        image.registry
    );
    let suffix = declaration.tag_suffix.as_deref().with_context(|| {
        format!(
            "tool `{}` image tag `{}` must start with release `{}`",
            declaration.name, image.tag, declaration.release
        )
    })?;
    let current = Version::parse(&declaration.release).with_context(|| {
        format!(
            "tool `{}` release `{}` is not a stable semver version",
            declaration.name, declaration.release
        )
    })?;

    ensure!(
        current.pre.is_empty() && current.build.is_empty(),
        "tool `{}` release `{}` is not a stable semver version",
        declaration.name,
        declaration.release
    );

    let tags = client.tags(&image.repository)?;
    let versions = matching_versions(&tags, suffix);
    ensure!(
        !versions.is_empty(),
        "tool `{}` has no semver tags matching suffix `{suffix}`",
        declaration.name
    );

    let newest = versions
        .iter()
        .max_by(|left, right| left.version.cmp(&right.version))
        .expect("versions is not empty");
    let allowed = versions
        .iter()
        .filter(|candidate| allow_major || candidate.version.major == current.major)
        .max_by(|left, right| left.version.cmp(&right.version));
    let selected = match allowed {
        Some(candidate) if candidate.version > current => Some(candidate),
        _ => None,
    };

    let (status, latest) = if newest.version <= current {
        (ToolStatus::Current, newest)
    } else if let Some(candidate) = selected {
        (ToolStatus::UpdateAvailable, candidate)
    } else {
        (ToolStatus::MajorSkipped, newest)
    };

    let latest_digest = if latest.version == current {
        image.digest.clone()
    } else {
        client.manifest_digest(&image.repository, &latest.tag)?
    };
    let latest_image = image.with_tag_and_digest(&latest.tag, &latest_digest);
    let (update_release, update_image) = if status == ToolStatus::UpdateAvailable {
        (Some(latest.version.to_string()), Some(latest_image.clone()))
    } else {
        (None, None)
    };

    Ok(ToolCheck {
        name: declaration.name.clone(),
        status,
        current_release: declaration.release.clone(),
        latest_release: latest.version.to_string(),
        current_image: declaration
            .image
            .clone()
            .expect("image-backed declaration must have image"),
        latest_image,
        current_digest: image.digest.clone(),
        latest_digest,
        update_release,
        update_image,
        definition_files: declaration.definition_files.clone(),
    })
}

#[derive(Debug)]
struct CandidateVersion {
    version: Version,
    tag: String,
}

fn matching_versions(tags: &[String], suffix: &str) -> Vec<CandidateVersion> {
    let mut versions = Vec::new();

    for tag in tags {
        let Some(version_text) = tag.strip_suffix(suffix) else {
            continue;
        };
        let Ok(version) = Version::parse(version_text) else {
            continue;
        };

        if version.pre.is_empty() && version.build.is_empty() {
            versions.push(CandidateVersion {
                version,
                tag: tag.clone(),
            });
        }
    }

    versions
}

fn apply_updates(updates: &[PlannedToolUpdate]) -> Result<()> {
    let mut files = BTreeSet::new();
    for update in updates {
        files.extend(update.definition_files.iter().cloned());
    }

    for file in files {
        let mut contents = fs::read_to_string(&file)
            .with_context(|| format!("failed to read {}", file.display()))?;

        for update in updates
            .iter()
            .filter(|update| update.definition_files.contains(&file))
        {
            contents = update_tool_block(&contents, update)?;
        }

        fs::write(&file, contents)
            .with_context(|| format!("failed to write {}", file.display()))?;
    }

    Ok(())
}

fn update_tool_block(contents: &str, update: &PlannedToolUpdate) -> Result<String> {
    let range = tool_block_range(contents, &update.name)
        .with_context(|| format!("tool block `[tools.{}]` was not found", update.name))?;
    let block = &contents[range.clone()];
    let old_release = format!("release = \"{}\"", update.old_release);
    let new_release = format!("release = \"{}\"", update.new_release);
    let old_image = format!("image = \"{}\"", update.old_image);
    let new_image = format!("image = \"{}\"", update.new_image);

    ensure!(
        block.contains(&old_release),
        "tool `{}` block does not contain expected release `{}`",
        update.name,
        update.old_release
    );
    ensure!(
        block.contains(&old_image),
        "tool `{}` block does not contain expected image `{}`",
        update.name,
        update.old_image
    );

    let updated_block = block
        .replace(&old_release, &new_release)
        .replace(&old_image, &new_image);
    let mut updated = String::with_capacity(contents.len() + updated_block.len() - block.len());
    updated.push_str(&contents[..range.start]);
    updated.push_str(&updated_block);
    updated.push_str(&contents[range.end..]);

    Ok(updated)
}

fn tool_block_range(contents: &str, tool_name: &str) -> Option<Range<usize>> {
    let header = format!("[tools.{tool_name}]");
    let mut start = None;
    let mut offset = 0;

    for line in contents.split_inclusive('\n') {
        let trimmed = line.trim();
        if start.is_none() && trimmed == header {
            start = Some(offset);
        } else if start.is_some() && trimmed.starts_with('[') {
            return Some(start.expect("start is set")..offset);
        }

        offset += line.len();
    }

    start.map(|start| start..contents.len())
}

fn parse_pinned_image_reference(reference: &str) -> Result<ImageReference> {
    let (tag_reference, digest) = reference
        .split_once('@')
        .with_context(|| format!("image reference `{reference}` must be pinned by digest"))?;
    ensure!(
        digest.starts_with("sha256:"),
        "image reference `{reference}` must use a sha256 digest"
    );

    let slash = tag_reference
        .find('/')
        .with_context(|| format!("image reference `{reference}` must include a registry"))?;
    let colon = tag_reference
        .rfind(':')
        .with_context(|| format!("image reference `{reference}` must include a tag"))?;
    ensure!(
        colon > slash,
        "image reference `{reference}` must include a tag after the repository path"
    );

    Ok(ImageReference {
        registry: tag_reference[..slash].to_string(),
        repository: tag_reference[slash + 1..colon].to_string(),
        tag: tag_reference[colon + 1..].to_string(),
        digest: digest.to_string(),
    })
}

fn derive_tag_suffix(release: &str, tag: &str) -> Result<String> {
    tag.strip_prefix(release)
        .map(str::to_string)
        .with_context(|| format!("tag `{tag}` does not start with release `{release}`"))
}

struct RegistryClient {
    agent: ureq::Agent,
}

impl RegistryClient {
    fn new() -> Self {
        let agent = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(30)))
            .http_status_as_error(false)
            .build()
            .new_agent();

        Self { agent }
    }

    fn tags(&self, repository: &str) -> Result<Vec<String>> {
        let token = self.token(repository)?;
        let url = format!("https://ghcr.io/v2/{repository}/tags/list?n=1000");
        let mut response = self
            .agent
            .get(&url)
            .header("Authorization", format!("Bearer {token}"))
            .header("User-Agent", user_agent())
            .call()
            .with_context(|| format!("failed to fetch tags for ghcr.io/{repository}"))?;

        ensure!(
            response.status().is_success(),
            "failed to fetch tags for ghcr.io/{repository}: HTTP {}",
            response.status().as_u16()
        );

        let body = response.body_mut().read_to_string()?;
        let tags: TagsResponse = serde_json::from_str(&body)
            .with_context(|| format!("failed to parse tag list for ghcr.io/{repository}"))?;

        Ok(tags.tags.unwrap_or_default())
    }

    fn manifest_digest(&self, repository: &str, tag: &str) -> Result<String> {
        let token = self.token(repository)?;
        let url = format!("https://ghcr.io/v2/{repository}/manifests/{tag}");
        let response = self
            .agent
            .head(&url)
            .header("Authorization", format!("Bearer {token}"))
            .header("Accept", OCI_ACCEPT)
            .header("User-Agent", user_agent())
            .call()
            .with_context(|| {
                format!("failed to fetch manifest digest for ghcr.io/{repository}:{tag}")
            })?;

        ensure!(
            response.status().is_success(),
            "failed to fetch manifest digest for ghcr.io/{repository}:{tag}: HTTP {}",
            response.status().as_u16()
        );

        response
            .headers()
            .get("docker-content-digest")
            .and_then(|value| value.to_str().ok())
            .map(str::to_string)
            .with_context(|| {
                format!("manifest for ghcr.io/{repository}:{tag} did not include a digest")
            })
    }

    fn token(&self, repository: &str) -> Result<String> {
        let scope = format!("repository:{repository}:pull");
        let url = format!(
            "https://ghcr.io/token?service=ghcr.io&scope={}",
            percent_encode(&scope)
        );
        let mut response = self
            .agent
            .get(&url)
            .header("User-Agent", user_agent())
            .call()
            .with_context(|| format!("failed to request ghcr.io token for {repository}"))?;

        ensure!(
            response.status().is_success(),
            "failed to request ghcr.io token for {repository}: HTTP {}",
            response.status().as_u16()
        );

        let body = response.body_mut().read_to_string()?;
        let token: TokenResponse = serde_json::from_str(&body)
            .with_context(|| format!("failed to parse ghcr.io token for {repository}"))?;

        token
            .token
            .or(token.access_token)
            .context("ghcr.io token response did not include a token")
    }
}

#[derive(Debug, Deserialize)]
struct TagsResponse {
    tags: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    token: Option<String>,
    access_token: Option<String>,
}

fn user_agent() -> String {
    format!("keeline/{}", env!("CARGO_PKG_VERSION"))
}

fn print_checks(checks: &[ToolCheck]) {
    let rows = checks
        .iter()
        .map(|check| {
            vec![
                check.name.clone(),
                check.current_release.clone(),
                check.latest_release.clone(),
                check.status.as_str().to_string(),
                short_digest(&check.current_digest),
                short_digest(&check.latest_digest),
            ]
        })
        .collect::<Vec<_>>();

    print_table(
        &[
            "TOOL",
            "CURRENT",
            "LATEST",
            "STATUS",
            "CURRENT DIGEST",
            "LATEST DIGEST",
        ],
        &rows,
    );
}

fn print_table(headers: &[&str], rows: &[Vec<String>]) {
    let mut widths = headers
        .iter()
        .map(|header| header.len())
        .collect::<Vec<_>>();

    for row in rows {
        for (index, value) in row.iter().enumerate() {
            widths[index] = widths[index].max(value.len());
        }
    }

    for (index, header) in headers.iter().enumerate() {
        if index > 0 {
            print!("  ");
        }
        print!("{header:<width$}", width = widths[index]);
    }
    println!();

    for (index, width) in widths.iter().enumerate() {
        if index > 0 {
            print!("  ");
        }
        print!("{:-<width$}", "", width = width);
    }
    println!();

    for row in rows {
        for (index, value) in row.iter().enumerate() {
            if index > 0 {
                print!("  ");
            }
            print!("{value:<width$}", width = widths[index]);
        }
        println!();
    }
}

fn short_digest(digest: &str) -> String {
    let hash = digest.strip_prefix("sha256:").unwrap_or(digest);
    let prefix = hash.get(..12).unwrap_or(hash);
    format!("sha256:{prefix}")
}

fn updated_file_count(updates: &[PlannedToolUpdate]) -> usize {
    updates
        .iter()
        .flat_map(|update| update.definition_files.iter())
        .collect::<BTreeSet<_>>()
        .len()
}

fn render_summary(summary: render::SyncSummary) -> String {
    let changed = summary.generated + summary.updated;

    if changed == 0 {
        return "all dockerfiles already up to date".to_string();
    }

    format!(
        "generated {} dockerfile{}, updated {} dockerfile{}, {} unchanged",
        summary.generated,
        plural(summary.generated),
        summary.updated,
        plural(summary.updated),
        summary.unchanged
    )
}

fn plural(count: usize) -> &'static str {
    if count == 1 { "" } else { "s" }
}

fn percent_encode(value: &str) -> String {
    let mut encoded = String::new();

    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                encoded.push(byte as char)
            }
            _ => encoded.push_str(&format!("%{byte:02X}")),
        }
    }

    encoded
}

#[cfg(test)]
mod tests {
    use super::{
        PlannedToolUpdate, derive_tag_suffix, matching_versions, parse_pinned_image_reference,
        percent_encode, update_tool_block,
    };

    #[test]
    fn parses_pinned_ghcr_image_references() {
        let image = parse_pinned_image_reference(
            "ghcr.io/lvillis/motdyn:1.0.14-slim@sha256:68eb88ae6031b08afaade56e04a0497f6139f80445bb6b3d27dc03a294ed1ef6",
        )
        .unwrap();

        assert_eq!(image.registry, "ghcr.io");
        assert_eq!(image.repository, "lvillis/motdyn");
        assert_eq!(image.tag, "1.0.14-slim");
        assert_eq!(image.tag_reference(), "ghcr.io/lvillis/motdyn:1.0.14-slim");
    }

    #[test]
    fn derives_custom_tag_suffix_from_release() {
        assert_eq!(derive_tag_suffix("1.0.14", "1.0.14-slim").unwrap(), "-slim");
        assert_eq!(derive_tag_suffix("0.1.26", "0.1.26").unwrap(), "");
        assert!(derive_tag_suffix("1.0.14", "latest").is_err());
    }

    #[test]
    fn matches_stable_versions_with_expected_suffix() {
        let tags = vec![
            "1.0.13-slim".to_string(),
            "1.0.14-slim".to_string(),
            "1.0.15".to_string(),
            "1.0.15-beta-slim".to_string(),
            "sha256:example".to_string(),
        ];
        let versions = matching_versions(&tags, "-slim");

        assert_eq!(
            versions
                .iter()
                .map(|candidate| candidate.version.to_string())
                .collect::<Vec<_>>(),
            vec!["1.0.13", "1.0.14"]
        );
    }

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
    fn percent_encodes_registry_scopes() {
        assert_eq!(
            percent_encode("repository:lvillis/motdyn:pull"),
            "repository%3Alvillis%2Fmotdyn%3Apull"
        );
    }
}
