use std::collections::HashSet;

use anyhow::{Result, bail, ensure};
use regex::Regex;

use crate::domain::model::{ImageCatalog, ImageTarget, SourceArchive};

const BANNED_TAGS: &[&str] = &[
    "latest",
    "stable",
    "v21",
    "jdk-21",
    "bookworm-21",
    "trixie-21",
];

pub fn validate(catalog: &ImageCatalog) -> Result<()> {
    ensure!(
        catalog.root.exists(),
        "images directory does not exist: {}",
        catalog.root.display()
    );
    ensure!(
        !catalog.targets.is_empty(),
        "no image.toml definitions were discovered"
    );

    let package_pattern = Regex::new(r"^keeline-[a-z0-9-]+$")?;
    let family_pattern = Regex::new(r"^[a-z0-9]+$")?;
    let image_reference_pattern =
        Regex::new(r"^(?:scratch|[a-z0-9./_-]+:[A-Za-z0-9._-]+(?:@sha256:[a-f0-9]{64})?)$")?;
    let platform_pattern = Regex::new(r"^[a-z0-9]+/[a-z0-9]+$")?;
    let debian_canonical = Regex::new(r"^[0-9]+(?:\.[0-9]+)?(?:-[a-z0-9]+)?$")?;
    let debian_alias = Regex::new(r"^[a-z][a-z0-9]*(?:-[a-z0-9]+)?$")?;
    let java_canonical = Regex::new(
        r"^(?:jdk|jre|runtime)-[0-9]+(?:(?:\.[0-9]+)*|u[0-9]+)?-[a-z0-9]+(?:-[a-z0-9]+)?$",
    )?;
    let generic_tag = Regex::new(r"^[a-z0-9][a-z0-9.-]*$")?;
    let sha256_pattern = Regex::new(r"^[a-f0-9]{64}$")?;

    let mut ids = HashSet::new();
    let mut published_tags = HashSet::new();

    for target in &catalog.targets {
        ensure!(
            target.schema == 1,
            "image `{}` declares unsupported schema `{}`",
            target.id,
            target.schema
        );
        ensure!(
            ids.insert(target.id.clone()),
            "duplicate image id `{}` from {}",
            target.id,
            target.definition_file.display()
        );

        ensure!(
            package_pattern.is_match(&target.package),
            "package `{}` is invalid for {}",
            target.package,
            target.definition_file.display()
        );
        ensure!(
            family_pattern.is_match(&target.family),
            "image `{}` declares invalid family `{}`",
            target.id,
            target.family
        );
        ensure!(
            target.package == format!("keeline-{}", target.family),
            "image `{}` package `{}` does not match family `{}`",
            target.id,
            target.package,
            target.family
        );

        ensure!(
            !target.canonical_tags.is_empty(),
            "image `{}` must declare at least one canonical tag",
            target.id
        );
        ensure!(
            !target.platforms.is_empty(),
            "image `{}` must declare at least one platform",
            target.id
        );
        ensure!(
            image_reference_pattern.is_match(&target.base_image),
            "image `{}` declares invalid base image `{}`",
            target.id,
            target.base_image
        );
        if let Some(builder_image) = &target.builder_image {
            ensure!(
                image_reference_pattern.is_match(builder_image),
                "image `{}` declares invalid builder image `{}`",
                target.id,
                builder_image
            );
        }
        ensure!(
            !target.title.trim().is_empty(),
            "image `{}` must declare a title",
            target.id
        );
        ensure!(
            !target.description.trim().is_empty(),
            "image `{}` must declare a description",
            target.id
        );
        ensure!(
            !target.command.is_empty(),
            "image `{}` must declare a default command",
            target.id
        );

        for platform in &target.platforms {
            ensure!(
                platform_pattern.is_match(platform),
                "image `{}` declares invalid platform `{platform}`",
                target.id
            );
        }

        validate_init_runtime(target, &image_reference_pattern, &sha256_pattern)?;
        validate_healthcheck_runtime(target, &image_reference_pattern, &sha256_pattern)?;

        match target.family.as_str() {
            "debian" => validate_debian_target(target)?,
            "java" => validate_java_target(target, &sha256_pattern)?,
            "scratch" => validate_scratch_target(target)?,
            _ => {
                ensure!(
                    target.source.is_none(),
                    "image `{}` family `{}` cannot declare a source yet",
                    target.id,
                    target.family
                );
            }
        }

        validate_tags(
            target,
            &debian_canonical,
            &debian_alias,
            &java_canonical,
            &generic_tag,
            &mut published_tags,
        )?;
    }

    Ok(())
}

