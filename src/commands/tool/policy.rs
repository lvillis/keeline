use anyhow::{Context, Result, ensure};
use semver::Version;

use super::model::{ToolCheck, ToolDeclaration, ToolStatus};
use super::registry::Registry;

pub(super) fn check_all(
    client: &impl Registry,
    declarations: &[ToolDeclaration],
    allow_major: bool,
) -> Result<Vec<ToolCheck>> {
    declarations
        .iter()
        .map(|declaration| check(client, declaration, allow_major))
        .collect()
}

fn check(
    client: &impl Registry,
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
    ensure!(
        tags.iter().any(|tag| tag == &image.tag),
        "tool `{}` current image tag `{}` no longer exists in ghcr.io/{}",
        declaration.name,
        image.tag,
        image.repository
    );
    let mut versions = matching_versions(&tags, suffix);
    ensure!(
        !versions.is_empty(),
        "tool `{}` has no semver tags matching suffix `{suffix}`",
        declaration.name
    );
    versions.sort_by(|left, right| left.version.cmp(&right.version));

    let newest = versions
        .last()
        .context("matching tool versions unexpectedly became empty")?;
    let selected_update = versions
        .iter()
        .rfind(|candidate| allow_major || candidate.version.major == current.major)
        .filter(|candidate| candidate.version > current);

    let (status, update_release, update_image, latest_image, latest_digest) =
        if let Some(candidate) = selected_update {
            let digest = client.manifest_digest(&image.repository, &candidate.tag)?;
            let candidate_image = image.with_tag_and_digest(&candidate.tag, &digest);
            (
                ToolStatus::UpdateAvailable,
                Some(candidate.version.to_string()),
                Some(candidate_image.clone()),
                candidate_image,
                digest,
            )
        } else {
            let current_digest = client.manifest_digest(&image.repository, &image.tag)?;
            if current_digest != image.digest {
                let refreshed_image = image.with_tag_and_digest(&image.tag, &current_digest);
                (
                    ToolStatus::DigestChanged,
                    Some(declaration.release.clone()),
                    Some(refreshed_image.clone()),
                    refreshed_image,
                    current_digest,
                )
            } else if newest.version > current {
                let newest_digest = client.manifest_digest(&image.repository, &newest.tag)?;
                (
                    ToolStatus::MajorSkipped,
                    None,
                    None,
                    image.with_tag_and_digest(&newest.tag, &newest_digest),
                    newest_digest,
                )
            } else {
                (
                    ToolStatus::Current,
                    None,
                    None,
                    image.with_tag_and_digest(&image.tag, &image.digest),
                    image.digest.clone(),
                )
            }
        };

    let current_image = declaration
        .image
        .clone()
        .with_context(|| format!("tool `{}` is not image-backed", declaration.name))?;

    Ok(ToolCheck {
        name: declaration.name.clone(),
        status,
        current_release: declaration.release.clone(),
        latest_release: update_release
            .clone()
            .unwrap_or_else(|| newest.version.to_string()),
        current_image,
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
    tags.iter()
        .filter_map(|tag| {
            let version = Version::parse(tag.strip_suffix(suffix)?).ok()?;
            (version.pre.is_empty() && version.build.is_empty()).then(|| CandidateVersion {
                version,
                tag: tag.clone(),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use anyhow::{Context, Result};

    use super::{check, matching_versions};
    use crate::commands::tool::model::{ImageReference, ToolDeclaration, ToolStatus};
    use crate::commands::tool::registry::Registry;
    use crate::domain::ToolRole;

    struct MockRegistry {
        tags: Vec<String>,
        digests: BTreeMap<String, String>,
    }

    impl Registry for MockRegistry {
        fn tags(&self, _repository: &str) -> Result<Vec<String>> {
            Ok(self.tags.clone())
        }

        fn manifest_digest(&self, _repository: &str, tag: &str) -> Result<String> {
            self.digests
                .get(tag)
                .cloned()
                .with_context(|| format!("unexpected digest lookup for `{tag}`"))
        }
    }

    fn declaration() -> ToolDeclaration {
        let image = "ghcr.io/lvillis/example:1.0.0-slim@sha256:old".to_string();
        ToolDeclaration {
            name: "example".to_string(),
            role: ToolRole::Utility,
            release: "1.0.0".to_string(),
            image: Some(image),
            source_path: "/bin/example".to_string(),
            target_path: "/bin/example".to_string(),
            tag_suffix: Some("-slim".to_string()),
            image_ref: Some(ImageReference {
                registry: "ghcr.io".to_string(),
                repository: "lvillis/example".to_string(),
                tag: "1.0.0-slim".to_string(),
                digest: "sha256:old".to_string(),
            }),
            definition_files: BTreeSet::new(),
            target_ids: BTreeSet::new(),
        }
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
    fn rejects_a_missing_current_tag_before_reading_a_digest() {
        let registry = MockRegistry {
            tags: vec!["1.0.1-slim".to_string()],
            digests: BTreeMap::new(),
        };

        let error = check(&registry, &declaration(), false).unwrap_err();

        assert!(error.to_string().contains("current image tag"));
    }

    #[test]
    fn resolves_the_digest_for_the_selected_update_tag() {
        let registry = MockRegistry {
            tags: vec![
                "1.0.0-slim".to_string(),
                "1.1.0-slim".to_string(),
                "2.0.0-slim".to_string(),
            ],
            digests: BTreeMap::from([("1.1.0-slim".to_string(), "sha256:selected".to_string())]),
        };

        let check = check(&registry, &declaration(), false).unwrap();

        assert_eq!(check.status, ToolStatus::UpdateAvailable);
        assert_eq!(check.latest_release, "1.1.0");
        assert_eq!(check.latest_digest, "sha256:selected");
        assert!(
            check
                .update_image
                .unwrap()
                .ends_with("1.1.0-slim@sha256:selected")
        );
    }

    #[test]
    fn reports_digest_drift_for_the_exact_current_tag() {
        let registry = MockRegistry {
            tags: vec!["1.0.0-slim".to_string()],
            digests: BTreeMap::from([("1.0.0-slim".to_string(), "sha256:changed".to_string())]),
        };

        let check = check(&registry, &declaration(), false).unwrap();

        assert_eq!(check.status, ToolStatus::DigestChanged);
        assert_eq!(check.latest_digest, "sha256:changed");
    }

    #[test]
    fn reports_current_digest_drift_before_a_skipped_major_update() {
        let registry = MockRegistry {
            tags: vec!["1.0.0-slim".to_string(), "2.0.0-slim".to_string()],
            digests: BTreeMap::from([("1.0.0-slim".to_string(), "sha256:changed".to_string())]),
        };

        let check = check(&registry, &declaration(), false).unwrap();

        assert_eq!(check.status, ToolStatus::DigestChanged);
        assert_eq!(check.latest_digest, "sha256:changed");
    }
}
