use std::ffi::{OsStr, OsString};
use std::path::PathBuf;
use std::process::Command;
use std::{fs, path::Path};

use anyhow::{Context, Result, bail};

use crate::domain::Platform;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildOutput {
    Load,
    Push,
}

impl BuildOutput {
    fn argument(self) -> &'static str {
        match self {
            Self::Load => "--load",
            Self::Push => "--push",
        }
    }
}

#[derive(Debug, Clone)]
pub struct DockerBuild {
    pub context: PathBuf,
    pub dockerfile: PathBuf,
    pub build_args: Vec<(String, String)>,
    pub cache_from: Vec<String>,
    pub cache_to: Vec<String>,
    pub metadata_file: Option<PathBuf>,
    pub tags: Vec<String>,
    pub platforms: Vec<Platform>,
    pub attestations: bool,
    pub output: BuildOutput,
}

#[derive(Debug, Clone)]
pub struct DockerManifest {
    pub annotations: Vec<(String, String)>,
    pub tags: Vec<String>,
    pub sources: Vec<String>,
}

impl DockerBuild {
    pub fn display(&self) -> String {
        display_command("docker", &self.arguments())
    }

    fn arguments(&self) -> Vec<OsString> {
        let mut parts = vec![OsString::from("buildx"), OsString::from("build")];

        parts.push(OsString::from("--file"));
        parts.push(self.dockerfile.as_os_str().to_owned());

        for (name, value) in &self.build_args {
            parts.push(OsString::from("--build-arg"));
            parts.push(OsString::from(format!("{name}={value}")));
        }

        for cache_from in &self.cache_from {
            parts.push(OsString::from("--cache-from"));
            parts.push(OsString::from(cache_from));
        }

        for cache_to in &self.cache_to {
            parts.push(OsString::from("--cache-to"));
            parts.push(OsString::from(cache_to));
        }

        if let Some(metadata_file) = &self.metadata_file {
            parts.push(OsString::from("--metadata-file"));
            parts.push(metadata_file.as_os_str().to_owned());
        }

        if !self.platforms.is_empty() {
            parts.push(OsString::from("--platform"));
            parts.push(OsString::from(
                self.platforms
                    .iter()
                    .map(|platform| platform.as_str())
                    .collect::<Vec<_>>()
                    .join(","),
            ));
        }

        if self.attestations {
            parts.push(OsString::from("--sbom=true"));
            parts.push(OsString::from("--provenance=true"));
        }

        for tag in &self.tags {
            parts.push(OsString::from("--tag"));
            parts.push(OsString::from(tag));
        }

        parts.push(OsString::from(self.output.argument()));
        parts.push(self.context.as_os_str().to_owned());
        parts
    }

    pub fn run(&self) -> Result<()> {
        if let Some(metadata_file) = &self.metadata_file {
            create_parent_dir(metadata_file)?;
        }

        let mut command = Command::new("docker");
        command.args(self.arguments());

        let status = command
            .status()
            .context("failed to invoke docker buildx build")?;

        if !status.success() {
            bail!("docker buildx build exited with status {status}");
        }

        Ok(())
    }
}

impl DockerManifest {
    pub fn display(&self) -> String {
        display_command("docker", &self.arguments())
    }

    fn arguments(&self) -> Vec<OsString> {
        let mut parts = vec![
            OsString::from("buildx"),
            OsString::from("imagetools"),
            OsString::from("create"),
        ];

        for tag in &self.tags {
            parts.push(OsString::from("--tag"));
            parts.push(OsString::from(tag));
        }

        for (name, value) in &self.annotations {
            parts.push(OsString::from("--annotation"));
            parts.push(OsString::from(format!("{name}={value}")));
        }

        parts.extend(self.sources.iter().map(OsString::from));
        parts
    }

