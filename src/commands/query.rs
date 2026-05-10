use crate::cli::{Neo4jArgs, QueryArgs};
use crate::commands::from_cypher_diagnostic;
use anyhow::{anyhow, Result};
use gram_diagnostics::Severity;
use neo4rs::Graph;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

// ── T007: StatementSource ─────────────────────────────────────────────────────

#[derive(Debug, Clone)]
enum StatementSource {
    Inline,
    File {
        path: PathBuf,
        line: u32,
        /// 0-based index within the file. Used for display: index 0 shows just
        /// the filename; subsequent statements also show the start line so the
        /// user can locate them in multi-statement files.
        statement_idx: usize,
    },
}

impl std::fmt::Display for StatementSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StatementSource::Inline => write!(f, "<inline>"),
            StatementSource::File {
                path,
                statement_idx: 0,
                ..
            } => write!(f, "{}", path.display()),
            StatementSource::File { path, line, .. } => {
                write!(f, "{}:{}", path.display(), line)
            }
        }
    }
}

// ── T008: StatementEntry ──────────────────────────────────────────────────────

#[derive(Debug)]
struct StatementEntry {
    source: StatementSource,
    text: String,
}

// ── T009: ParamValue / ParamMap ───────────────────────────────────────────────

#[derive(Debug)]
enum ParamValue {
    Integer(i64),
    Float(f64),
    Boolean(bool),
    String(String),
    Json(serde_json::Value),
}

type ParamMap = HashMap<String, ParamValue>;

// ── T010: QueryResult / QuerySummary ──────────────────────────────────────────

#[derive(Debug, Default)]
struct QuerySummary {
    nodes_created: u64,
    nodes_deleted: u64,
    relationships_created: u64,
    relationships_deleted: u64,
    properties_set: u64,
    labels_added: u64,
}

#[derive(Debug)]
struct QueryResult {
    source: StatementSource,
    columns: Vec<String>,
    rows: Vec<Vec<serde_json::Value>>,
    summary: QuerySummary,
    /// True when the statement was classified as a write operation.
    /// Used to display "(write completed)" vs "(no rows returned)".
    is_write: bool,
}

// ── T027: Classification ──────────────────────────────────────────────────────

enum Classification {
    Read,
    Write { first_write_kind: String },
}

// ── T011: Build queue from inline expressions ─────────────────────────────────

fn build_queue_inline(exprs: &[String]) -> Vec<StatementEntry> {
    exprs
        .iter()
        .map(|text| StatementEntry {
            source: StatementSource::Inline,
            text: text.clone(),
        })
        .collect()
}

// ── T016: Build queue from a .cypher file ─────────────────────────────────────

fn build_queue_file(path: &Path) -> Result<Vec<StatementEntry>> {
    let source = std::fs::read_to_string(path)?;

    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(&tree_sitter::Language::from(tree_sitter_cypher::LANGUAGE))
        .map_err(|e| anyhow!("failed to initialise Cypher parser: {e}"))?;

    let tree = parser
        .parse(&source, None)
        .ok_or_else(|| anyhow!("failed to parse {}", path.display()))?;

    let root = tree.root_node();
    let mut entries = Vec::new();
    let mut cursor = root.walk();

    for child in root.children(&mut cursor) {
        // Only "statement" nodes are Cypher statements.
        // "doc_comment" (cypherdoc), ";" separators, ERROR nodes, etc. are skipped.
        if child.kind() != "statement" {
            continue;
        }
        let text = source[child.byte_range()].trim().to_string();
        let line = child.start_position().row as u32 + 1; // 1-based
        let statement_idx = entries.len();
        entries.push(StatementEntry {
            source: StatementSource::File {
                path: path.to_owned(),
                line,
                statement_idx,
            },
            text,
        });
    }

    if entries.is_empty() {
        return Err(anyhow!("no statements found in {}", path.display()));
    }

    Ok(entries)
}

// ── T012: Preflight Stage 1 — Lint ───────────────────────────────────────────