fn validate_debian_target(target: &ImageTarget) -> Result<()> {
    ensure!(
        target.source.is_none(),
        "debian image `{}` must not declare an upstream source",
        target.id
    );
    ensure!(
        target.java.is_none(),
        "debian image `{}` must not declare java runtime metadata",
        target.id
    );

    Ok(())
}

fn validate_scratch_target(target: &ImageTarget) -> Result<()> {
    ensure!(
        target.source.is_none(),
        "scratch image `{}` must not declare an upstream source",
        target.id
    );
    ensure!(
        target.java.is_none(),
        "scratch image `{}` must not declare java runtime metadata",
        target.id
    );
    ensure!(
        target.base_image == "scratch",
        "scratch image `{}` must use base image `scratch`",
        target.id
    );
    ensure!(
        target.builder_image.is_some(),
        "scratch image `{}` must declare a builder image",
        target.id
    );

    Ok(())
}

fn validate_init_runtime(
    target: &ImageTarget,
    image_reference_pattern: &Regex,
    sha256_pattern: &Regex,
) -> Result<()> {
    let init = target
        .init
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("image `{}` must declare init metadata", target.id))?;

    ensure!(
        init.provider == "tino",
        "image `{}` must use init provider `tino`",
        target.id
    );
    ensure!(
        !init.release.trim().is_empty(),
        "image `{}` must declare an init release",
        target.id
    );
    ensure!(
        init.binary_path.starts_with('/'),
        "image `{}` must declare an absolute init binary path",
        target.id
    );
    ensure!(
        !init.entrypoint.is_empty(),
        "image `{}` must declare an init entrypoint",
        target.id
    );
    ensure!(
        init.entrypoint[0] == init.binary_path,
        "image `{}` init entrypoint must start with `{}`",
        target.id,
        init.binary_path
    );
    validate_tool_source(
        target,
        "init",
        init.image.as_deref(),
        &init.install_packages,
        &init.archives,
        image_reference_pattern,
        sha256_pattern,
    )?;

    Ok(())
}

fn validate_tool_source(
    target: &ImageTarget,
    kind: &str,
    image: Option<&str>,
    install_packages: &[String],
    archives: &[SourceArchive],
    image_reference_pattern: &Regex,
    sha256_pattern: &Regex,
) -> Result<()> {
    ensure!(
        image.is_some() || !archives.is_empty(),
        "image `{}` must declare either a {kind} image or {kind} archives",
        target.id
    );
    ensure!(
        image.is_none() || archives.is_empty(),
        "image `{}` cannot declare both a {kind} image and {kind} archives",
        target.id
    );

    if let Some(image) = image {
        ensure!(
            image_reference_pattern.is_match(image),
            "image `{}` declares invalid {kind} image `{image}`",
            target.id
        );
        ensure!(
            image.contains("@sha256:"),
            "image `{}` {kind} image must be pinned by digest",
            target.id
        );
        return Ok(());
    }

    ensure!(
        !install_packages.is_empty(),
        "image `{}` must declare {kind} install packages",
        target.id
    );

    let mut seen_archives = HashSet::new();
    for archive in archives {
        ensure!(
            seen_archives.insert(archive.platform.clone()),
            "image `{}` repeats {kind} archive for platform `{}`",
            target.id,
            archive.platform
        );
        ensure!(
            target.platforms.contains(&archive.platform),
            "image `{}` declares {kind} archive for unsupported platform `{}`",
            target.id,
            archive.platform
        );
        ensure!(
            archive.url.starts_with("https://"),
            "image `{}` {kind} archive URL must use https: {}",
            target.id,
            archive.url
        );
        ensure!(
            sha256_pattern.is_match(&archive.sha256),
            "image `{}` {kind} archive checksum is invalid for platform `{}`",
            target.id,
            archive.platform
        );
    }

    for platform in &target.platforms {
        ensure!(
            archives.iter().any(|archive| archive.platform == *platform),
            "image `{}` is missing a {kind} archive for platform `{platform}`",
            target.id
        );
    }

    Ok(())
}

