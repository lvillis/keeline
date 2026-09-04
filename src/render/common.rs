use std::fmt::Write;

use anyhow::Result;

use crate::domain::{ImageTarget, Platform, SourceArchive, ToolRole, ToolRuntime};

pub(super) fn append_tool_stages(
    output: &mut String,
    target: &ImageTarget,
    builder_image: &str,
) -> Result<()> {
    for tool in &target.tools {
        append_tool_stage(output, tool, builder_image)?;
    }

    Ok(())
}

fn append_tool_stage(output: &mut String, tool: &ToolRuntime, builder_image: &str) -> Result<()> {
    if let Some(source_image) = tool.image.as_deref() {
        writeln!(output, "FROM {source_image} AS {}", tool.name)?;
        writeln!(output)?;
        return Ok(());
    }

    let case_statement = render_archive_case_statement(&tool.archives)?;
    let source_name = archive_binary_name(&tool.source_path)?;
    let target_name = archive_binary_name(&tool.target_path)?;

    writeln!(output, "FROM {builder_image} AS {}", tool.name)?;
    writeln!(output)?;
    writeln!(
        output,
        "ENV {}={}",
        tool_release_var_name(&tool.name),
        tool.release
    )?;
    writeln!(output)?;
    append_apt_install(output, &tool.install_packages, false)?;
    writeln!(output)?;
    output.push_str("RUN set -eux; \\\n");
    output.push_str("    ARCH=\"$(dpkg --print-architecture)\"; \\\n");
    output.push_str("    case \"${ARCH}\" in \\\n");
    output.push_str(&case_statement);
    output.push_str("    esac; \\\n");
    output.push_str("    wget --progress=dot:giga -O /tmp/archive.tar.gz \"${BINARY_URL}\"; \\\n");
    output.push_str("    echo \"${ESUM} */tmp/archive.tar.gz\" | sha256sum -c -; \\\n");
    output.push_str("    mkdir -p /tmp/artifact /out; \\\n");
    writeln!(
        output,
        "    tar --extract --file /tmp/archive.tar.gz --directory /tmp/artifact --strip-components {} --no-same-owner; \\",
        tool.strip_components
    )?;
    writeln!(
        output,
        "    cp /tmp/artifact/{source_name} /out/{target_name}; \\"
    )?;
    writeln!(output, "    chmod 0755 /out/{target_name}; \\")?;
    output.push_str("    rm -rf /tmp/artifact /tmp/archive.tar.gz\n\n");

    Ok(())
}

pub(super) fn append_tool_copies(output: &mut String, target: &ImageTarget) -> Result<()> {
    for tool in &target.tools {
        writeln!(
            output,
            "COPY --from={} {} {}",
            tool.name,
            tool_stage_source_path(tool)?,
            tool.target_path
        )?;
    }

    Ok(())
}

pub(super) fn append_label_args(output: &mut String) {
    output.push_str("ARG KEELINE_IMAGE_SOURCE=https://github.com/unknown/unknown\n");
    output.push_str("ARG KEELINE_IMAGE_REVISION=unknown\n");
    output.push_str("ARG KEELINE_IMAGE_LICENSES=Apache-2.0\n");
}

pub(super) fn append_common_labels(output: &mut String, target: &ImageTarget) -> Result<()> {
    writeln!(
        output,
        "LABEL org.opencontainers.image.title=\"{}\" \\",
        docker_escape(&target.title)
    )?;
    writeln!(
        output,
        "      org.opencontainers.image.description=\"{}\" \\",
        docker_escape(&target.description)
    )?;
    output.push_str("      org.opencontainers.image.source=\"${KEELINE_IMAGE_SOURCE}\" \\\n");
    output.push_str("      org.opencontainers.image.revision=\"${KEELINE_IMAGE_REVISION}\" \\\n");
    output.push_str("      org.opencontainers.image.licenses=\"${KEELINE_IMAGE_LICENSES}\" \\\n");
    writeln!(
        output,
        "      org.opencontainers.image.version=\"{}\"",
        docker_escape(&target.version)
    )?;
    Ok(())
}

pub(super) fn append_apt_install(
    output: &mut String,
    packages: &[String],
    configure_locales: bool,
) -> Result<()> {
    output.push_str("RUN set -eux; \\\n");
    output.push_str("    apt-get update; \\\n");
    output.push_str(
        "    DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \\\n",
    );
    for package in packages {
        writeln!(output, "        {package} \\")?;
    }
    output.push_str("    ; \\\n");
    if configure_locales {
        output.push_str("    echo \"en_US.UTF-8 UTF-8\" > /etc/locale.gen; \\\n");
        output.push_str("    locale-gen en_US.UTF-8; \\\n");
    }
    output.push_str("    rm -rf /var/lib/apt/lists/*\n");
    Ok(())
}