fn to_byte_offset(source: &str, line: u32, character: u32) -> usize {
    let line_start: usize = source
        .split('\n')
        .take(line as usize)
        .map(|l| l.len() + 1)
        .sum();
    (line_start + character as usize).min(source.len())
}

fn preflight_lint(queue: &[StatementEntry]) {
    let mut has_errors = false;

    for entry in queue {
        let opts = cypher_data::lint::LintOptions { strict: false };
        let diags: Vec<gram_diagnostics::Diagnostic> =
            cypher_data::lint::lint_source(&entry.text, &opts)
                .into_iter()
                .map(from_cypher_diagnostic)
                .collect();

        for diag in &diags {
            if matches!(diag.severity, Severity::Error) {
                has_errors = true;
            }

            let source_name = entry.source.to_string();
            let start = &diag.range.start;
            let end = &diag.range.end;
            let start_off = to_byte_offset(&entry.text, start.line, start.character);
            let end_off = to_byte_offset(&entry.text, end.line, end.character).max(start_off + 1);

            let kind = match diag.severity {
                Severity::Error => ariadne::ReportKind::Error,
                Severity::Warning => ariadne::ReportKind::Warning,
                _ => ariadne::ReportKind::Advice,
            };

            let result = ariadne::Report::build(kind, (source_name.as_str(), start_off..end_off))
                .with_message(format!("[{}] {}", diag.rule, diag.message))
                .with_label(
                    ariadne::Label::new((source_name.as_str(), start_off..end_off))
                        .with_message(diag.message.as_str()),
                )
                .finish()
                .print((
                    source_name.as_str(),
                    ariadne::Source::from(entry.text.as_str()),
                ));

            if result.is_err() {
                eprintln!(
                    "{}:{}:{}: {} [{}] {}",
                    source_name,
                    start.line + 1,
                    start.character + 1,
                    match diag.severity {
                        Severity::Error => "error",
                        Severity::Warning => "warning",
                        _ => "info",
                    },
                    diag.rule,
                    diag.message
                );
            }
        }
    }

    if has_errors {
        std::process::exit(1);
    }
}

// ── T028: Write classification ────────────────────────────────────────────────

// Node kinds for write clauses in tree-sitter-cypher 0.2.x.
// "call_clause" is classified conservatively as Write.
const WRITE_CLAUSE_KINDS: &[&str] = &[
    "create_clause",
    "merge_clause",
    "set_clause",
    "delete_clause",
    "remove_clause",
    "foreach_clause",
    "call_clause",
];

fn find_write_clause(node: tree_sitter::Node) -> Option<String> {
    let kind = node.kind();
    if WRITE_CLAUSE_KINDS.contains(&kind) {
        return Some(kind.to_string());
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if let Some(k) = find_write_clause(child) {
            return Some(k);
        }
    }
    None
}

fn classify_statement(text: &str) -> Classification {
    let mut parser = tree_sitter::Parser::new();
    if parser
        .set_language(&tree_sitter::Language::from(tree_sitter_cypher::LANGUAGE))
        .is_err()
    {
        return Classification::Read;
    }
    let tree = match parser.parse(text, None) {
        Some(t) => t,
        None => return Classification::Read,
    };
    match find_write_clause(tree.root_node()) {
        Some(kind) => Classification::Write {
            first_write_kind: kind,
        },
        None => Classification::Read,
    }
}

// ── T029: Preflight Stage 2 — Write classification ───────────────────────────

fn clause_kind_to_keyword(kind: &str) -> &str {
    match kind {
        "create_clause" => "CREATE",
        "merge_clause" => "MERGE",
        "set_clause" => "SET",
        "delete_clause" => "DELETE",
        "remove_clause" => "REMOVE",
        "foreach_clause" => "FOREACH",
        "call_clause" => "CALL",
        other => other,
    }
}

