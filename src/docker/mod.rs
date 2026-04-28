use std::path::PathBuf;
use std::process::Command;
use std::{fs, path::Path};

use anyhow::{Context, Result, bail};

#[derive(Debug, Clone)]
pub struct DockerBuild {
    pub context: PathBuf,
    pub dockerfile: PathBuf,
    pub build_args: Vec<(String, String)>,
    pub cache_from: Vec<String>,
    pub cache_to: Vec<String>,
    pub metadata_file: Option<PathBuf>,
    pub tags: Vec<String>,
    pub platforms: Vec<String>,
    pub sbom: bool,
    pub provenance: bool,
    pub push: bool,
    pub load: bool,
}

impl DockerBuild {
    pub fn display(&self) -> String {
        let mut parts = vec![
            "docker".to_string(),
            "buildx".to_string(),
            "build".to_string(),
        ];

        parts.push("--file".to_string());
        parts.push(self.dockerfile.display().to_string());

        for (name, value) in &self.build_args {
            parts.push("--build-arg".to_string());
            parts.push(format!("{name}={value}"));
        }

        for cache_from in &self.cache_from {
            parts.push("--cache-from".to_string());
            parts.push(cache_from.clone());
        }

        for cache_to in &self.cache_to {
            parts.push("--cache-to".to_string());
            parts.push(cache_to.clone());
        }

        if let Some(metadata_file) = &self.metadata_file {
            parts.push("--metadata-file".to_string());
            parts.push(metadata_file.display().to_string());
        }

        if !self.platforms.is_empty() {
            parts.push("--platform".to_string());
            parts.push(self.platforms.join(","));
        }

        if self.sbom {
            parts.push("--sbom=true".to_string());
        }

        if self.provenance {
            parts.push("--provenance=true".to_string());
        }

        for tag in &self.tags {
            parts.push("--tag".to_string());
            parts.push(tag.clone());
        }

        if self.push {
            parts.push("--push".to_string());
        }

        if self.load {
            parts.push("--load".to_string());
        }

        parts.push(self.context.display().to_string());

        parts.join(" ")
    }

    pub fn run(&self) -> Result<()> {
        if self.push && self.load {
            bail!("docker build request cannot use both --push and --load");
        }

        if let Some(metadata_file) = &self.metadata_file {
            create_parent_dir(metadata_file)?;
        }

        let mut command = Command::new("docker");
        command.arg("buildx").arg("build");
        command.arg("--file").arg(&self.dockerfile);

        for (name, value) in &self.build_args {
            command.arg("--build-arg").arg(format!("{name}={value}"));
        }

        for cache_from in &self.cache_from {
            command.arg("--cache-from").arg(cache_from);
        }

        for cache_to in &self.cache_to {
            command.arg("--cache-to").arg(cache_to);
        }

        if let Some(metadata_file) = &self.metadata_file {
            command.arg("--metadata-file").arg(metadata_file);
        }

        if !self.platforms.is_empty() {
            command.arg("--platform").arg(self.platforms.join(","));
        }

        if self.sbom {
            command.arg("--sbom=true");
        }

        if self.provenance {
            command.arg("--provenance=true");
        }

        for tag in &self.tags {
            command.arg("--tag").arg(tag);
        }

        if self.push {
            command.arg("--push");
        }

        if self.load {
            command.arg("--load");
        }

        command.arg(&self.context);

        let status = command
            .status()
            .context("failed to invoke docker buildx build")?;

        if !status.success() {
            bail!("docker buildx build exited with status {status}");
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

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::DockerBuild;

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
            tags: vec!["ghcr.io/example/keeline-java:jdk-21-trixie".to_string()],
            platforms: vec!["linux/amd64".to_string(), "linux/arm64".to_string()],
            sbom: true,
            provenance: true,
            push: true,
            load: false,
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
    }
}
