use anyhow::Result;
use clap::Parser;

mod cli;
mod commands;

use cli::{Cli, Commands};

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Lint(args) => commands::lint::run(args),
        Commands::Parse(args) => commands::parse::run(args),
        Commands::Write(args) => commands::write::run(args).await,
        Commands::Read(args) => commands::read::run(args).await,
        Commands::Mcp(args) => commands::mcp::run(args).await,
    }
}