pub(super) fn render_archive_case_statement(archives: &[SourceArchive]) -> Result<String> {
    let mut output = String::new();

    for archive in archives {
        writeln!(
            output,
            "        {}) \\",
            dpkg_arch_for_platform(archive.platform)
        )?;
        writeln!(
            output,
            "            ESUM={}; \\",
            shell_quote(&archive.sha256)
        )?;
        writeln!(
            output,
            "            BINARY_URL={}; \\",
            shell_quote(&archive.url)
        )?;
        output.push_str("            ;; \\\n");
    }

    output.push_str("        *) \\\n");
    output.push_str("            echo \"Unsupported arch: ${ARCH}\"; \\\n");
    output.push_str("            exit 1; \\\n");
    output.push_str("            ;; \\\n");

    Ok(output)
}

fn dpkg_arch_for_platform(platform: Platform) -> &'static str {
    match platform {
        Platform::LinuxAmd64 => "amd64",
        Platform::LinuxArm64 => "arm64",
    }
}

fn default_builder_image(base_image: &str) -> String {
    if base_image.contains('@') {
        return base_image.to_string();
    }

    if let Some((repository, tag)) = split_image_reference(base_image)
        && matches!(
            repository,
            "debian" | "docker.io/debian" | "docker.io/library/debian"
        )
    {
        if tag.ends_with("-slim") {
            return base_image.to_string();
        }

        return format!("{repository}:{tag}-slim");
    }

    base_image.to_string()
}

pub(super) fn effective_builder_image(target: &ImageTarget) -> String {
    target
        .builder_image
        .clone()
        .unwrap_or_else(|| default_builder_image(&target.base_image))
}

pub(super) fn required_tool_by_role(target: &ImageTarget, role: ToolRole) -> Result<&ToolRuntime> {
    target.tool_by_role(role).ok_or_else(|| {
        anyhow::anyhow!(
            "image `{}` is missing {} tool metadata",
            target.id,
            role.as_str()
        )
    })
}

fn split_image_reference(reference: &str) -> Option<(&str, &str)> {
    let last_slash = reference.rfind('/').unwrap_or(0);
    let last_colon = reference.rfind(':')?;

    if last_colon < last_slash {
        return None;
    }

    Some((&reference[..last_colon], &reference[last_colon + 1..]))
}

pub(super) fn normalize_trim_path(path: &str) -> String {
    if path.starts_with('/') || path.starts_with("$JAVA_HOME/") {
        path.to_string()
    } else {
        format!("$JAVA_HOME/{path}")
    }
}

fn archive_binary_name(path: &str) -> Result<&str> {
    path.rsplit('/')
        .next()
        .filter(|name| !name.is_empty())
        .ok_or_else(|| anyhow::anyhow!("path `{path}` must end with a file name"))
}

fn tool_stage_source_path(tool: &ToolRuntime) -> Result<String> {
    if tool.image.is_some() {
        return Ok(tool.source_path.clone());
    }

    Ok(format!("/out/{}", archive_binary_name(&tool.target_path)?))
}

fn tool_release_var_name(tool_name: &str) -> String {
    let mut variable = String::with_capacity(tool_name.len() + "_RELEASE".len());
    for character in tool_name.chars() {
        if character == '-' {
            variable.push('_');
        } else {
            variable.push(character.to_ascii_uppercase());
        }
    }
    variable.push_str("_RELEASE");
    variable
}

fn docker_escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

pub(super) fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

#[cfg(test)]
mod tests {
    use super::{default_builder_image, normalize_trim_path};

    #[test]
    fn derives_slim_builder_image_from_full_debian_base() {
        assert_eq!(
            default_builder_image("docker.io/library/debian:13"),
            "docker.io/library/debian:13-slim"
        );
        assert_eq!(
            default_builder_image("docker.io/library/debian:13-slim"),
            "docker.io/library/debian:13-slim"
        );
        let pinned = format!("docker.io/library/debian:13@sha256:{}", "a".repeat(64));
        assert_eq!(default_builder_image(&pinned), pinned);
    }

    #[test]
    fn normalizes_trim_paths_under_java_home() {
        assert_eq!(normalize_trim_path("lib/src.zip"), "$JAVA_HOME/lib/src.zip");
        assert_eq!(normalize_trim_path("/tmp/example"), "/tmp/example");
    }
}