fn validate_healthcheck_runtime(
    target: &ImageTarget,
    image_reference_pattern: &Regex,
    sha256_pattern: &Regex,
) -> Result<()> {
    let healthcheck = target.healthcheck.as_ref().ok_or_else(|| {
        anyhow::anyhow!("image `{}` must declare healthcheck metadata", target.id)
    })?;

    ensure!(
        healthcheck.provider == "salus",
        "image `{}` must use healthcheck provider `salus`",
        target.id
    );
    ensure!(
        !healthcheck.release.trim().is_empty(),
        "image `{}` must declare a healthcheck release",
        target.id
    );
    ensure!(
        healthcheck.binary_path.starts_with('/'),
        "image `{}` must declare an absolute healthcheck binary path",
        target.id
    );
    validate_tool_source(
        target,
        "healthcheck",
        healthcheck.image.as_deref(),
        &healthcheck.install_packages,
        &healthcheck.archives,
        image_reference_pattern,
        sha256_pattern,
    )?;

    Ok(())
}

fn validate_java_target(target: &ImageTarget, sha256_pattern: &Regex) -> Result<()> {
    let source = target
        .source
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("java image `{}` must declare a source", target.id))?;
    let java = target.java.as_ref().ok_or_else(|| {
        anyhow::anyhow!(
            "java image `{}` must declare java runtime metadata",
            target.id
        )
    })?;

    ensure!(
        source.provider == "adoptium-temurin",
        "java image `{}` must use provider `adoptium-temurin`",
        target.id
    );
    ensure!(
        !source.release.trim().is_empty(),
        "java image `{}` must declare a source release",
        target.id
    );
    ensure!(
        !source.gpg_key.trim().is_empty(),
        "java image `{}` must declare a GPG key",
        target.id
    );
    ensure!(
        !source.archives.is_empty(),
        "java image `{}` must declare source archives",
        target.id
    );
    ensure!(
        !java.java_home.trim().is_empty(),
        "java image `{}` must declare JAVA_HOME",
        target.id
    );
    ensure!(
        !java.builder_packages.is_empty(),
        "java image `{}` must declare builder packages",
        target.id
    );
    ensure!(
        !java.runtime_packages.is_empty(),
        "java image `{}` must declare runtime packages",
        target.id
    );
    ensure!(
        !java.lang.trim().is_empty(),
        "java image `{}` must declare LANG",
        target.id
    );
    ensure!(
        !java.language.trim().is_empty(),
        "java image `{}` must declare LANGUAGE",
        target.id
    );
    ensure!(
        !java.lc_all.trim().is_empty(),
        "java image `{}` must declare LC_ALL",
        target.id
    );
    ensure!(
        !java.verify_commands.is_empty(),
        "java image `{}` must declare verify commands",
        target.id
    );
    if java.generate_locales {
        ensure!(
            java.runtime_packages
                .iter()
                .any(|package| package == "locales"),
            "java image `{}` generates locales but does not install `locales`",
            target.id
        );
    }

    let mut seen_archives = HashSet::new();

    for archive in &source.archives {
        ensure!(
            seen_archives.insert(archive.platform.clone()),
            "java image `{}` repeats archive for platform `{}`",
            target.id,
            archive.platform
        );
        ensure!(
            target.platforms.contains(&archive.platform),
            "java image `{}` declares archive for unsupported platform `{}`",
            target.id,
            archive.platform
        );
        ensure!(
            archive.url.starts_with("https://"),
            "java image `{}` archive URL must use https: {}",
            target.id,
            archive.url
        );
        ensure!(
            sha256_pattern.is_match(&archive.sha256),
            "java image `{}` archive checksum is invalid for platform `{}`",
            target.id,
            archive.platform
        );
    }

    for platform in &target.platforms {
        ensure!(
            target.source_archive_for_platform(platform).is_some(),
            "java image `{}` is missing a source archive for platform `{platform}`",
            target.id
        );
    }

    Ok(())
}

