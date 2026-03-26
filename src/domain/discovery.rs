use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use walkdir::WalkDir;

use crate::domain::model::{
    ImageCatalog, ImageSource, ImageTarget, JavaRuntime, RawImageDefinition, SourceArchive,
};

pub fn discover(root: &Path) -> Result<ImageCatalog> {
    let root = root.to_path_buf();
    let mut targets = Vec::new();

    for entry in WalkDir::new(&root)
        .into_iter()
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().is_file() && entry.file_name() == "image.toml")
    {
        let definition_file = entry.path().to_path_buf();
        let context_dir = definition_file
            .parent()
            .context("image.toml must have a parent directory")?;
        let contents = fs::read_to_string(&definition_file)
            .with_context(|| format!("failed to read {}", definition_file.display()))?;
        let definition: RawImageDefinition = toml::from_str(&contents)
            .with_context(|| format!("failed to parse {}", definition_file.display()))?;

        let source = definition.source.as_ref().map(|source| ImageSource {
            provider: source.provider.clone(),
            release: source.release.clone(),
            gpg_key: source.gpg_key.clone(),
            strip_components: source.strip_components,
            archives: source
                .archives
                .iter()
                .map(|archive| SourceArchive {
                    platform: archive.platform.clone(),
                    url: archive.url.clone(),
                    sha256: archive.sha256.clone(),
                })
                .collect(),
        });
        let java = definition.java.as_ref().map(|java| JavaRuntime {
            java_home: java.java_home.clone(),
            builder_packages: java.builder_packages.clone(),
            runtime_packages: java.runtime_packages.clone(),
            verify_commands: java.verify_commands.clone(),
            trim_files: java.trim_files.clone(),
        });

        for variant in definition.variants {
            let id = match variant.name.as_str() {
                "default" => definition.id.clone(),
                name => format!("{}-{name}", definition.id),
            };

            targets.push(ImageTarget {
                schema: definition.schema,
                id,
                family: definition.family.clone(),
                line: definition.line.clone(),
                version: definition.version.clone(),
                distro: definition.distro.clone(),
                package: definition.package.clone(),
                variant: variant.name,
                context: context_dir.to_path_buf(),
                dockerfile: context_dir.join(variant.dockerfile),
                platforms: definition.platforms.clone(),
                base_image: variant.base_image,
                title: variant.title,
                description: variant.description,
                command: variant.command,
                canonical_tags: variant.canonical,
                alias_tags: variant.alias,
                source: source.clone(),
                java: java.clone(),
                definition_file: definition_file.clone(),
            });
        }
    }

    targets.sort_by(|left, right| left.id.cmp(&right.id));

    Ok(ImageCatalog { root, targets })
}
