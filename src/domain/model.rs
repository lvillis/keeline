use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ImageStatus {
    Stable,
    Experimental,
    Disabled,
}

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
    #[serde(default = "default_publish")]
    pub publish: bool,
    #[serde(default = "default_status")]
    pub status: ImageStatus,
    pub platforms: Vec<String>,
    #[serde(default)]
    pub init: Option<RawInitRuntime>,
    #[serde(default)]
    pub healthcheck: Option<RawHealthcheckRuntime>,
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
pub struct RawInitRuntime {
    pub provider: String,
    pub release: String,
    pub binary_path: String,
    #[serde(default = "default_download_install_packages")]
    pub install_packages: Vec<String>,
    #[serde(default = "default_strip_components")]
    pub strip_components: u8,
    pub entrypoint: Vec<String>,
    #[serde(default)]
    pub archives: Vec<RawSourceArchive>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RawHealthcheckRuntime {
    pub provider: String,
    pub release: String,
    pub binary_path: String,
    #[serde(default = "default_download_install_packages")]
    pub install_packages: Vec<String>,
    #[serde(default = "default_strip_components")]
    pub strip_components: u8,
    #[serde(default)]
    pub archives: Vec<RawSourceArchive>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RawJavaRuntime {
    pub java_home: String,
    #[serde(default)]
    pub builder_packages: Vec<String>,
    #[serde(default)]
    pub runtime_packages: Vec<String>,
    #[serde(default = "default_lang")]
    pub lang: String,
    #[serde(default = "default_language")]
    pub language: String,
    #[serde(default = "default_lc_all")]
    pub lc_all: String,
    #[serde(default = "default_generate_locales")]
    pub generate_locales: bool,
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
    #[serde(default)]
    pub runtime_packages: Option<Vec<String>>,
    #[serde(default)]
    pub lang: Option<String>,
    #[serde(default)]
    pub language: Option<String>,
    #[serde(default)]
    pub lc_all: Option<String>,
    #[serde(default)]
    pub generate_locales: Option<bool>,
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
pub struct InitRuntime {
    pub provider: String,
    pub release: String,
    pub binary_path: String,
    pub install_packages: Vec<String>,
    pub strip_components: u8,
    pub entrypoint: Vec<String>,
    pub archives: Vec<SourceArchive>,
}

#[derive(Debug, Clone, Serialize)]
pub struct HealthcheckRuntime {
    pub provider: String,
    pub release: String,
    pub binary_path: String,
    pub install_packages: Vec<String>,
    pub strip_components: u8,
    pub archives: Vec<SourceArchive>,
}

#[derive(Debug, Clone, Serialize)]
pub struct JavaRuntime {
    pub java_home: String,
    pub builder_packages: Vec<String>,
    pub runtime_packages: Vec<String>,
    pub lang: String,
    pub language: String,
    pub lc_all: String,
    pub generate_locales: bool,
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
    pub publish: bool,
    pub status: ImageStatus,
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
    pub init: Option<InitRuntime>,
    pub healthcheck: Option<HealthcheckRuntime>,
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

    pub fn release_targets(&self) -> impl Iterator<Item = &ImageTarget> {
        self.targets.iter().filter(|target| target.is_releasable())
    }

    pub fn release_target_count(&self) -> usize {
        self.release_targets().count()
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

    pub fn init_archive_for_platform(&self, platform: &str) -> Option<&SourceArchive> {
        self.init
            .as_ref()?
            .archives
            .iter()
            .find(|archive| archive.platform == platform)
    }

    pub fn healthcheck_archive_for_platform(&self, platform: &str) -> Option<&SourceArchive> {
        self.healthcheck
            .as_ref()?
            .archives
            .iter()
            .find(|archive| archive.platform == platform)
    }

    pub fn is_releasable(&self) -> bool {
        self.publish && self.status == ImageStatus::Stable
    }

    pub fn status_label(&self) -> &'static str {
        self.status.as_str()
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

pub fn default_lang() -> String {
    "en_US.UTF-8".to_string()
}

pub fn default_language() -> String {
    "en_US:en".to_string()
}

pub fn default_lc_all() -> String {
    "en_US.UTF-8".to_string()
}

pub fn default_generate_locales() -> bool {
    true
}

pub fn default_download_install_packages() -> Vec<String> {
    vec!["ca-certificates".to_string(), "wget".to_string()]
}

pub fn default_publish() -> bool {
    true
}

pub fn default_status() -> ImageStatus {
    ImageStatus::Stable
}

impl ImageStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Stable => "stable",
            Self::Experimental => "experimental",
            Self::Disabled => "disabled",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ImageCatalog, ImageStatus, ImageTarget};

    fn make_target(id: &str, publish: bool, status: ImageStatus) -> ImageTarget {
        ImageTarget {
            schema: 1,
            id: id.to_string(),
            family: "jdk".to_string(),
            line: "21".to_string(),
            version: "21.0.10".to_string(),
            distro: Some("trixie".to_string()),
            package: "keeline-jdk".to_string(),
            publish,
            status,
            variant: "default".to_string(),
            context: "images/jdk/21/trixie".into(),
            dockerfile: "images/jdk/21/trixie/Dockerfile".into(),
            platforms: vec!["linux/amd64".to_string()],
            base_image: "docker.io/library/debian:13".to_string(),
            title: "Sample".to_string(),
            description: "Sample image".to_string(),
            command: vec!["jshell".to_string()],
            canonical_tags: vec!["21-trixie".to_string()],
            alias_tags: Vec::new(),
            init: None,
            healthcheck: None,
            source: None,
            java: None,
            definition_file: "images/jdk/21/trixie/image.toml".into(),
        }
    }

    #[test]
    fn release_targets_include_only_stable_published_images() {
        let catalog = ImageCatalog {
            root: "images".into(),
            targets: vec![
                make_target("stable", true, ImageStatus::Stable),
                make_target("experimental", true, ImageStatus::Experimental),
                make_target("disabled", true, ImageStatus::Disabled),
                make_target("hidden", false, ImageStatus::Stable),
            ],
        };

        let ids: Vec<&str> = catalog
            .release_targets()
            .map(|target| target.id.as_str())
            .collect();

        assert_eq!(ids, vec!["stable"]);
        assert_eq!(catalog.release_target_count(), 1);
    }
}
