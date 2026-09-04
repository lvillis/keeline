use std::collections::BTreeSet;

use super::model::{PlannedToolUpdate, ToolCheck, ToolDeclaration};
use crate::output::Table;
use crate::render;

pub(super) fn print_declarations(declarations: Vec<ToolDeclaration>) {
    let mut table = Table::new([
        "TOOL", "ROLE", "RELEASE", "IMAGE", "DIGEST", "DEFS", "TARGETS",
    ]);

    for tool in declarations {
        let image = tool
            .image_tag_reference()
            .unwrap_or_else(|| "-".to_string());
        let digest = tool
            .image_ref
            .as_ref()
            .map(|image| short_digest(&image.digest))
            .unwrap_or_else(|| "-".to_string());
        table.push_row([
            tool.name,
            tool.role.as_str().to_string(),
            tool.release,
            image,
            digest,
            tool.definition_files.len().to_string(),
            tool.target_ids.len().to_string(),
        ]);
    }

    table.print();
}

pub(super) fn print_checks(checks: &[ToolCheck]) {
    let mut table = Table::new([
        "TOOL",
        "CURRENT",
        "LATEST",
        "STATUS",
        "CURRENT DIGEST",
        "LATEST DIGEST",
    ]);

    for check in checks {
        table.push_row([
            check.name.clone(),
            check.current_release.clone(),
            check.latest_release.clone(),
            check.status.as_str().to_string(),
            short_digest(&check.current_digest),
            short_digest(&check.latest_digest),
        ]);
    }

    table.print();
}

pub(super) fn updated_file_count(updates: &[PlannedToolUpdate]) -> usize {
    updates
        .iter()
        .flat_map(|update| update.definition_files.iter())
        .collect::<BTreeSet<_>>()
        .len()
}

pub(super) fn render_summary(summary: render::SyncSummary) -> String {
    let changed = summary.generated + summary.updated;

    if changed == 0 {
        return "all dockerfiles already up to date".to_string();
    }

    format!(
        "generated {} dockerfile{}, updated {} dockerfile{}, {} unchanged",
        summary.generated,
        plural(summary.generated),
        summary.updated,
        plural(summary.updated),
        summary.unchanged
    )
}

pub(super) fn plural(count: usize) -> &'static str {
    if count == 1 { "" } else { "s" }
}

fn short_digest(digest: &str) -> String {
    let hash = digest.strip_prefix("sha256:").unwrap_or(digest);
    let prefix = hash.get(..12).unwrap_or(hash);
    format!("sha256:{prefix}")
}
