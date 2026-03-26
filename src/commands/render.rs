use anyhow::{Context, Result, bail};

use crate::cli::RenderArgs;
use crate::domain::{ImageCatalog, ImageTarget};
use crate::render::{self, SyncSummary};

pub fn run(catalog: &ImageCatalog, args: &RenderArgs) -> Result<()> {
    catalog.validate()?;

    let targets = selected_targets(catalog, args.image_id.as_deref())?;

    if args.stdout && targets.len() != 1 {
        bail!("`render --stdout` requires exactly one image id");
    }

    if args.check {
        for target in targets {
            render::check_target(target)?;
        }

        println!("rendered dockerfiles are in sync");
        return Ok(());
    }

    if args.stdout {
        println!("{}", render::render(targets[0])?);
        return Ok(());
    }

    let mut summary = SyncSummary::default();
    for target in targets {
        match render::sync_target(target)? {
            render::SyncStatus::Generated => summary.generated += 1,
            render::SyncStatus::Updated => summary.updated += 1,
            render::SyncStatus::Unchanged => summary.unchanged += 1,
        }
    }

    println!("{}", render_message(summary));
    Ok(())
}

fn selected_targets<'a>(
    catalog: &'a ImageCatalog,
    image_id: Option<&str>,
) -> Result<Vec<&'a ImageTarget>> {
    match image_id {
        Some(image_id) => {
            Ok(vec![catalog.target(image_id).with_context(|| {
                format!("unknown image id `{image_id}`")
            })?])
        }
        None => Ok(catalog.targets.iter().collect()),
    }
}

fn render_message(summary: SyncSummary) -> String {
    let changed = summary.generated + summary.updated;

    if changed == 0 {
        return "all dockerfiles already up to date".to_string();
    }

    let mut parts = Vec::new();

    if summary.generated > 0 {
        parts.push(format!(
            "generated {} dockerfile{}",
            summary.generated,
            plural(summary.generated)
        ));
    }

    if summary.updated > 0 {
        parts.push(format!(
            "updated {} dockerfile{}",
            summary.updated,
            plural(summary.updated)
        ));
    }

    if summary.unchanged > 0 {
        parts.push(format!(
            "{} already up to date",
            count_with_noun(summary.unchanged)
        ));
    }

    parts.join(", ")
}

fn count_with_noun(count: usize) -> String {
    format!("{count} dockerfile{}", plural(count))
}

fn plural(count: usize) -> &'static str {
    if count == 1 { "" } else { "s" }
}

#[cfg(test)]
mod tests {
    use crate::render::SyncSummary;

    use super::render_message;

    #[test]
    fn describes_when_everything_is_already_current() {
        assert_eq!(
            render_message(SyncSummary {
                generated: 0,
                updated: 0,
                unchanged: 7,
            }),
            "all dockerfiles already up to date"
        );
    }

    #[test]
    fn describes_generated_and_unchanged_files() {
        assert_eq!(
            render_message(SyncSummary {
                generated: 1,
                updated: 0,
                unchanged: 6,
            }),
            "generated 1 dockerfile, 6 dockerfiles already up to date"
        );
    }
}