    pub fn run(&self) -> Result<()> {
        if self.tags.is_empty() {
            bail!("docker manifest request must include at least one tag");
        }
        if self.sources.is_empty() {
            bail!("docker manifest request must include at least one source");
        }

        let mut command = Command::new("docker");
        command.args(self.arguments());

        let status = command
            .status()
            .context("failed to invoke docker buildx imagetools create")?;

        if !status.success() {
            bail!("docker buildx imagetools create exited with status {status}");
        }

        Ok(())
    }
}

fn create_parent_dir(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    Ok(())
}

fn display_command(program: &str, arguments: &[OsString]) -> String {
    std::iter::once(OsStr::new(program))
        .chain(arguments.iter().map(OsString::as_os_str))
        .map(shell_quote)
        .collect::<Vec<_>>()
        .join(" ")
}

fn shell_quote(value: &OsStr) -> String {
    let value = value.to_string_lossy();
    if value
        .bytes()
        .all(|byte| matches!(byte, b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'_' | b'-' | b'.' | b'/' | b':' | b'=' | b',' | b'@'))
    {
        return value.into_owned();
    }

    format!("'{}'", value.replace('\'', "'\\''"))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{BuildOutput, DockerBuild, DockerManifest};
    use crate::domain::Platform;

    #[test]
    fn display_includes_explicit_cache_options() {
        let request = DockerBuild {
            context: PathBuf::from("images/java/21/trixie"),
            dockerfile: PathBuf::from("images/java/21/trixie/Dockerfile"),
            build_args: vec![(
                "KEELINE_IMAGE_SOURCE".to_string(),
                "https://example.com/repo".to_string(),
            )],
            cache_from: vec![
                "type=registry,ref=ghcr.io/example/keeline-java:buildcache".to_string(),
            ],
            cache_to: vec![
                "type=registry,ref=ghcr.io/example/keeline-java:buildcache,mode=max".to_string(),
            ],
            metadata_file: Some(PathBuf::from("target/keeline-release-metadata/sample.json")),
            tags: vec!["ghcr.io/example/keeline-java:21-trixie".to_string()],
            platforms: vec![Platform::LinuxAmd64, Platform::LinuxArm64],
            attestations: true,
            output: BuildOutput::Push,
        };

        let display = request.display();
        assert!(
            display
                .contains("--cache-from type=registry,ref=ghcr.io/example/keeline-java:buildcache")
        );
        assert!(display.contains(
            "--cache-to type=registry,ref=ghcr.io/example/keeline-java:buildcache,mode=max"
        ));
        assert!(display.contains("--metadata-file target/keeline-release-metadata/sample.json"));
        assert!(display.contains("--sbom=true"));
        assert!(display.contains("--provenance=true"));
        assert!(display.contains("--push"));
        assert!(!display.contains("--load"));
    }

    #[test]
    fn manifest_display_includes_tags_and_sources() {
        let request = DockerManifest {
            annotations: vec![
                (
                    "index:org.opencontainers.image.description".to_string(),
                    "Keeline Debian 13".to_string(),
                ),
                (
                    "index:org.opencontainers.image.source".to_string(),
                    "https://github.com/example/keeline".to_string(),
                ),
                (
                    "index:org.opencontainers.image.licenses".to_string(),
                    "Apache-2.0".to_string(),
                ),
            ],
            tags: vec!["ghcr.io/example/keeline-debian:13".to_string()],
            sources: vec![
                "ghcr.io/example/keeline-debian:13-amd64".to_string(),
                "ghcr.io/example/keeline-debian:13-arm64".to_string(),
            ],
        };

        assert_eq!(
            request.display(),
            "docker buildx imagetools create --tag ghcr.io/example/keeline-debian:13 --annotation 'index:org.opencontainers.image.description=Keeline Debian 13' --annotation index:org.opencontainers.image.source=https://github.com/example/keeline --annotation index:org.opencontainers.image.licenses=Apache-2.0 ghcr.io/example/keeline-debian:13-amd64 ghcr.io/example/keeline-debian:13-arm64"
        );
    }
}
