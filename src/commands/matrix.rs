use anyhow::Result;
use serde::Serialize;

use crate::cli::MatrixArgs;
use crate::commands::{platform_runner, platform_suffix};
use crate::domain::{ImageCatalog, ImageTarget};

#[derive(Debug, Serialize)]
struct Matrix {
    include: Vec<MatrixEntry>,
}

#[derive(Debug, Serialize)]
struct MatrixEntry {
    id: String,
    family: String,
    line: String,
    version: String,
    distro: Option<String>,
    package: String,
    publish: bool,
    status: &'static str,
    releasable: bool,
    context: String,
    dockerfile: String,
    base_image: String,
    platforms: Vec<String>,
    canonical: Vec<String>,
    alias: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    platform: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    arch: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    runner: Option<&'static str>,
}

pub fn run(catalog: &ImageCatalog, args: &MatrixArgs) -> Result<()> {
    catalog.validate()?;

    let targets: Vec<&ImageTarget> = catalog
        .targets
        .iter()
        .filter(|target| args.all || target.is_releasable())
        .collect();

    let include = if args.per_platform {
        let mut entries = Vec::new();
        for target in targets {
            for platform in &target.platforms {
                entries.push(entry(target, Some(platform))?);
            }
        }
        entries
    } else {
        targets
            .into_iter()
            .map(|target| entry(target, None))
            .collect::<Result<Vec<_>>>()?
    };

    let matrix = Matrix { include };

    if args.pretty {
        println!("{}", serde_json::to_string_pretty(&matrix)?);
    } else {
        println!("{}", serde_json::to_string(&matrix)?);
    }

    Ok(())
}

fn entry(target: &ImageTarget, platform: Option<&String>) -> Result<MatrixEntry> {
    let (platform, arch, runner) = match platform {
        Some(platform) => (
            Some(platform.clone()),
            Some(platform_suffix(platform)?),
            Some(platform_runner(platform)?),
        ),
        None => (None, None, None),
    };

    Ok(MatrixEntry {
        id: target.id.clone(),
        family: target.family.clone(),
        line: target.line.clone(),
        version: target.version.clone(),
        distro: target.distro.clone(),
        package: target.package.clone(),
        publish: target.publish,
        status: target.status_label(),
        releasable: target.is_releasable(),
        context: target.context.display().to_string(),
        dockerfile: target.dockerfile.display().to_string(),
        base_image: target.base_image.clone(),
        platforms: target.platforms.clone(),
        canonical: target.canonical_tags.clone(),
        alias: target.alias_tags.clone(),
        platform,
        arch,
        runner,
    })
}
