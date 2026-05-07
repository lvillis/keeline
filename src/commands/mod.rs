use std::process::Command;

use anyhow::{Result, bail};

pub mod build;
pub mod list;
pub mod manifest;
pub mod matrix;
pub mod release;
pub mod render;
pub mod tool;
pub mod verify;

pub(crate) const RELEASE_METADATA_DIR: &str = "target/keeline-release-metadata";

pub(crate) const IMAGE_LICENSES: &str = "Apache-2.0";

pub(crate) fn build_args() -> Vec<(String, String)> {
    vec![
        ("KEELINE_IMAGE_SOURCE".to_string(), image_source()),
        ("KEELINE_IMAGE_REVISION".to_string(), image_revision()),
        ("KEELINE_IMAGE_LICENSES".to_string(), image_licenses()),
    ]
}

pub(crate) fn registry_cache_from(repository: &str, cache_tag: &str) -> Vec<String> {
    vec![format!("type=registry,ref={repository}:{cache_tag}")]
}

pub(crate) fn registry_cache_to(repository: &str, cache_tag: &str) -> Vec<String> {
    vec![format!(
        "type=registry,ref={repository}:{cache_tag},mode=max"
    )]
}

pub(crate) fn platform_suffix(platform: &str) -> Result<&'static str> {
    match platform {
        "linux/amd64" => Ok("amd64"),
        "linux/arm64" => Ok("arm64"),
        _ => bail!("unsupported native release platform `{platform}`"),
    }
}

pub(crate) fn platform_runner(platform: &str) -> Result<&'static str> {
    match platform {
        "linux/amd64" => Ok("ubuntu-latest"),
        "linux/arm64" => Ok("ubuntu-24.04-arm"),
        _ => bail!("unsupported native release platform `{platform}`"),
    }
}

pub(crate) fn tag_with_suffix(tag: &str, suffix: &str) -> String {
    format!("{tag}-{suffix}")
}

pub(crate) fn oci_image_source() -> String {
    image_source()
}

pub(crate) fn oci_image_licenses() -> String {
    image_licenses()
}

fn image_source() -> String {
    std::env::var("KEELINE_IMAGE_SOURCE")
        .ok()
        .or_else(|| {
            let server = std::env::var("GITHUB_SERVER_URL").ok()?;
            let repo = std::env::var("GITHUB_REPOSITORY").ok()?;
            Some(format!("{server}/{repo}"))
        })
        .or_else(|| git_output(&["config", "--get", "remote.origin.url"]).map(normalize_git_url))
        .unwrap_or_else(|| "https://github.com/unknown/unknown".to_string())
}

fn image_revision() -> String {
    std::env::var("KEELINE_IMAGE_REVISION")
        .ok()
        .or_else(|| std::env::var("GITHUB_SHA").ok())
        .or_else(|| git_output(&["rev-parse", "HEAD"]))
        .unwrap_or_else(|| "unknown".to_string())
}

fn image_licenses() -> String {
    std::env::var("KEELINE_IMAGE_LICENSES").unwrap_or_else(|_| IMAGE_LICENSES.to_string())
}

fn git_output(args: &[&str]) -> Option<String> {
    let output = Command::new("git").args(args).output().ok()?;

    if !output.status.success() {
        return None;
    }

    let value = String::from_utf8(output.stdout).ok()?;
    let trimmed = value.trim();

    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn normalize_git_url(url: String) -> String {
    if let Some(rest) = url.strip_prefix("git@github.com:") {
        return format!("https://github.com/{}", rest.trim_end_matches(".git"));
    }

    if let Some(rest) = url.strip_prefix("https://github.com/") {
        return format!("https://github.com/{}", rest.trim_end_matches(".git"));
    }

    url.trim_end_matches(".git").to_string()
}

#[cfg(test)]
mod tests {
    use super::{platform_runner, platform_suffix, tag_with_suffix};

    #[test]
    fn maps_supported_platforms_to_release_suffixes_and_runners() {
        assert_eq!(platform_suffix("linux/amd64").unwrap(), "amd64");
        assert_eq!(platform_suffix("linux/arm64").unwrap(), "arm64");
        assert_eq!(platform_runner("linux/amd64").unwrap(), "ubuntu-latest");
        assert_eq!(platform_runner("linux/arm64").unwrap(), "ubuntu-24.04-arm");
    }

    #[test]
    fn rejects_unsupported_native_release_platforms() {
        assert!(platform_suffix("linux/arm/v7").is_err());
        assert!(platform_runner("linux/arm/v7").is_err());
    }

    #[test]
    fn appends_arch_suffix_to_release_tags() {
        assert_eq!(tag_with_suffix("13-slim", "arm64"), "13-slim-arm64");
    }
}
