use std::collections::BTreeSet;
use std::time::Duration;

use anyhow::{Context, Result, bail, ensure};
use serde::Deserialize;

const GHCR_ORIGIN: &str = "https://ghcr.io";
const MAX_TAG_PAGES: usize = 100;
const OCI_ACCEPT: &str = concat!(
    "application/vnd.oci.image.index.v1+json, ",
    "application/vnd.docker.distribution.manifest.list.v2+json, ",
    "application/vnd.oci.image.manifest.v1+json, ",
    "application/vnd.docker.distribution.manifest.v2+json"
);

pub(super) trait Registry {
    fn tags(&self, repository: &str) -> Result<Vec<String>>;
    fn manifest_digest(&self, repository: &str, tag: &str) -> Result<String>;
}

pub(super) struct GhcrRegistry {
    agent: ureq::Agent,
}

impl GhcrRegistry {
    pub(super) fn new() -> Self {
        let agent = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(30)))
            .http_status_as_error(false)
            .https_only(true)
            .build()
            .new_agent();

        Self { agent }
    }

    fn token(&self, repository: &str) -> Result<String> {
        let scope = format!("repository:{repository}:pull");
        let url = format!(
            "{GHCR_ORIGIN}/token?service=ghcr.io&scope={}",
            percent_encode(&scope)
        );
        let mut response = self
            .agent
            .get(&url)
            .header("User-Agent", user_agent())
            .call()
            .with_context(|| format!("failed to request ghcr.io token for {repository}"))?;

        ensure!(
            response.status().is_success(),
            "failed to request ghcr.io token for {repository}: HTTP {}",
            response.status().as_u16()
        );

        let body = response.body_mut().read_to_string()?;
        let token: TokenResponse = serde_json::from_str(&body)
            .with_context(|| format!("failed to parse ghcr.io token for {repository}"))?;

        token
            .token
            .or(token.access_token)
            .context("ghcr.io token response did not include a token")
    }
}

impl Registry for GhcrRegistry {
    fn tags(&self, repository: &str) -> Result<Vec<String>> {
        let token = self.token(repository)?;
        let mut url = format!("{GHCR_ORIGIN}/v2/{repository}/tags/list?n=100");
        let mut visited = BTreeSet::new();
        let mut tags = Vec::new();

        for _ in 0..MAX_TAG_PAGES {
            ensure!(
                visited.insert(url.clone()),
                "ghcr.io tag pagination loop detected for repository `{repository}`"
            );

            let mut response = self
                .agent
                .get(&url)
                .header("Authorization", format!("Bearer {token}"))
                .header("User-Agent", user_agent())
                .call()
                .with_context(|| format!("failed to fetch tags for ghcr.io/{repository}"))?;

            ensure!(
                response.status().is_success(),
                "failed to fetch tags for ghcr.io/{repository}: HTTP {}",
                response.status().as_u16()
            );

            let next = response
                .headers()
                .get("link")
                .map(|value| {
                    let header = value
                        .to_str()
                        .context("ghcr.io returned a non-UTF-8 pagination link")?;
                    next_link(header)
                })
                .transpose()?
                .flatten();
            let body = response.body_mut().read_to_string()?;
            let page: TagsResponse = serde_json::from_str(&body)
                .with_context(|| format!("failed to parse tag list for ghcr.io/{repository}"))?;
            tags.extend(page.tags.unwrap_or_default());

            match next {
                Some(next) => url = registry_url(&next)?,
                None => {
                    tags.sort();
                    tags.dedup();
                    return Ok(tags);
                }
            }
        }

        bail!("ghcr.io tag pagination exceeded {MAX_TAG_PAGES} pages for repository `{repository}`")
    }

