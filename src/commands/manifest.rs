use anyhow::Result;

use crate::cli::ManifestArgs;
use crate::commands::{
    oci_image_licenses, oci_image_source, platform_suffix, select_release_targets, tag_with_suffix,
};
use crate::docker::DockerManifest;
use crate::domain::{ImageCatalog, ImageTarget};

pub fn run(catalog: &ImageCatalog, args: &ManifestArgs) -> Result<()> {
    catalog.validate()?;

    let targets = select_release_targets(catalog, args.image_id.as_deref())?;

    let image_source = oci_image_source();
    let image_licenses = oci_image_licenses();

    for target in targets {
        let repository = target.repository(&args.owner);
        let platform_suffixes = target
            .platforms
            .iter()
            .copied()
            .map(platform_suffix)
            .collect::<Vec<_>>();

        for tag in target.all_tags() {
            let sources = platform_suffixes
                .iter()
                .map(|suffix| format!("{repository}:{}", tag_with_suffix(&tag, suffix)))
                .collect();
            let request = DockerManifest {
                annotations: oci_index_annotations(target, &image_source, &image_licenses),
                tags: vec![format!("{repository}:{tag}")],
                sources,
            };

            if args.dry_run {
                println!("{}", request.display());
                continue;
            }

            request.run()?;
        }
    }

    Ok(())
}

fn oci_index_annotations(
    target: &ImageTarget,
    image_source: &str,
    image_licenses: &str,
) -> Vec<(String, String)> {
    vec![
        (
            "index:org.opencontainers.image.description".to_string(),
            target.description.clone(),
        ),
        (
            "index:org.opencontainers.image.source".to_string(),
            image_source.to_string(),
        ),
        (
            "index:org.opencontainers.image.licenses".to_string(),
            image_licenses.to_string(),
        ),
    ]
}
