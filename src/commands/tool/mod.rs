use anyhow::{Result, bail};

use crate::cli::{ToolArgs, ToolCommand, ToolListArgs, ToolOutdatedArgs, ToolUpdateArgs};
use crate::domain::ImageCatalog;

mod catalog;
mod model;
mod output;
mod policy;
mod registry;
mod update;

use model::{PlannedToolUpdate, ToolStatus};
use output::{plural, print_checks, render_summary, updated_file_count};
use registry::GhcrRegistry;

pub fn run(catalog: &ImageCatalog, args: &ToolArgs) -> Result<()> {
    catalog.validate()?;

    match &args.command {
        ToolCommand::List(args) => list(catalog, args),
        ToolCommand::Outdated(args) => outdated(catalog, args),
        ToolCommand::Update(args) => update(catalog, args),
    }
}

fn list(catalog: &ImageCatalog, args: &ToolListArgs) -> Result<()> {
    let declarations = catalog::declarations(catalog)?;

    if args.json {
        println!("{}", serde_json::to_string_pretty(&declarations)?);
    } else {
        output::print_declarations(declarations);
    }

    Ok(())
}

fn outdated(catalog: &ImageCatalog, args: &ToolOutdatedArgs) -> Result<()> {
    let declarations = catalog::selected(catalog, &args.names)?;
    let checks = policy::check_all(&GhcrRegistry::new(), &declarations, args.allow_major)?;

    if args.json {
        println!("{}", serde_json::to_string_pretty(&checks)?);
    } else {
        print_checks(&checks);
    }

    if args.check && checks.iter().any(model::ToolCheck::needs_attention) {
        bail!("tool updates are available");
    }

    Ok(())
}

fn update(catalog: &ImageCatalog, args: &ToolUpdateArgs) -> Result<()> {
    let declarations = catalog::selected(catalog, &args.names)?;
    let checks = policy::check_all(&GhcrRegistry::new(), &declarations, args.allow_major)?;

    if args.dry_run {
        print_checks(&checks);
        return Ok(());
    }

    let digest_changes = checks
        .iter()
        .filter(|check| check.status == ToolStatus::DigestChanged)
        .collect::<Vec<_>>();
    if !args.accept_digest_change && !digest_changes.is_empty() {
        print_checks(&checks);
        let names = digest_changes
            .iter()
            .map(|check| check.name.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        bail!(
            "mutable tag digest changed for {names}; review the upstream images and rerun with \
             --accept-digest-change to trust the new digest"
        );
    }

    let updates = checks
        .iter()
        .filter_map(PlannedToolUpdate::from_check)
        .collect::<Vec<_>>();
    if updates.is_empty() {
        print_checks(&checks);
        if checks.iter().any(model::ToolCheck::needs_attention) {
            println!("no tool updates applied; use --allow-major to include major updates");
        } else {
            println!("all tools are current");
        }
        return Ok(());
    }

    let summary = update::apply_updates(catalog, &updates)?;
    let file_count = updated_file_count(&updates);
    println!(
        "updated {} tool declaration{} across {} image definition file{}; {}",
        updates.len(),
        plural(updates.len()),
        file_count,
        plural(file_count),
        render_summary(summary)
    );

    Ok(())
}