fn preflight_write(queue: &[StatementEntry], allow_write: bool) {
    for entry in queue {
        if let Classification::Write { first_write_kind } = classify_statement(&entry.text) {
            if !allow_write {
                eprintln!("Error: write operation requires --write flag");
                eprintln!("  Statement: {}", entry.text.lines().next().unwrap_or(""));
                eprintln!("  Source: {}", entry.source);
                eprintln!("  Clause: {}", clause_kind_to_keyword(&first_write_kind));
                eprintln!();
                eprintln!("  Re-run with --write to allow mutations.");
                std::process::exit(1);
            }
        }
    }
}

// ── T020–T022: Named parameters ───────────────────────────────────────────────

fn parse_param_flag(s: &str) -> Result<(String, ParamValue)> {
    let eq = s
        .find('=')
        .ok_or_else(|| anyhow!("--param must be NAME=VALUE, got: {s:?}"))?;
    let name = s[..eq].to_string();
    let raw = &s[eq + 1..];

    let value = if let Ok(i) = raw.parse::<i64>() {
        ParamValue::Integer(i)
    } else if let Ok(f) = raw.parse::<f64>() {
        ParamValue::Float(f)
    } else if raw == "true" {
        ParamValue::Boolean(true)
    } else if raw == "false" {
        ParamValue::Boolean(false)
    } else {
        ParamValue::String(raw.to_string())
    };

    Ok((name, value))
}

fn load_params_file(path: &Path) -> Result<ParamMap> {
    let content = std::fs::read_to_string(path)?;
    let map: serde_json::Map<String, serde_json::Value> = serde_json::from_str(&content)
        .map_err(|e| anyhow!("--params file is not a valid JSON object: {e}"))?;
    Ok(map
        .into_iter()
        .map(|(k, v)| (k, ParamValue::Json(v)))
        .collect())
}

fn build_param_map(args: &QueryArgs) -> Result<ParamMap> {
    let mut params: ParamMap = if let Some(path) = &args.params {
        load_params_file(path)?
    } else {
        HashMap::new()
    };
    for s in &args.param {
        let (k, v) = parse_param_flag(s)?;
        params.insert(k, v);
    }
    Ok(params)
}

// ── T023–T024: Preflight Stage 3 — Parameter validation ──────────────────────

fn collect_param_refs_recursive(
    node: tree_sitter::Node,
    source: &[u8],
    refs: &mut HashSet<String>,
) {
    // tree-sitter-cypher represents $name as a "parameter" node
    if node.kind() == "parameter" {
        if let Ok(text) = node.utf8_text(source) {
            let name = text.trim_start_matches('$').to_string();
            if !name.is_empty() {
                refs.insert(name);
            }
        }
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_param_refs_recursive(child, source, refs);
    }
}

fn collect_param_refs(text: &str) -> HashSet<String> {
    let mut parser = tree_sitter::Parser::new();
    if parser
        .set_language(&tree_sitter::Language::from(tree_sitter_cypher::LANGUAGE))
        .is_err()
    {
        return HashSet::new();
    }
    let tree = match parser.parse(text, None) {
        Some(t) => t,
        None => return HashSet::new(),
    };
    let mut refs = HashSet::new();
    collect_param_refs_recursive(tree.root_node(), text.as_bytes(), &mut refs);
    refs
}

fn preflight_params(queue: &[StatementEntry], params: &ParamMap) {
    let mut all_refs: HashSet<String> = HashSet::new();
    let mut ref_to_source: HashMap<String, String> = HashMap::new();

    for entry in queue {
        for name in collect_param_refs(&entry.text) {
            ref_to_source
                .entry(name.clone())
                .or_insert_with(|| entry.source.to_string());
            all_refs.insert(name);
        }
    }

    let mut missing = false;
    let mut sorted_refs: Vec<&String> = all_refs.iter().collect();
    sorted_refs.sort();
    for name in sorted_refs {
        if !params.contains_key(name.as_str()) {
            let source = ref_to_source.get(name).map(String::as_str).unwrap_or("?");
            eprintln!("Error: missing required parameter '${name}'");
            eprintln!("  Source: {source}");
            eprintln!("  Hint: pass --param {name}=<value>");
            missing = true;
        }
    }
    if missing {
        std::process::exit(1);
    }

    for key in params.keys() {
        if !all_refs.contains(key.as_str()) {
            eprintln!("Warning: parameter '{key}' is not referenced in any statement");
        }
    }
}

