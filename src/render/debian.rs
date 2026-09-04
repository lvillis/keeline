use std::fmt::Write;

use anyhow::Result;

use super::GENERATED_HEADER;
use super::common::{
    append_common_labels, append_label_args, append_tool_copies, append_tool_stages,
    effective_builder_image, required_tool_by_role,
};
use crate::domain::{ImageTarget, ToolRole};

pub(super) fn render(target: &ImageTarget) -> Result<String> {
    let init = required_tool_by_role(target, ToolRole::Init)?;
    let mut output = String::new();
    let command = serde_json::to_string(&target.command)?;
    let entrypoint = serde_json::to_string(&init.entrypoint)?;
    let builder_image = effective_builder_image(target);

    output.push_str(GENERATED_HEADER);
    append_tool_stages(&mut output, target, &builder_image)?;
    writeln!(output, "FROM {}", target.base_image)?;
    output.push('\n');
    append_tool_copies(&mut output, target)?;
    writeln!(output)?;
    append_label_args(&mut output);
    append_common_labels(&mut output, target)?;
    output.push('\n');
    writeln!(output, "ENTRYPOINT {entrypoint}")?;
    writeln!(output, "CMD {command}")?;

    Ok(output)
}
