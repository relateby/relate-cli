use anyhow::Result;
use clap::Parser;

mod cli;
mod commands;

use cli::{Cli, Commands};

#[tokio::main]
async fn main() -> Result<()> {
    // Load .env if present — silently ignored when absent
    let _ = dotenvy::dotenv();

    let cli = Cli::parse();

    match cli.command {
        Commands::Lint(args) => {
            // lint uses exit codes 0 (clean), 1 (findings), 2 (tool error).
            // std::process::exit(1) is called inside run() for findings;
            // Err propagated here means a tool failure → exit 2.
            if let Err(e) = commands::lint::run(args) {
                eprintln!("error: {e:#}");
                std::process::exit(2);
            }
            Ok(())
        }
        Commands::Parse(args) => commands::parse::run(args),
        Commands::Query(args) => {
            // query uses exit codes 0 (success), 1 (preflight failure),
            // 2 (runtime failure). std::process::exit is called inside run();
            // Err propagated here means an internal failure → exit 2.
            if let Err(e) = commands::query::run(args, cli.neo4j).await {
                eprintln!("error: {e:#}");
                std::process::exit(2);
            }
            Ok(())
        }
        Commands::Write(args) => commands::write::run(args, cli.neo4j).await,
        Commands::Read(args) => commands::read::run(args, cli.neo4j).await,
        Commands::Mcp(args) => commands::mcp::run(args).await,
    }
}
