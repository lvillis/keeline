use std::path::PathBuf;

use anyhow::{Context, Result, bail, ensure};

use crate::cli::ReleaseArgs;
use crate::commands::{
    RELEASE_METADATA_DIR, build_args, platform_suffix, registry_cache_from, registry_cache_to,
    tag_with_suffix,
};
use crate::docker::DockerBuild;
use crate::domain::{ImageCatalog, ImageTarget};
use crate::render;

pub fn run(catalog: &ImageCatalog, args: &ReleaseArgs) -> Result<()> {
    catalog.validate()?;

    let targets: Vec<&ImageTarget> = match &args.image_id {
        Some(image_id) => vec![{
            let target = catalog
                .target(image_id)
                .with_context(|| format!("unknown image id `{image_id}`"))?;
            ensure!(
                target.is_releasable(),
                "image `{}` is not releasable: publish={}, status={}",
                target.id,
                target.publish,
                target.status_label()
            );
            target
        }],
        None => catalog.release_targets().collect(),
    };

    ensure!(
        !targets.is_empty(),
        "no releasable image targets were selected"
    );

    let tag_suffix = match (&args.platform, &args.tag_suffix) {
        (Some(platform), Some(suffix)) => {
            ensure!(
                !suffix.trim().is_empty(),
                "release tag suffix cannot be empty when --platform is used"
            );
            platform_suffix(platform)?;
            Some(suffix.clone())
        }
        (Some(platform), None) => Some(platform_suffix(platform)?.to_string()),
        (None, Some(_)) => bail!("--tag-suffix can only be used together with --platform"),
        (None, None) => None,
    };

    for target in targets {
        render::sync_target(target)?;

        let platforms = if let Some(platform) = &args.platform {
            ensure!(
                target.platforms.contains(platform),
                "image `{}` does not support platform `{platform}`",
                target.id
            );
            vec![platform.clone()]
        } else {
            target.platforms.clone()
        };

        let repository = target.repository(&args.owner);
        let cache_tag = match &tag_suffix {
            Some(suffix) => format!("buildcache-{suffix}"),
            None => "buildcache".to_string(),
        };
        let tags = target
            .all_tags()
            .into_iter()
            .map(|tag| match &tag_suffix {
                Some(suffix) => format!("{repository}:{}", tag_with_suffix(&tag, suffix)),
                None => format!("{repository}:{tag}"),
            })
            .collect();

        let request = DockerBuild {
            context: target.context.clone(),
            dockerfile: target.dockerfile.clone(),
            build_args: build_args(),
            cache_from: registry_cache_from(&repository, &cache_tag),
            cache_to: registry_cache_to(&repository, &cache_tag),
            metadata_file: Some(metadata_file(target, tag_suffix.as_deref())),
            tags,
            platforms,
            sbom: true,
            provenance: true,
            push: true,
            load: false,
        };

        if args.dry_run {
            println!("{}", request.display());
            continue;
        }

        request.run()?;
    }

    Ok(())
}

fn metadata_file(target: &ImageTarget, tag_suffix: Option<&str>) -> PathBuf {
    let file_name = match tag_suffix {
        Some(suffix) => format!("{}-{suffix}.json", target.id),
        None => format!("{}.json", target.id),
    };

    PathBuf::from(RELEASE_METADATA_DIR).join(file_name)
}
