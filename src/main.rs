use anyhow::Result;
use clap::Parser;

fn main() -> Result<()> {
    let cli = keeline::cli::Cli::parse();
    cli.run()
}
