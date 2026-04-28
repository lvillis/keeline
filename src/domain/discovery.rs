use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use walkdir::WalkDir;

use crate::domain::model::{
    ImageCatalog, ImageSource, ImageTarget, JavaRuntime, RawImageDefinition, SourceArchive,
    ToolRuntime,
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
        let mut tools: Vec<ToolRuntime> = definition
            .tools
            .iter()
            .map(|(name, tool)| ToolRuntime {
                name: name.clone(),
                role: tool.role,
                release: tool.release.clone(),
                image: tool.image.clone(),
                source_path: tool.source_path.clone(),
                target_path: tool.target_path.clone(),
                install_packages: tool.install_packages.clone(),
                strip_components: tool.strip_components,
                entrypoint: tool.entrypoint.clone(),
                archives: tool
                    .archives
                    .iter()
                    .map(|archive| SourceArchive {
                        platform: archive.platform.clone(),
                        url: archive.url.clone(),
                        sha256: archive.sha256.clone(),
                    })
                    .collect(),
            })
            .collect();
        tools.sort_by(|left, right| {
            left.role
                .sort_order()
                .cmp(&right.role.sort_order())
                .then_with(|| left.name.cmp(&right.name))
        });
        let java = definition.java.as_ref().map(|java| JavaRuntime {
            java_home: java.java_home.clone(),
            builder_packages: java.builder_packages.clone(),
            runtime_packages: java.runtime_packages.clone(),
            lang: java.lang.clone(),
            language: java.language.clone(),
            lc_all: java.lc_all.clone(),
            generate_locales: java.generate_locales,
            verify_commands: java.verify_commands.clone(),
            trim_files: java.trim_files.clone(),
        });

        for variant in definition.variants {
            let id = match variant.name.as_str() {
                "default" => definition.id.clone(),
                name => format!("{}-{name}", definition.id),
            };

            let mut variant_java = java.clone();
            if let Some(java) = &mut variant_java {
                if let Some(runtime_packages) = variant.runtime_packages.clone() {
                    java.runtime_packages = runtime_packages;
                }
                if let Some(lang) = variant.lang.clone() {
                    java.lang = lang;
                }
                if let Some(language) = variant.language.clone() {
                    java.language = language;
                }
                if let Some(lc_all) = variant.lc_all.clone() {
                    java.lc_all = lc_all;
                }
                if let Some(generate_locales) = variant.generate_locales {
                    java.generate_locales = generate_locales;
                }
            }

            targets.push(ImageTarget {
                schema: definition.schema,
                id,
                family: definition.family.clone(),
                line: definition.line.clone(),
                version: definition.version.clone(),
                distro: definition.distro.clone(),
                package: definition.package.clone(),
                publish: definition.publish,
                status: definition.status.clone(),
                variant: variant.name,
                context: context_dir.to_path_buf(),
                dockerfile: context_dir.join(variant.dockerfile),
                platforms: definition.platforms.clone(),
                base_image: variant.base_image,
                builder_image: variant.builder_image,
                title: variant.title,
                description: variant.description,
                command: variant.command,
                canonical_tags: variant.canonical,
                alias_tags: variant.alias,
                tools: tools.clone(),
                source: source.clone(),
                java: variant_java,
                definition_file: definition_file.clone(),
            });
        }
    }

    targets.sort_by(|left, right| left.id.cmp(&right.id));

    Ok(ImageCatalog { root, targets })
}
