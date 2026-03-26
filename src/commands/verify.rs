use anyhow::Result;

use crate::cli::VerifyArgs;
use crate::domain::ImageCatalog;
use crate::render;

pub fn run(catalog: &ImageCatalog, _args: &VerifyArgs) -> Result<()> {
    catalog.validate()?;
    render::check_catalog(catalog)?;

    println!(
        "validated {} image targets across {} packages and confirmed rendered dockerfiles",
        catalog.targets.len(),
        catalog.package_count()
    );

    Ok(())
}
