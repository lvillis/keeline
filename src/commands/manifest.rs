use anyhow::{Context, Result, ensure};

use crate::cli::ManifestArgs;
use crate::commands::{platform_suffix, tag_with_suffix};
use crate::docker::DockerManifest;
use crate::domain::{ImageCatalog, ImageTarget};

pub fn run(catalog: &ImageCatalog, args: &ManifestArgs) -> Result<()> {
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
        let repository = target.repository(&args.owner);
        let platform_suffixes = target
            .platforms
            .iter()
            .map(|platform| platform_suffix(platform))
            .collect::<Result<Vec<_>>>()?;

        for tag in target.all_tags() {
            let sources = platform_suffixes
                .iter()
                .map(|suffix| format!("{repository}:{}", tag_with_suffix(&tag, suffix)))
                .collect();
            let request = DockerManifest {
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
