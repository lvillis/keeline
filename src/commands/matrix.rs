use anyhow::Result;
use serde::Serialize;

use crate::cli::MatrixArgs;
use crate::domain::ImageCatalog;

#[derive(Debug, Serialize)]
struct Matrix<'a> {
    include: Vec<MatrixEntry<'a>>,
}

#[derive(Debug, Serialize)]
struct MatrixEntry<'a> {
    id: &'a str,
    family: &'a str,
    line: &'a str,
    version: &'a str,
    distro: Option<&'a str>,
    package: &'a str,
    publish: bool,
    status: &'a str,
    releasable: bool,
    context: String,
    dockerfile: String,
    base_image: &'a str,
    platforms: &'a [String],
    canonical: &'a [String],
    alias: &'a [String],
}

pub fn run(catalog: &ImageCatalog, args: &MatrixArgs) -> Result<()> {
    catalog.validate()?;

    let matrix = Matrix {
        include: catalog
            .targets
            .iter()
            .filter(|target| args.all || target.is_releasable())
            .map(|target| MatrixEntry {
                id: &target.id,
                family: &target.family,
                line: &target.line,
                version: &target.version,
                distro: target.distro.as_deref(),
                package: &target.package,
                publish: target.publish,
                status: target.status_label(),
                releasable: target.is_releasable(),
                context: target.context.display().to_string(),
                dockerfile: target.dockerfile.display().to_string(),
                base_image: &target.base_image,
                platforms: &target.platforms,
                canonical: &target.canonical_tags,
                alias: &target.alias_tags,
            })
            .collect(),
    };

    if args.pretty {
        println!("{}", serde_json::to_string_pretty(&matrix)?);
    } else {
        println!("{}", serde_json::to_string(&matrix)?);
    }

    Ok(())
}