// ── T013: Execute queue ───────────────────────────────────────────────────────

fn bind_params(mut q: neo4rs::Query, params: &ParamMap) -> neo4rs::Query {
    for (name, value) in params {
        q = match value {
            ParamValue::Integer(i) => q.param(name.as_str(), *i),
            ParamValue::Float(f) => q.param(name.as_str(), *f),
            ParamValue::Boolean(b) => q.param(name.as_str(), *b),
            ParamValue::String(s) => q.param(name.as_str(), s.as_str()),
            ParamValue::Json(v) => match v {
                serde_json::Value::String(s) => q.param(name.as_str(), s.as_str()),
                serde_json::Value::Number(n) => {
                    if let Some(i) = n.as_i64() {
                        q.param(name.as_str(), i)
                    } else if let Some(f) = n.as_f64() {
                        q.param(name.as_str(), f)
                    } else {
                        eprintln!("Warning: parameter '{name}' has an unrepresentable numeric value — skipped");
                        q
                    }
                }
                serde_json::Value::Bool(b) => q.param(name.as_str(), *b),
                serde_json::Value::Null => {
                    eprintln!("Warning: parameter '{name}' is JSON null — null parameters are not yet supported via --params and will not be bound");
                    q
                }
                serde_json::Value::Array(_) | serde_json::Value::Object(_) => {
                    eprintln!("Warning: parameter '{name}' is a JSON array or object — complex types are not yet supported via --params and will not be bound");
                    q
                }
            },
        };
    }
    q
}

async fn execute_queue(
    queue: &[StatementEntry],
    params: &ParamMap,
    neo4j: &Neo4jArgs,
) -> Result<Vec<QueryResult>> {
    let password = neo4j.require_password()?;
    let graph = match Graph::new(&neo4j.uri, &neo4j.user, password) {
        Ok(g) => g,
        Err(e) => {
            eprintln!("Error: failed to connect to Neo4j: {e}");
            std::process::exit(2);
        }
    };

    let mut results = Vec::new();

    for entry in queue {
        let is_write = matches!(
            classify_statement(&entry.text),
            Classification::Write { .. }
        );
        let q = bind_params(neo4rs::query(&entry.text), params);

        let mut stream: neo4rs::DetachedRowStream = match graph.execute(q).await {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Error: {e}");
                eprintln!("  Source: {}", entry.source);
                std::process::exit(2);
            }
        };

        // Collect column names from the first row or fall back to empty
        let mut columns: Vec<String> = Vec::new();
        let mut rows: Vec<Vec<serde_json::Value>> = Vec::new();
        let mut first = true;

        loop {
            match stream.next().await {
                Ok(Some(row)) => {
                    if first {
                        columns = row.keys().into_iter().map(|k| k.to_string()).collect();
                        first = false;
                    }
                    let vals: Vec<serde_json::Value> = columns
                        .iter()
                        .map(|col| {
                            row.get::<serde_json::Value>(col.as_str())
                                .unwrap_or(serde_json::Value::Null)
                        })
                        .collect();
                    rows.push(vals);
                }
                Ok(None) => break,
                Err(e) => {
                    eprintln!("Error: {e}");
                    eprintln!("  Source: {}", entry.source);
                    std::process::exit(2);
                }
            }
        }

        // Consume the stream to completion; counters require the
        // `unstable-result-summary` neo4rs feature which has a compilation
        // bug in 0.9.0-rc.9. Summary fields stay zeroed until that stabilises.
        let _ = stream.finish().await;

        results.push(QueryResult {
            source: entry.source.clone(),
            columns,
            rows,
            summary: QuerySummary::default(),
            is_write,
        });
    }

    Ok(results)
}

