use anyhow::Result;

use crate::cli::ListArgs;
use crate::domain::ImageCatalog;
use crate::output::Table;

pub fn run(catalog: &ImageCatalog, args: &ListArgs) -> Result<()> {
    catalog.validate()?;

    if args.json {
        println!("{}", serde_json::to_string_pretty(&catalog.targets)?);
        return Ok(());
    }

    let owner = args
        .owner
        .clone()
        .or_else(|| std::env::var("GITHUB_REPOSITORY_OWNER").ok())
        .unwrap_or_else(|| "<owner>".to_string());

    let mut table = Table::new(["ID", "STATUS", "GHCR", "CONTEXT"]);

    for target in &catalog.targets {
        table.push_row([
            target.id.clone(),
            render_status(target.publish, target.status_label()),
            format!("{}:{}", target.repository(&owner), target.primary_tag()),
            target.context.display().to_string(),
        ]);
    }

    table.print();
    Ok(())
}

fn render_status(publish: bool, status: &str) -> String {
    if publish {
        status.to_string()
    } else {
        format!("{status}/hidden")
    }
}

#[cfg(test)]
mod tests {
    use super::render_status;

    #[test]
    fn renders_hidden_status_label_for_unpublished_targets() {
        assert_eq!(render_status(true, "stable"), "stable");
        assert_eq!(render_status(false, "experimental"), "experimental/hidden");
    }
}
