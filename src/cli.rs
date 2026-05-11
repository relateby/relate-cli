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
                  relate query -e 'MATCH (n) RETURN count(n) AS total'\n  \
                  relate query find_person.cypher --param name=Alice\n  \
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
    /// Execute a Cypher query against Neo4j
    Query(QueryArgs),
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

#[derive(Debug, clap::Args)]
#[command(
    long_about = "Execute a parameterized Cypher statement against Neo4j.\n\n\
                  Statements are linted before execution. Lint is syntactic — \
                  runtime errors (unknown labels, constraint violations) can still \
                  occur after lint passes.\n\n\
                  Write operations (CREATE, MERGE, SET, DELETE, REMOVE, FOREACH) \
                  require --write.\n\n\
                  QUERY resolution:\n  \
                  A bare query name (no path separator, no .cypher extension) is\n  \
                  resolved against the query library directory (default: ./cypher/).\n  \
                  Use 'file/stmt' to target a named statement within a multi-statement\n  \
                  file (e.g. relate query person/upsert).\n\n\
                  DISCOVERY:\n  \
                  --list with no [QUERY] lists all named statements in the library.\n  \
                  --list [QUERY] lists statements in one file. Pair with --json for\n  \
                  scripting.\n\n\
                  BATCH EXECUTION:\n  \
                  --apply <FILE> runs the query once per row of a .csv, .json, or .jsonl\n  \
                  data file. Headers/keys map to query parameters by name. Mutually\n  \
                  exclusive with [PARAMS]. Use --param to inject constants across rows.\n  \
                  Transaction mode: default commits every 1000 rows; --batch N tunes the\n  \
                  interval; --atomic wraps all rows in one transaction (full rollback on\n  \
                  any error). Progress streams to stderr; --json emits one result object\n  \
                  per row with an added 'row' index field.",
    after_help = "Examples:\n  \
                  relate query -e 'MATCH (n:Person) RETURN n.name AS name'\n  \
                  relate query find_person '{name: \"Alice\"}'\n  \
                  relate query create_person '{name: \"Alice\", age: 30}' --write\n  \
                  relate query person/upsert '{name: \"Alice\"}' --write\n  \
                  relate query --list\n  \
                  relate query --list movies\n  \
                  relate query --list --json\n  \
                  relate query --describe person\n  \
                  relate query find_person.cypher --param name=Alice\n  \
                  relate query create_person --apply people.csv --write\n  \
                  relate query create_person --apply people.jsonl --batch 500 --write\n  \
                  relate query create_person --apply people.json --atomic --write\n  \
                  relate query -e 'MATCH (n) RETURN count(n)' --json"
)]
pub struct QueryArgs {
    /// .cypher file path, bare query name, or file/stmt address (mutually exclusive with -e)
    pub query: Option<String>,

    /// Cypher map literal of named parameters e.g. '{name: "Alice", age: 30}'.
    /// Merged with --param flags; --param takes precedence on conflicts.
    /// Mutually exclusive with --params.
    pub params_map: Option<String>,

    /// Inline Cypher statement (repeatable; mutually exclusive with [QUERY])
    #[arg(short = 'e', long = "expr")]
    pub expr: Vec<String>,

    /// Named query parameter NAME=VALUE (repeatable)
    #[arg(short = 'p', long = "param")]
    pub param: Vec<String>,

    /// JSON file of named parameters (--param takes precedence on conflicts)
    #[arg(long)]
    pub params: Option<PathBuf>,

    /// Allow write operations (CREATE, MERGE, SET, DELETE, REMOVE, FOREACH)
    #[arg(long)]
    pub write: bool,

    /// List named statements in the query library (or in [QUERY] if given)
    #[arg(long)]
    pub list: bool,

    /// Print cypherdoc documentation for the query without executing
    #[arg(long)]
    pub describe: bool,

    /// Query library directory for bare-name resolution
    #[arg(long, default_value = "./cypher/")]
    pub cypher_dir: PathBuf,

    /// Apply the query once per row in a .csv, .json, or .jsonl file (mutually exclusive with [PARAMS])
    #[arg(long)]
    pub apply: Option<PathBuf>,

    /// Rows per transaction for --apply (default: 1000; mutually exclusive with --atomic; requires --apply)
    #[arg(long)]
    pub batch: Option<usize>,

    /// Wrap all --apply iterations in a single transaction (mutually exclusive with --batch; requires --apply)
    #[arg(long)]
    pub atomic: bool,

    /// Output results as JSON
    #[arg(long)]
    pub json: bool,
}
