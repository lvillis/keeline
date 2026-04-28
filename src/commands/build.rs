use anyhow::{Context, Result};

use crate::cli::BuildArgs;
use crate::commands::build_args;
use crate::docker::DockerBuild;
use crate::domain::ImageCatalog;
use crate::render;

pub fn run(catalog: &ImageCatalog, args: &BuildArgs) -> Result<()> {
    catalog.validate()?;

    let target = catalog
        .target(&args.image_id)
        .with_context(|| format!("unknown image id `{}`", args.image_id))?;

    render::sync_target(target)?;

    let repository = match &args.owner {
        Some(owner) => format!("ghcr.io/{owner}/{}", target.package),
        None => format!("keeline-local/{}", target.package),
    };

    let request = DockerBuild {
        context: target.context.clone(),
        dockerfile: target.dockerfile.clone(),
        build_args: build_args(),
        cache_from: Vec::new(),
        cache_to: Vec::new(),
        metadata_file: None,
        tags: vec![format!("{repository}:{}", target.primary_tag())],
        platforms: args.platform.clone().into_iter().collect(),
        sbom: false,
        provenance: false,
        push: false,
        load: true,
    };

    if args.dry_run {
        println!("{}", request.display());
        return Ok(());
    }

    request.run()
}
