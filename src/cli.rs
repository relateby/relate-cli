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
                  relate lint -e 'MATCH (n) RETURN n'\n  \
                  relate parse schema.gram\n  \
                  relate write nodes.gram\n  \
                  relate mcp ./queries/"
)]
pub struct Cli {
    #[command(flatten)]
    pub neo4j: Neo4jArgs,

    #[command(subcommand)]
    pub command: Commands,
}

/// Neo4j connection arguments, available globally across all subcommands.
/// Validation (requiring --password) is performed only by commands that need it.
#[derive(Debug, Clone, clap::Args)]
pub struct Neo4jArgs {
    /// Neo4j Bolt URI
    #[arg(long, global = true, default_value = "bolt://localhost:7687")]
    pub uri: String,

    /// Neo4j username
    #[arg(long, global = true, default_value = "neo4j")]
    pub user: String,

    /// Neo4j password (also read from NEO4J_PASSWORD env var)
    #[arg(long, global = true, env = "NEO4J_PASSWORD")]
    pub password: Option<String>,
}

impl Neo4jArgs {
    /// Returns the password or an error — called by commands that require Neo4j (write, read).
    #[allow(dead_code)]
    pub fn require_password(&self) -> anyhow::Result<&str> {
        self.password
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("--password or NEO4J_PASSWORD is required"))
    }
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

/// Language selection for --expr and stdin input.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum Lang {
    /// Cypher query language
    Cypher,
    /// Gram graph notation
    Gram,
}

#[derive(Debug, clap::Args)]
pub struct LintArgs {
    /// Files or directories to lint (.cypher or .gram); reads stdin if omitted
    pub files: Vec<PathBuf>,

    /// Lint an inline expression instead of a file
    #[arg(short = 'e', long = "expr")]
    pub expr: Option<String>,

    /// Language for --expr or stdin
    #[arg(long, value_enum, default_value_t = Lang::Cypher)]
    pub lang: Lang,

    /// Output results as JSON
    #[arg(long)]
    pub json: bool,

    /// Treat warnings as errors
    #[arg(long)]
    pub strict: bool,
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
    /// .gram files to write
    #[arg(required = true)]
    pub files: Vec<PathBuf>,
}

#[derive(Debug, clap::Args)]
pub struct ReadArgs {
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