fn validate_tags(
    target: &ImageTarget,
    debian_canonical: &Regex,
    debian_alias: &Regex,
    java_canonical: &Regex,
    generic_tag: &Regex,
    published_tags: &mut HashSet<(String, String)>,
) -> Result<()> {
    let mut local_tags = HashSet::new();

    for tag in &target.canonical_tags {
        ensure!(
            local_tags.insert(tag.clone()),
            "image `{}` repeats tag `{tag}`",
            target.id
        );
        ensure!(
            !BANNED_TAGS.contains(&tag.as_str()),
            "image `{}` uses banned tag `{tag}`",
            target.id
        );

        match target.family.as_str() {
            "debian" => ensure!(
                debian_canonical.is_match(tag),
                "debian image `{}` has invalid canonical tag `{tag}`",
                target.id
            ),
            "java" => ensure!(
                java_canonical.is_match(tag),
                "java image `{}` has invalid canonical tag `{tag}`",
                target.id
            ),
            _ => ensure!(
                generic_tag.is_match(tag),
                "image `{}` has invalid canonical tag `{tag}`",
                target.id
            ),
        }

        ensure!(
            published_tags.insert((target.package.clone(), tag.clone())),
            "package `{}` publishes duplicate tag `{tag}`",
            target.package
        );
    }

    for tag in &target.alias_tags {
        ensure!(
            local_tags.insert(tag.clone()),
            "image `{}` repeats tag `{tag}`",
            target.id
        );
        ensure!(
            !BANNED_TAGS.contains(&tag.as_str()),
            "image `{}` uses banned tag `{tag}`",
            target.id
        );

        match target.family.as_str() {
            "debian" => ensure!(
                debian_alias.is_match(tag),
                "debian image `{}` has invalid alias tag `{tag}`",
                target.id
            ),
            "java" => ensure!(
                java_canonical.is_match(tag) || generic_tag.is_match(tag),
                "java image `{}` has invalid alias tag `{tag}`",
                target.id
            ),
            _ => ensure!(
                generic_tag.is_match(tag),
                "image `{}` has invalid alias tag `{tag}`",
                target.id
            ),
        }

        ensure!(
            published_tags.insert((target.package.clone(), tag.clone())),
            "package `{}` publishes duplicate tag `{tag}`",
            target.package
        );
    }

    if target.variant != "default" && target.id.ends_with("-default") {
        bail!(
            "non-default image `{}` cannot end with `-default`",
            target.id
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use regex::Regex;

    use super::{validate_tags, validate_tool_source};
    use crate::domain::model::{ImageTarget, SourceArchive};

    fn make_target(package: &str, canonical: &[&str], alias: &[&str]) -> ImageTarget {
        ImageTarget {
            schema: 1,
            id: "sample".to_string(),
            family: package.trim_start_matches("keeline-").to_string(),
            line: "21".to_string(),
            version: "21.0.10".to_string(),
            distro: Some("trixie".to_string()),
            package: package.to_string(),
            publish: true,
            status: crate::domain::ImageStatus::Stable,
            variant: "default".to_string(),
            context: "images/sample".into(),
            dockerfile: "images/sample/Dockerfile".into(),
            platforms: vec!["linux/amd64".to_string()],
            base_image: "docker.io/library/debian:13".to_string(),
            builder_image: None,
            title: "Sample".to_string(),
            description: "Sample image".to_string(),
            command: vec!["bash".to_string()],
            canonical_tags: canonical.iter().map(|value| value.to_string()).collect(),
            alias_tags: alias.iter().map(|value| value.to_string()).collect(),
            init: None,
            healthcheck: None,
            source: None,
            java: None,
            definition_file: "images/sample/image.toml".into(),
        }
    }

    #[test]
    fn accepts_debian_tags() {
        let debian_canonical = Regex::new(r"^[0-9]+(?:\.[0-9]+)?(?:-[a-z0-9]+)?$").unwrap();
        let debian_alias = Regex::new(r"^[a-z][a-z0-9]*(?:-[a-z0-9]+)?$").unwrap();
        let java_canonical = Regex::new(
            r"^(?:jdk|jre|runtime)-[0-9]+(?:(?:\.[0-9]+)*|u[0-9]+)?-[a-z0-9]+(?:-[a-z0-9]+)?$",
        )
        .unwrap();
        let generic_tag = Regex::new(r"^[a-z0-9][a-z0-9.-]*$").unwrap();
        let mut published = std::collections::HashSet::new();

        let target = make_target("keeline-debian", &["13", "13-slim"], &["trixie"]);
        validate_tags(
            &target,
            &debian_canonical,
            &debian_alias,
            &java_canonical,
            &generic_tag,
            &mut published,
        )
        .unwrap();
    }

    #[test]
    fn rejects_banned_tags() {
        let debian_canonical = Regex::new(r"^[0-9]+(?:\.[0-9]+)?(?:-[a-z0-9]+)?$").unwrap();
        let debian_alias = Regex::new(r"^[a-z][a-z0-9]*(?:-[a-z0-9]+)?$").unwrap();
        let java_canonical = Regex::new(
            r"^(?:jdk|jre|runtime)-[0-9]+(?:(?:\.[0-9]+)*|u[0-9]+)?-[a-z0-9]+(?:-[a-z0-9]+)?$",
        )
        .unwrap();
        let generic_tag = Regex::new(r"^[a-z0-9][a-z0-9.-]*$").unwrap();
        let mut published = std::collections::HashSet::new();

        let target = make_target("keeline-debian", &["latest"], &[]);
        assert!(
            validate_tags(
                &target,
                &debian_canonical,
                &debian_alias,
                &java_canonical,
                &generic_tag,
                &mut published,
            )
            .is_err()
        );
    }

    #[test]
    fn accepts_jdk_update_tags() {
        let debian_canonical = Regex::new(r"^[0-9]+(?:\.[0-9]+)?(?:-[a-z0-9]+)?$").unwrap();
        let debian_alias = Regex::new(r"^[a-z][a-z0-9]*(?:-[a-z0-9]+)?$").unwrap();
        let java_canonical = Regex::new(
            r"^(?:jdk|jre|runtime)-[0-9]+(?:(?:\.[0-9]+)*|u[0-9]+)?-[a-z0-9]+(?:-[a-z0-9]+)?$",
        )
        .unwrap();
        let generic_tag = Regex::new(r"^[a-z0-9][a-z0-9.-]*$").unwrap();
        let mut published = std::collections::HashSet::new();

        let target = make_target("keeline-java", &["jdk-8u372-trixie"], &[]);
        validate_tags(
            &target,
            &debian_canonical,
            &debian_alias,
            &java_canonical,
            &generic_tag,
            &mut published,
        )
        .unwrap();
    }

    #[test]
    fn accepts_digest_pinned_tool_source_images() {
        let target = make_target("keeline-debian", &["13"], &[]);
        let image_reference =
            Regex::new(r"^(?:scratch|[a-z0-9./_-]+:[A-Za-z0-9._-]+(?:@sha256:[a-f0-9]{64})?)$")
                .unwrap();
        let sha256 = Regex::new(r"^[a-f0-9]{64}$").unwrap();
        let archives: Vec<SourceArchive> = Vec::new();

        validate_tool_source(
            &target,
            "init",
            Some(
                "ghcr.io/lvillis/tino:0.1.26@sha256:8ad7b87083aee56d97f68c355bf57ad0a55ad5b00508f87dd86e148dcf91374b",
            ),
            &[],
            &archives,
            &image_reference,
            &sha256,
        )
        .unwrap();
    }

    #[test]
    fn rejects_mutable_tool_source_images() {
        let target = make_target("keeline-debian", &["13"], &[]);
        let image_reference =
            Regex::new(r"^(?:scratch|[a-z0-9./_-]+:[A-Za-z0-9._-]+(?:@sha256:[a-f0-9]{64})?)$")
                .unwrap();
        let sha256 = Regex::new(r"^[a-f0-9]{64}$").unwrap();
        let archives: Vec<SourceArchive> = Vec::new();

        assert!(
            validate_tool_source(
                &target,
                "init",
                Some("ghcr.io/lvillis/tino:0.1.26"),
                &[],
                &archives,
                &image_reference,
                &sha256,
            )
            .is_err()
        );
    }
}
