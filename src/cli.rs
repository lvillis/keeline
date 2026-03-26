use std::path::PathBuf;

use anyhow::Result;
use clap::{Args, Parser, Subcommand};

use crate::commands;
use crate::domain::ImageCatalog;

#[derive(Debug, Parser)]
#[command(name = "keeline")]
#[command(about = "Manage Keeline runtime image metadata and releases")]
pub struct Cli {
    #[arg(long, global = true, default_value = "images")]
    pub images_dir: PathBuf,
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    List(ListArgs),
    Matrix(MatrixArgs),
    Render(RenderArgs),
    Build(BuildArgs),
    Release(ReleaseArgs),
    Verify(VerifyArgs),
}

#[derive(Debug, Args)]
pub struct ListArgs {
    #[arg(long)]
    pub json: bool,
    #[arg(long)]
    pub owner: Option<String>,
}

#[derive(Debug, Args)]
pub struct MatrixArgs {
    #[arg(long)]
    pub pretty: bool,
    #[arg(long)]
    pub all: bool,
}

#[derive(Debug, Args)]
pub struct RenderArgs {
    pub image_id: Option<String>,
    #[arg(long)]
    pub check: bool,
    #[arg(long)]
    pub stdout: bool,
}

#[derive(Debug, Args)]
pub struct BuildArgs {
    pub image_id: String,
    #[arg(long)]
    pub owner: Option<String>,
    #[arg(long)]
    pub platform: Option<String>,
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Debug, Args)]
pub struct ReleaseArgs {
    pub image_id: Option<String>,
    #[arg(long)]
    pub owner: String,
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Debug, Args)]
pub struct VerifyArgs {}

impl Cli {
    pub fn run(self) -> Result<()> {
        let catalog = ImageCatalog::discover(&self.images_dir)?;

        match self.command {
            Command::List(args) => commands::list::run(&catalog, &args),
            Command::Matrix(args) => commands::matrix::run(&catalog, &args),
            Command::Render(args) => commands::render::run(&catalog, &args),
            Command::Build(args) => commands::build::run(&catalog, &args),
            Command::Release(args) => commands::release::run(&catalog, &args),
            Command::Verify(args) => commands::verify::run(&catalog, &args),
        }
    }
}
