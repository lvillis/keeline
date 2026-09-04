use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, ensure};

use super::model::{ImageReference, ToolDeclaration, ToolKey};
use crate::domain::ImageCatalog;

pub(super) fn declarations(catalog: &ImageCatalog) -> Result<Vec<ToolDeclaration>> {
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

pub(super) fn selected(catalog: &ImageCatalog, names: &[String]) -> Result<Vec<ToolDeclaration>> {
    let declarations = declarations(catalog)?;

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
    let suffix = tag
        .strip_prefix(release)
        .with_context(|| format!("tag `{tag}` does not start with release `{release}`"))?;
    ensure!(
        suffix.is_empty() || suffix.starts_with('-'),
        "tag `{tag}` must equal release `{release}` or use a hyphenated suffix"
    );
    Ok(suffix.to_string())
}

#[cfg(test)]
mod tests {
    use super::{derive_tag_suffix, parse_pinned_image_reference};

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
        assert!(derive_tag_suffix("1.0.1", "1.0.10-slim").is_err());
    }
}
