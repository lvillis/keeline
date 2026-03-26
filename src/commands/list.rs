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

    let headers = ["ID", "GHCR", "CONTEXT"];
    let rows: Vec<[String; 3]> = catalog
        .targets
        .iter()
        .map(|target| {
            [
                target.id.clone(),
                format!("{}:{}", target.repository(&owner), target.primary_tag()),
                target.context.display().to_string(),
            ]
        })
        .collect();

    let widths = column_widths(&headers, &rows);

    println!(
        "{:<id$}  {:<ghcr$}  {}",
        headers[0],
        headers[1],
        headers[2],
        id = widths[0],
        ghcr = widths[1],
    );
    println!(
        "{:-<id$}  {:-<ghcr$}  {:-<context$}",
        "",
        "",
        "",
        id = widths[0],
        ghcr = widths[1],
        context = widths[2],
    );

    for row in rows {
        println!(
            "{:<id$}  {:<ghcr$}  {}",
            row[0],
            row[1],
            row[2],
            id = widths[0],
            ghcr = widths[1],
        );
    }

    Ok(())
}

fn column_widths(headers: &[&str; 3], rows: &[[String; 3]]) -> [usize; 3] {
    let mut widths = [headers[0].len(), headers[1].len(), headers[2].len()];

    for row in rows {
        for (index, value) in row.iter().enumerate() {
            widths[index] = widths[index].max(value.len());
        }
    }

    widths
}

#[cfg(test)]
mod tests {
    use super::column_widths;

    #[test]
    fn computes_widths_from_headers_and_rows() {
        let headers = ["ID", "GHCR", "CONTEXT"];
        let rows = vec![[
            "jdk-8u372-trixie".to_string(),
            "ghcr.io/example/keeline-jdk:8u372-trixie".to_string(),
            "images/jdk/8/trixie".to_string(),
        ]];

        assert_eq!(column_widths(&headers, &rows), [16, 40, 19]);
    }
}
