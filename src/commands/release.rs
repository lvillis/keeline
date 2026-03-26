use anyhow::{Context, Result};

use crate::cli::ReleaseArgs;
use crate::commands::build_args;
use crate::docker::DockerBuild;
use crate::domain::{ImageCatalog, ImageTarget};
use crate::render;

pub fn run(catalog: &ImageCatalog, args: &ReleaseArgs) -> Result<()> {
    catalog.validate()?;

    let targets: Vec<&ImageTarget> = match &args.image_id {
        Some(image_id) => vec![
            catalog
                .target(image_id)
                .with_context(|| format!("unknown image id `{image_id}`"))?,
        ],
        None => catalog.targets.iter().collect(),
    };

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
