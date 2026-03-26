use std::process::Command;

pub mod build;
pub mod list;
pub mod matrix;
pub mod release;
pub mod render;
pub mod verify;

pub(crate) fn build_args() -> Vec<(String, String)> {
    vec![
        ("KEELINE_IMAGE_SOURCE".to_string(), image_source()),
        ("KEELINE_IMAGE_REVISION".to_string(), image_revision()),
        (
            "KEELINE_IMAGE_LICENSES".to_string(),
            "Apache-2.0".to_string(),
        ),
    ]
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
