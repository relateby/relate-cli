use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(
    name = "relate",
    version,
    about = "CLI for working with .cypher and .gram files and Neo4j",
    long_about = "relate — work with Cypher and Gram files, and connect to Neo4j.\n\n\
                  Examples:\n  \
                  relate lint my-query.cypher\n  \
                  relate parse schema.gram\n  \
                  relate write nodes.gram\n  \
                  relate mcp ./queries/"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Lint .cypher or .gram files and snippets
    Lint(LintArgs),
    /// Parse a file and display its syntax tree
    Parse(ParseArgs),
    /// Write .gram files to Neo4j
    Write(WriteArgs),
    /// Read Neo4j results and save as .gram
    Read(ReadArgs),
    /// Host a directory of parameterized .cypher files as MCP tools (stdio)
    Mcp(McpArgs),
}

#[derive(Debug, clap::Args)]
pub struct LintArgs {
    /// Files to lint (.cypher or .gram)
    #[arg(required = true)]
    pub files: Vec<PathBuf>,
}

#[derive(Debug, clap::Args)]
pub struct ParseArgs {
    /// Output format
    #[arg(long, value_enum, default_value_t = ParseFormat::Sexp)]
    pub format: ParseFormat,

    /// Files to parse (.cypher or .gram)
    #[arg(required = true)]
    pub files: Vec<PathBuf>,
}

#[derive(Debug, Clone, ValueEnum)]
pub enum ParseFormat {
    /// S-expression tree (tree-sitter default)
    Sexp,
    /// JSON representation
    Json,
}

#[derive(Debug, clap::Args)]
pub struct WriteArgs {
    /// Neo4j Bolt URI
    #[arg(long, default_value = "bolt://localhost:7687")]
    pub uri: String,

    /// Neo4j username
    #[arg(long, default_value = "neo4j")]
    pub user: String,

    /// Neo4j password
    #[arg(long, env = "NEO4J_PASSWORD")]
    pub password: String,

    /// .gram files to write
    #[arg(required = true)]
    pub files: Vec<PathBuf>,
}

#[derive(Debug, clap::Args)]
pub struct ReadArgs {
    /// Neo4j Bolt URI
    #[arg(long, default_value = "bolt://localhost:7687")]
    pub uri: String,

    /// Neo4j username
    #[arg(long, default_value = "neo4j")]
    pub user: String,

    /// Neo4j password
    #[arg(long, env = "NEO4J_PASSWORD")]
    pub password: String,

    /// Cypher query to execute
    pub query: String,

    /// Output file (default: stdout)
    #[arg(long)]
    pub output: Option<PathBuf>,
}

#[derive(Debug, clap::Args)]
pub struct McpArgs {
    /// Directory containing parameterized .cypher files
    #[arg(default_value = ".")]
    pub dir: PathBuf,
}
