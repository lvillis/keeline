use anyhow::Result;

use crate::cli::ListArgs;
use crate::domain::ImageCatalog;

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

    let headers = ["ID", "STATUS", "GHCR", "CONTEXT"];
    let rows: Vec<[String; 4]> = catalog
        .targets
        .iter()
        .map(|target| {
            [
                target.id.clone(),
                render_status(target.publish, target.status_label()),
                format!("{}:{}", target.repository(&owner), target.primary_tag()),
                target.context.display().to_string(),
            ]
        })
        .collect();

    let widths = column_widths(&headers, &rows);

    println!(
        "{:<id$}  {:<status$}  {:<ghcr$}  {}",
        headers[0],
        headers[1],
        headers[2],
        headers[3],
        id = widths[0],
        status = widths[1],
        ghcr = widths[2],
    );
    println!(
        "{:-<id$}  {:-<status$}  {:-<ghcr$}  {:-<context$}",
        "",
        "",
        "",
        "",
        id = widths[0],
        status = widths[1],
        ghcr = widths[2],
        context = widths[3],
    );

    for row in rows {
        println!(
            "{:<id$}  {:<status$}  {:<ghcr$}  {}",
            row[0],
            row[1],
            row[2],
            row[3],
            id = widths[0],
            status = widths[1],
            ghcr = widths[2],
        );
    }

    Ok(())
}

fn render_status(publish: bool, status: &str) -> String {
    if publish {
        status.to_string()
    } else {
        format!("{status}/hidden")
    }
}

fn column_widths(headers: &[&str; 4], rows: &[[String; 4]]) -> [usize; 4] {
    let mut widths = [
        headers[0].len(),
        headers[1].len(),
        headers[2].len(),
        headers[3].len(),
    ];

    for row in rows {
        for (index, value) in row.iter().enumerate() {
            widths[index] = widths[index].max(value.len());
        }
    }

    widths
}

#[cfg(test)]
mod tests {
    use super::{column_widths, render_status};

    #[test]
    fn computes_widths_from_headers_and_rows() {
        let headers = ["ID", "STATUS", "GHCR", "CONTEXT"];
        let rows = vec![[
            "java-jdk-8u372-trixie".to_string(),
            "stable".to_string(),
            "ghcr.io/example/keeline-java:jdk-8u372-trixie".to_string(),
            "images/java/8/trixie".to_string(),
        ]];

        assert_eq!(column_widths(&headers, &rows), [21, 6, 45, 20]);
    }

    #[test]
    fn renders_hidden_status_label_for_unpublished_targets() {
        assert_eq!(render_status(true, "stable"), "stable");
        assert_eq!(render_status(false, "experimental"), "experimental/hidden");
    }
}
