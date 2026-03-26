use anyhow::{Context, Result, ensure};

use crate::cli::ReleaseArgs;
use crate::commands::{build_args, registry_cache_from, registry_cache_to};
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

    for target in targets {
        render::sync_target(target)?;

        let repository = target.repository(&args.owner);
        let tags = target
            .all_tags()
            .into_iter()
            .map(|tag| format!("{repository}:{tag}"))
            .collect();

        let request = DockerBuild {
            context: target.context.clone(),
            dockerfile: target.dockerfile.clone(),
            build_args: build_args(),
            cache_from: registry_cache_from(&repository),
            cache_to: registry_cache_to(&repository),
            tags,
            platforms: target.platforms.clone(),
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
