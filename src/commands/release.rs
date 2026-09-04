use std::path::PathBuf;

use anyhow::{Result, ensure};

use crate::cli::ReleaseArgs;
use crate::commands::{
    RELEASE_METADATA_DIR, build_args, platform_suffix, registry_cache_from, registry_cache_to,
    select_release_targets, tag_with_suffix,
};
use crate::docker::{BuildOutput, DockerBuild};
use crate::domain::{ImageCatalog, ImageTarget};
use crate::render;

pub fn run(catalog: &ImageCatalog, args: &ReleaseArgs) -> Result<()> {
    catalog.validate()?;

    let targets = select_release_targets(catalog, args.image_id.as_deref())?;

    let tag_suffix = platform_suffix(args.platform);

    for target in targets {
        render::check_target(target)?;

        ensure!(
            target.platforms.contains(&args.platform),
            "image `{}` does not support platform `{}`",
            target.id,
            args.platform
        );
        let platforms = vec![args.platform];

        let repository = target.repository(&args.owner);
        let cache_tag = format!("buildcache-{tag_suffix}");
        let tags = target
            .all_tags()
            .into_iter()
            .map(|tag| format!("{repository}:{}", tag_with_suffix(&tag, tag_suffix)))
            .collect();

        let request = DockerBuild {
            context: target.context.clone(),
            dockerfile: target.dockerfile.clone(),
            build_args: build_args(),
            cache_from: registry_cache_from(&repository, &cache_tag),
            cache_to: registry_cache_to(&repository, &cache_tag),
            metadata_file: Some(metadata_file(target, tag_suffix)),
            tags,
            platforms,
            attestations: true,
            output: BuildOutput::Push,
        };

        if args.dry_run {
            println!("{}", request.display());
            continue;
        }

        request.run()?;
    }

    Ok(())
}

fn metadata_file(target: &ImageTarget, tag_suffix: &str) -> PathBuf {
    let file_name = format!("{}-{tag_suffix}.json", target.id);
    PathBuf::from(RELEASE_METADATA_DIR).join(file_name)
}