    fn manifest_digest(&self, repository: &str, tag: &str) -> Result<String> {
        let token = self.token(repository)?;
        let url = format!("{GHCR_ORIGIN}/v2/{repository}/manifests/{tag}");
        let response = self
            .agent
            .head(&url)
            .header("Authorization", format!("Bearer {token}"))
            .header("Accept", OCI_ACCEPT)
            .header("User-Agent", user_agent())
            .call()
            .with_context(|| {
                format!("failed to fetch manifest digest for ghcr.io/{repository}:{tag}")
            })?;

        ensure!(
            response.status().is_success(),
            "failed to fetch manifest digest for ghcr.io/{repository}:{tag}: HTTP {}",
            response.status().as_u16()
        );

        response
            .headers()
            .get("docker-content-digest")
            .and_then(|value| value.to_str().ok())
            .map(str::to_string)
            .with_context(|| {
                format!("manifest for ghcr.io/{repository}:{tag} did not include a digest")
            })
    }
}

#[derive(Debug, Deserialize)]
struct TagsResponse {
    tags: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    token: Option<String>,
    access_token: Option<String>,
}

fn user_agent() -> String {
    format!("keeline/{}", env!("CARGO_PKG_VERSION"))
}

fn next_link(header: &str) -> Result<Option<String>> {
    for link in header.split(',') {
        let link = link.trim();
        let Some((reference, params)) = link.split_once(';') else {
            continue;
        };
        if !params
            .split(';')
            .any(|param| param.trim() == r#"rel="next""#)
        {
            continue;
        }

        let reference = reference.trim();
        let link = reference.strip_circumfix("<", ">").with_context(|| {
            format!("ghcr.io returned a malformed next pagination link: `{reference}`")
        })?;
        return Ok(Some(link.to_string()));
    }

    Ok(None)
}

fn registry_url(link: &str) -> Result<String> {
    if let Some(path) = link.strip_prefix(GHCR_ORIGIN) {
        ensure!(
            path.starts_with('/') && !path.starts_with("//"),
            "ghcr.io pagination link has an invalid path: `{link}`"
        );
        return Ok(format!("{GHCR_ORIGIN}{path}"));
    }

    ensure!(
        link.starts_with('/') && !link.starts_with("//"),
        "ghcr.io pagination link must remain on {GHCR_ORIGIN}: `{link}`"
    );
    Ok(format!("{GHCR_ORIGIN}{link}"))
}

fn percent_encode(value: &str) -> String {
    let mut encoded = String::new();

    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                encoded.push(byte as char)
            }
            _ => encoded.push_str(&format!("%{byte:02X}")),
        }
    }

    encoded
}

#[cfg(test)]
mod tests {
    use super::{next_link, percent_encode, registry_url};

    #[test]
    fn parses_same_origin_next_links() {
        let header = r#"</v2/lvillis/tino/tags/list?n=100&last=0.1.1>; rel="next""#;

        assert_eq!(
            next_link(header).unwrap().as_deref(),
            Some("/v2/lvillis/tino/tags/list?n=100&last=0.1.1")
        );
        assert_eq!(
            registry_url("/v2/lvillis/tino/tags/list?n=100").unwrap(),
            "https://ghcr.io/v2/lvillis/tino/tags/list?n=100"
        );
        assert_eq!(
            registry_url("https://ghcr.io/v2/lvillis/tino/tags/list?n=100").unwrap(),
            "https://ghcr.io/v2/lvillis/tino/tags/list?n=100"
        );
    }

    #[test]
    fn rejects_cross_origin_and_protocol_relative_next_links() {
        assert!(registry_url("https://example.com/v2/tags/list").is_err());
        assert!(registry_url("//example.com/v2/tags/list").is_err());
        assert!(registry_url("https://ghcr.io.example.com/v2/tags/list").is_err());
    }

    #[test]
    fn rejects_malformed_next_link_headers() {
        assert!(next_link(r#"/v2/tags/list; rel="next""#).is_err());
    }

    #[test]
    fn percent_encodes_registry_scopes() {
        assert_eq!(
            percent_encode("repository:lvillis/motdyn:pull"),
            "repository%3Alvillis%2Fmotdyn%3Apull"
        );
    }
}
