use anyhow::Result;
use clap::Parser;

mod cli;
mod commands;
mod gram_render;

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
        Commands::Render(args) => {
            if let Err(e) = commands::render::run(args) {
                eprintln!("error: {e:#}");
                // render::run handles --json error output and exits internally;
                // reaching here means non-JSON mode. Use contract exit codes:
                // 1 = parse/logic error (RenderError), 2 = I/O error.
                if e.downcast_ref::<gram_render::RenderError>().is_some() {
                    std::process::exit(1);
                } else {
                    std::process::exit(2);
                }
            }
            Ok(())
        }
        Commands::External(args) => {
            let (name, ext_args) = args
                .split_first()
                .expect("clap guarantees at least one element in External");
            commands::external::exec_extension(name, ext_args, &cli.neo4j);
        }
    }
}