// ── T014: Human-readable table output ────────────────────────────────────────

fn format_summary(s: &QuerySummary) -> String {
    let mut parts: Vec<String> = Vec::new();
    if s.nodes_created > 0 {
        parts.push(format!("created {} node(s)", s.nodes_created));
    }
    if s.nodes_deleted > 0 {
        parts.push(format!("deleted {} node(s)", s.nodes_deleted));
    }
    if s.relationships_created > 0 {
        parts.push(format!(
            "created {} relationship(s)",
            s.relationships_created
        ));
    }
    if s.relationships_deleted > 0 {
        parts.push(format!(
            "deleted {} relationship(s)",
            s.relationships_deleted
        ));
    }
    if s.properties_set > 0 {
        parts.push(format!("set {} property(ies)", s.properties_set));
    }
    if s.labels_added > 0 {
        parts.push(format!("added {} label(s)", s.labels_added));
    }
    parts.join(", ")
}

fn print_table(result: &QueryResult) {
    use comfy_table::{presets::UTF8_FULL, ContentArrangement, Table};

    println!("-- {}", result.source);

    if result.rows.is_empty() {
        let summary_str = format_summary(&result.summary);
        if !summary_str.is_empty() {
            let mut chars = summary_str.chars();
            let cap = chars
                .next()
                .map(|c| c.to_uppercase().collect::<String>())
                .unwrap_or_default();
            println!("{}{}.", cap, chars.as_str());
        } else if result.is_write {
            println!("(write completed)");
        } else {
            println!("(no rows returned)");
        }
    } else {
        let mut table = Table::new();
        table
            .load_preset(UTF8_FULL)
            .set_content_arrangement(ContentArrangement::Dynamic)
            .set_header(result.columns.clone());

        for row in &result.rows {
            let cells: Vec<String> = row
                .iter()
                .map(|v| match v {
                    serde_json::Value::String(s) => s.clone(),
                    serde_json::Value::Null => "null".to_string(),
                    other => other.to_string(),
                })
                .collect();
            table.add_row(cells);
        }

        println!("{table}");
        let n = result.rows.len();
        println!("{n} row(s)");
    }
}

// ── T031: JSON output ─────────────────────────────────────────────────────────

#[derive(serde::Serialize)]
struct JsonSummary {
    nodes_created: u64,
    nodes_deleted: u64,
    relationships_created: u64,
    relationships_deleted: u64,
    properties_set: u64,
    labels_added: u64,
}

#[derive(serde::Serialize)]
struct JsonResult {
    source: String,
    is_write: bool,
    columns: Vec<String>,
    rows: Vec<serde_json::Value>,
    summary: JsonSummary,
}

fn print_json(results: &[QueryResult]) {
    let output: Vec<JsonResult> = results
        .iter()
        .map(|r| JsonResult {
            source: r.source.to_string(),
            is_write: r.is_write,
            columns: r.columns.clone(),
            rows: r
                .rows
                .iter()
                .map(|row| {
                    let obj: serde_json::Map<String, serde_json::Value> = r
                        .columns
                        .iter()
                        .zip(row.iter())
                        .map(|(k, v)| (k.clone(), v.clone()))
                        .collect();
                    serde_json::Value::Object(obj)
                })
                .collect(),
            summary: JsonSummary {
                nodes_created: r.summary.nodes_created,
                nodes_deleted: r.summary.nodes_deleted,
                relationships_created: r.summary.relationships_created,
                relationships_deleted: r.summary.relationships_deleted,
                properties_set: r.summary.properties_set,
                labels_added: r.summary.labels_added,
            },
        })
        .collect();
    println!(
        "{}",
        serde_json::to_string_pretty(&output).unwrap_or_else(|_| "[]".to_string())
    );
}

// ── T015: Entry point ─────────────────────────────────────────────────────────

