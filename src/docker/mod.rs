use std::path::PathBuf;
use std::process::Command;

use anyhow::{Context, Result, bail};

#[derive(Debug, Clone)]
pub struct DockerBuild {
    pub context: PathBuf,
    pub dockerfile: PathBuf,
    pub build_args: Vec<(String, String)>,
    pub tags: Vec<String>,
    pub platforms: Vec<String>,
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

        if !self.platforms.is_empty() {
            parts.push("--platform".to_string());
            parts.push(self.platforms.join(","));
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

        let mut command = Command::new("docker");
        command.arg("buildx").arg("build");
        command.arg("--file").arg(&self.dockerfile);

        for (name, value) in &self.build_args {
            command.arg("--build-arg").arg(format!("{name}={value}"));
        }

        if !self.platforms.is_empty() {
            command.arg("--platform").arg(self.platforms.join(","));
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
