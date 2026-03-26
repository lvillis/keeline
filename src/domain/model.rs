use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize)]
pub struct RawImageDefinition {
    #[serde(default = "default_schema")]
    pub schema: u32,
    pub family: String,
    pub line: String,
    pub version: String,
    #[serde(default)]
    pub distro: Option<String>,
    pub id: String,
    pub package: String,
    pub platforms: Vec<String>,
    #[serde(default)]
    pub source: Option<RawSource>,
    #[serde(default)]
    pub java: Option<RawJavaRuntime>,
    #[serde(default)]
    pub variants: Vec<RawVariant>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RawSource {
    pub provider: String,
    pub release: String,
    pub gpg_key: String,
    #[serde(default = "default_strip_components")]
    pub strip_components: u8,
    #[serde(default)]
    pub archives: Vec<RawSourceArchive>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RawSourceArchive {
    pub platform: String,
    pub url: String,
    pub sha256: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RawJavaRuntime {
    pub java_home: String,
    #[serde(default)]
    pub builder_packages: Vec<String>,
    #[serde(default)]
    pub runtime_packages: Vec<String>,
    #[serde(default)]
    pub verify_commands: Vec<String>,
    #[serde(default)]
    pub trim_files: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RawVariant {
    #[serde(default = "default_variant_name")]
    pub name: String,
    pub dockerfile: String,
    pub base_image: String,
    pub title: String,
    pub description: String,
    #[serde(default)]
    pub command: Vec<String>,
    pub canonical: Vec<String>,
    #[serde(default)]
    pub alias: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ImageSource {
    pub provider: String,
    pub release: String,
    pub gpg_key: String,
    pub strip_components: u8,
    pub archives: Vec<SourceArchive>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SourceArchive {
    pub platform: String,
    pub url: String,
    pub sha256: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct JavaRuntime {
    pub java_home: String,
    pub builder_packages: Vec<String>,
    pub runtime_packages: Vec<String>,
    pub verify_commands: Vec<String>,
    pub trim_files: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ImageTarget {
    pub schema: u32,
    pub id: String,
    pub family: String,
    pub line: String,
    pub version: String,
    pub distro: Option<String>,
    pub package: String,
    pub variant: String,
    pub context: PathBuf,
    pub dockerfile: PathBuf,
    pub platforms: Vec<String>,
    pub base_image: String,
    pub title: String,
    pub description: String,
    pub command: Vec<String>,
    pub canonical_tags: Vec<String>,
    pub alias_tags: Vec<String>,
    pub source: Option<ImageSource>,
    pub java: Option<JavaRuntime>,
    pub definition_file: PathBuf,
}

#[derive(Debug, Clone)]
pub struct ImageCatalog {
    pub root: PathBuf,
    pub targets: Vec<ImageTarget>,
}

impl ImageCatalog {
    pub fn discover(root: impl AsRef<Path>) -> anyhow::Result<Self> {
        crate::domain::discovery::discover(root.as_ref())
    }

    pub fn validate(&self) -> anyhow::Result<()> {
        crate::domain::validate::validate(self)
    }

    pub fn package_count(&self) -> usize {
        use std::collections::BTreeSet;

        self.targets
            .iter()
            .map(|target| target.package.clone())
            .collect::<BTreeSet<_>>()
            .len()
    }

    pub fn target(&self, id: &str) -> Option<&ImageTarget> {
        self.targets.iter().find(|target| target.id == id)
    }
}

impl ImageTarget {
    pub fn all_tags(&self) -> Vec<String> {
        self.canonical_tags
            .iter()
            .chain(self.alias_tags.iter())
            .cloned()
            .collect()
    }

    pub fn primary_tag(&self) -> &str {
        &self.canonical_tags[0]
    }

    pub fn repository(&self, owner: &str) -> String {
        format!("ghcr.io/{owner}/{}", self.package)
    }

    pub fn source_archive_for_platform(&self, platform: &str) -> Option<&SourceArchive> {
        self.source
            .as_ref()?
            .archives
            .iter()
            .find(|archive| archive.platform == platform)
    }
}

pub fn default_variant_name() -> String {
    "default".to_string()
}

pub fn default_schema() -> u32 {
    1
}

pub fn default_strip_components() -> u8 {
    1
}