pub async fn run(args: QueryArgs, neo4j: Neo4jArgs) -> Result<()> {
    // Mutual exclusion: [QUERY] and -e are mutually exclusive
    if args.query.is_some() && !args.expr.is_empty() {
        eprintln!("Error: [QUERY] and --expr (-e) are mutually exclusive");
        eprintln!("       provide either a file path or one or more -e flags, not both");
        std::process::exit(1);
    }

    // Build statement queue
    let queue = if !args.expr.is_empty() {
        build_queue_inline(&args.expr)
    } else if let Some(ref path) = args.query {
        match build_queue_file(path) {
            Ok(q) => q,
            Err(e) => {
                eprintln!("Error: {e}");
                std::process::exit(1);
            }
        }
    } else {
        eprintln!("Error: provide a .cypher file or use -e/--expr for an inline statement");
        eprintln!("       run 'relate query --help' for usage");
        std::process::exit(1);
    };

    // Preflight pipeline (all before opening a Bolt connection)
    preflight_lint(&queue);
    preflight_write(&queue, args.write);

    let params = match build_param_map(&args) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Error: {e}");
            std::process::exit(1);
        }
    };
    preflight_params(&queue, &params);

    // Execute
    let results = execute_queue(&queue, &params, &neo4j).await?;

    // Output
    if args.json {
        print_json(&results);
    } else {
        for result in &results {
            print_table(result);
            println!();
        }
        let total_rows: usize = results.iter().map(|r| r.rows.len()).sum();
        println!(
            "{} statement(s) executed, {} row(s) returned.",
            results.len(),
            total_rows
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_queue_inline() {
        let exprs = vec!["MATCH (n) RETURN n".to_string()];
        let queue = build_queue_inline(&exprs);
        assert_eq!(queue.len(), 1);
        assert_eq!(queue[0].text, "MATCH (n) RETURN n");
        assert!(matches!(queue[0].source, StatementSource::Inline));
    }

    #[test]
    fn test_parse_param_flag() {
        let (k, v) = parse_param_flag("name=Alice").unwrap();
        assert_eq!(k, "name");
        assert!(matches!(v, ParamValue::String(_)));

        let (k, v) = parse_param_flag("age=30").unwrap();
        assert_eq!(k, "age");
        assert!(matches!(v, ParamValue::Integer(30)));

        let (k, v) = parse_param_flag("score=3.14").unwrap();
        assert_eq!(k, "score");
        assert!(matches!(v, ParamValue::Float(_)));

        let (k, v) = parse_param_flag("active=true").unwrap();
        assert_eq!(k, "active");
        assert!(matches!(v, ParamValue::Boolean(true)));

        // = in value: only first = splits
        let (k, v) = parse_param_flag("url=http://example.com/path?q=1").unwrap();
        assert_eq!(k, "url");
        assert!(matches!(v, ParamValue::String(ref s) if s == "http://example.com/path?q=1"));

        assert!(parse_param_flag("noequalssign").is_err());
    }

    #[test]
    fn test_classify_read() {
        assert!(matches!(
            classify_statement("MATCH (n) RETURN n"),
            Classification::Read
        ));
    }

    #[test]
    fn test_classify_write() {
        assert!(matches!(
            classify_statement("CREATE (n:Person {name: 'Alice'})"),
            Classification::Write { .. }
        ));
        assert!(matches!(
            classify_statement("MERGE (n:Person {name: 'Alice'}) RETURN n"),
            Classification::Write { .. }
        ));
    }

    #[test]
    fn test_collect_param_refs() {
        let refs = collect_param_refs("MATCH (n {name: $name, age: $age}) RETURN n");
        assert!(refs.contains("name"), "expected 'name' in {refs:?}");
        assert!(refs.contains("age"), "expected 'age' in {refs:?}");
    }

    #[test]
    fn test_empty_file_error() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let err = build_queue_file(tmp.path()).unwrap_err();
        assert!(err.to_string().contains("no statements found"));
    }
}
