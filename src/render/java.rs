use std::fmt::Write;

use anyhow::{Context, Result};

use super::GENERATED_HEADER;
use super::common::{
    append_apt_install, append_common_labels, append_label_args, append_tool_copies,
    append_tool_stages, effective_builder_image, normalize_trim_path,
    render_archive_case_statement, required_tool_by_role, shell_quote,
};
use crate::domain::{ImageTarget, ToolRole};

pub(super) fn render(target: &ImageTarget) -> Result<String> {
    let init = required_tool_by_role(target, ToolRole::Init)?;
    let source = target
        .source
        .as_ref()
        .with_context(|| format!("java image `{}` is missing source metadata", target.id))?;
    let java = target
        .java
        .as_ref()
        .with_context(|| format!("java image `{}` is missing java metadata", target.id))?;
    let builder_image = effective_builder_image(target);
    let command = serde_json::to_string(&target.command)?;
    let trim_paths = java
        .trim_files
        .iter()
        .map(|path| normalize_trim_path(path))
        .collect::<Vec<_>>();
    let case_statement = render_archive_case_statement(&source.archives)?;

    let mut output = String::new();
    output.push_str(GENERATED_HEADER);
    append_tool_stages(&mut output, target, &builder_image)?;
    writeln!(output, "FROM {builder_image} AS source")?;
    writeln!(output)?;
    writeln!(output, "ENV JAVA_HOME={}", java.java_home)?;
    writeln!(output, "ENV JAVA_RELEASE={}", source.release)?;
    writeln!(output)?;
    append_apt_install(&mut output, &java.builder_packages, false)?;
    writeln!(output)?;
    output.push_str("RUN set -eux; \\\n");
    output.push_str("    ARCH=\"$(dpkg --print-architecture)\"; \\\n");
    output.push_str("    case \"${ARCH}\" in \\\n");
    output.push_str(&case_statement);
    output.push_str("    esac; \\\n");
    output.push_str("    wget --progress=dot:giga -O /tmp/openjdk.tar.gz \"${BINARY_URL}\"; \\\n");
    output.push_str(
        "    wget --progress=dot:giga -O /tmp/openjdk.tar.gz.sig \"${BINARY_URL}.sig\"; \\\n",
    );
    output.push_str("    export GNUPGHOME=\"$(mktemp -d)\"; \\\n");
    writeln!(
        output,
        "    gpg --batch --keyserver keyserver.ubuntu.com --recv-keys {}; \\",
        shell_quote(&source.gpg_key)
    )?;
    output.push_str("    gpg --batch --verify /tmp/openjdk.tar.gz.sig /tmp/openjdk.tar.gz; \\\n");
    output.push_str("    rm -rf \"${GNUPGHOME}\" /tmp/openjdk.tar.gz.sig; \\\n");
    output.push_str("    echo \"${ESUM} */tmp/openjdk.tar.gz\" | sha256sum -c -; \\\n");
    output.push_str("    mkdir -p \"$JAVA_HOME\"; \\\n");
    writeln!(
        output,
        "    tar --extract --file /tmp/openjdk.tar.gz --directory \"$JAVA_HOME\" --strip-components {} --no-same-owner; \\",
        source.strip_components
    )?;
    output.push_str("    rm -f /tmp/openjdk.tar.gz");
    for path in &trim_paths {
        write!(output, " {path}")?;
    }
    output.push_str(";\n\n");

    writeln!(output, "FROM {}", target.base_image)?;
    writeln!(output)?;
    writeln!(output, "ENV JAVA_HOME={}", java.java_home)?;
    output.push_str("ENV PATH=\"${JAVA_HOME}/bin:${PATH}\"\n");
    writeln!(output, "ENV LANG=\"{}\"", java.lang)?;
    writeln!(output, "ENV LANGUAGE=\"{}\"", java.language)?;
    writeln!(output, "ENV LC_ALL=\"{}\"", java.lc_all)?;
    output.push('\n');
    append_apt_install(&mut output, &java.runtime_packages, java.generate_locales)?;
    writeln!(output)?;
    writeln!(
        output,
        "COPY --from=source {} {}",
        java.java_home, java.java_home
    )?;
    append_tool_copies(&mut output, target)?;
    writeln!(output)?;
    output.push_str("RUN set -eux; \\\n");
    output.push_str(
        "    find \"$JAVA_HOME/lib\" -name '*.so' -exec dirname '{}' ';' | sort -u > /etc/ld.so.conf.d/docker-openjdk.conf; \\\n",
    );
    output.push_str("    ldconfig");
    if java.verify_commands.is_empty() {
        output.push('\n');
    } else {
        output.push_str("; \\\n");
        for (index, verify_command) in java.verify_commands.iter().enumerate() {
            output.push_str("    ");
            output.push_str(verify_command);
            if index + 1 == java.verify_commands.len() {
                output.push('\n');
            } else {
                output.push_str("; \\\n");
            }
        }
    }
    writeln!(output)?;
    append_label_args(&mut output);
    append_common_labels(&mut output, target)?;
    output.push('\n');
    writeln!(
        output,
        "ENTRYPOINT {}",
        serde_json::to_string(&init.entrypoint)?
    )?;
    writeln!(output, "CMD {command}")?;

    Ok(output)
}
