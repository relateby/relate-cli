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
        /// 0-based index within the file; index 0 shows just the filename.
        statement_idx: usize,
    },
    /// Bare-name resolution: ./cypher/<name>.cypher
    LibraryFile {
        path: PathBuf,
    },
    /// file/stmt addressing: one named statement within a library file
    LibraryStatement {
        path: PathBuf,
        stmt_name: String,
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
            StatementSource::LibraryFile { path } => write!(f, "{}", path.display()),
            StatementSource::LibraryStatement { path, stmt_name } => {
                write!(f, "{} ({})", path.display(), stmt_name)
            }
        }
    }
}

// ── T008: StatementEntry ──────────────────────────────────────────────────────

#[derive(Debug)]
struct StatementEntry {
    source: StatementSource,
    text: String,
    doc: Option<CypherDoc>,
}

// ── CypherDoc / ParamDecl ─────────────────────────────────────────────────────

#[derive(Debug, Clone)]
struct ParamDecl {
    name: String,
    type_: String,
    required: bool,
    default: Option<String>,
    description: Option<String>,
}

#[derive(Debug, Clone)]
struct CypherDoc {
    name: String,
    description: Option<String>,
    params: Vec<ParamDecl>,
    returns_raw: Option<String>,
}

// ── T009: ParamValue / ParamMap ───────────────────────────────────────────────

#[derive(Debug, Clone)]
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
    is_write: bool,
    /// 0-based row index when this result was produced by an --apply iteration;
    /// None for single-statement (M1/M2) invocations.
    row: Option<usize>,
}

// ── T027: Classification ──────────────────────────────────────────────────────

enum Classification {
    Read,
    Write { first_write_kind: String },
}

// ── QueryName — query resolution result ──────────────────────────────────────

enum QueryName {
    /// Explicit path: contains a separator or ends with .cypher
    ExplicitPath(PathBuf),
    /// Bare name: resolves to <cypher_dir>/<name>.cypher
    BareName { name: String, resolved: PathBuf },
    /// file/stmt address: one named statement inside a library file
    StmtAddress { file: PathBuf, stmt_name: String },
}

/// Classify a [QUERY] argument into an explicit path, bare name, or file/stmt address.
/// Pure function — no I/O.
fn resolve_query_source(query: &str, cypher_dir: &Path) -> QueryName {
    let ends_cypher = query.ends_with(".cypher");

    // Check file/stmt form first: exactly one '/' with bare identifiers on both sides.
    // This must come before the general separator check so that person/upsert is
    // recognised as StmtAddress rather than ExplicitPath.
    if !ends_cypher {
        if let Some(slash_pos) = query.find('/') {
            let file_part = &query[..slash_pos];
            let stmt_part = &query[slash_pos + 1..];
            if !file_part.contains('/')
                && !stmt_part.contains('/')
                && is_bare_identifier(file_part)
                && is_bare_identifier(stmt_part)
            {
                let file = cypher_dir.join(format!("{file_part}.cypher"));
                return QueryName::StmtAddress {
                    file,
                    stmt_name: stmt_part.to_string(),
                };
            }
        }
    }

    // Explicit path: contains any path separator, starts with '.', or ends with .cypher
    let has_separator = query.contains(std::path::MAIN_SEPARATOR)
        || query.contains('/')
        || query.contains('\\')
        || query.starts_with('.');

    if has_separator || ends_cypher {
        return QueryName::ExplicitPath(PathBuf::from(query));
    }

    // Bare name
    let resolved = cypher_dir.join(format!("{query}.cypher"));
    QueryName::BareName {
        name: query.to_string(),
        resolved,
    }
}

fn is_bare_identifier(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .next()
            .map(|c| c.is_alphabetic() || c == '_')
            .unwrap_or(false)
        && s.chars().all(|c| c.is_alphanumeric() || c == '_')
}

// ── T011: Build queue from inline expressions ─────────────────────────────────

fn build_queue_inline(exprs: &[String]) -> Vec<StatementEntry> {
    exprs
        .iter()
        .map(|text| StatementEntry {
            source: StatementSource::Inline,
            text: text.clone(),
            doc: None,
        })
        .collect()
}

// ── parse_cypherdoc — parse a /** ... */ block ────────────────────────────────

fn parse_cypherdoc(raw: &str) -> Option<CypherDoc> {
    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(&tree_sitter::Language::from(
            tree_sitter_cypherdoc::LANGUAGE,
        ))
        .ok()?;

    let tree = parser.parse(raw, None)?;
    let root = tree.root_node();
    if root.has_error() || root.kind() != "document" {
        return None;
    }

    let src = raw.as_bytes();
    let mut name = String::new();
    let mut description_lines: Vec<String> = Vec::new();
    let mut params: Vec<ParamDecl> = Vec::new();
    let mut returns_raw: Option<String> = None;

    let mut cursor = root.walk();
    for child in root.named_children(&mut cursor) {
        match child.kind() {
            "name" => {
                name = child.utf8_text(src).unwrap_or("").trim().to_string();
            }
            "description" => {
                let mut dcursor = child.walk();
                for line_node in child.named_children(&mut dcursor) {
                    if let Ok(t) = line_node.utf8_text(src) {
                        description_lines.push(t.trim().to_string());
                    }
                }
            }
            "param_tag" => {
                let type_ = child
                    .child_by_field_name("type")
                    .and_then(|n| {
                        n.named_child(0)
                            .and_then(|sc| sc.child_by_field_name("name"))
                            .and_then(|nm| nm.utf8_text(src).ok())
                            .map(str::to_string)
                    })
                    .unwrap_or_default();

                let Some(param_node) = child.child_by_field_name("param") else {
                    continue;
                };
                let (param_name, required, default) = match param_node.kind() {
                    "required_param" => {
                        let nm = param_node
                            .child_by_field_name("name")
                            .and_then(|n| n.utf8_text(src).ok())
                            .unwrap_or("")
                            .trim()
                            .to_string();
                        (nm, true, None)
                    }
                    "optional_param" => {
                        let nm = param_node
                            .child_by_field_name("name")
                            .and_then(|n| n.utf8_text(src).ok())
                            .unwrap_or("")
                            .trim()
                            .to_string();
                        let def = param_node.child_by_field_name("default").and_then(|n| {
                            n.named_child(0)
                                .and_then(|d| d.utf8_text(src).ok())
                                .map(str::to_string)
                        });
                        (nm, false, def)
                    }
                    _ => continue,
                };

                // Strip the conventional leading "- " from tag_description text.
                let description = child
                    .child_by_field_name("description")
                    .and_then(|n| n.utf8_text(src).ok())
                    .map(|s| s.trim().trim_start_matches('-').trim().to_string())
                    .filter(|s| !s.is_empty());

                params.push(ParamDecl {
                    name: param_name,
                    type_,
                    required,
                    default,
                    description,
                });
            }
            "returns_tag" => {
                if let Ok(t) = child.utf8_text(src) {
                    returns_raw = Some(t.trim().to_string());
                }
            }
            _ => {}
        }
    }

    if name.is_empty() {
        return None;
    }

    let description = if description_lines.is_empty() {
        None
    } else {
        Some(description_lines.join("\n"))
    };

    Some(CypherDoc {
        name,
        description,
        params,
        returns_raw,
    })
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
    let children: Vec<tree_sitter::Node> = {
        let mut c = root.walk();
        root.children(&mut c).collect()
    };

    let mut last_doc: Option<CypherDoc> = None;

    for child in &children {
        match child.kind() {
            "doc_comment" => {
                let raw = source[child.byte_range()].to_string();
                last_doc = parse_cypherdoc(&raw);
            }
            "statement" => {
                let text = source[child.byte_range()].trim().to_string();
                let line = child.start_position().row as u32 + 1;
                let statement_idx = entries.len();
                entries.push(StatementEntry {
                    source: StatementSource::File {
                        path: path.to_owned(),
                        line,
                        statement_idx,
                    },
                    text,
                    doc: last_doc.take(),
                });
            }
            _ => {
                // Only reset last_doc for named nodes that are neither doc_comment nor statement.
                // Unnamed nodes (whitespace, punctuation) are extras in tree-sitter and should
                // not interrupt the doc_comment → statement association.
                if child.is_named() && child.kind() != ";" {
                    last_doc = None;
                }
            }
        }
    }

    if entries.is_empty() {
        return Err(anyhow!("no statements found in {}", path.display()));
    }

    Ok(entries)
}

// ── filter_by_stmt_name ───────────────────────────────────────────────────────

fn filter_by_stmt_name(
    entries: Vec<StatementEntry>,
    stmt_name: &str,
    file: &Path,
) -> Result<Vec<StatementEntry>> {
    let available: Vec<String> = entries
        .iter()
        .filter_map(|e| e.doc.as_ref().map(|d| d.name.clone()))
        .collect();

    let filtered: Vec<StatementEntry> = entries
        .into_iter()
        .filter(|e| e.doc.as_ref().map(|d| d.name == stmt_name).unwrap_or(false))
        .collect();

    if filtered.is_empty() {
        let names = if available.is_empty() {
            "(none — no cypherdoc found in file)".to_string()
        } else {
            available.join(", ")
        };
        let path = file.display();
        return Err(anyhow!(
            "statement '{stmt_name}' not found in {path}\n  Available statements: {names}\n  \
             Hint: use 'relate query --describe <file>' to see full documentation"
        ));
    }

    Ok(filtered)
}

// ── open_library_entries — resolve QueryName to StatementEntry list ──────────

fn open_library_entries(name: &QueryName, cypher_dir: &Path) -> Result<Vec<StatementEntry>> {
    match name {
        QueryName::ExplicitPath(path) => build_queue_file(path),
        QueryName::BareName {
            name: bare,
            resolved,
        } => {
            if !resolved.exists() {
                eprintln!(
                    "Error: query '{bare}' not found in {}\n  (looked for: {})",
                    cypher_dir.display(),
                    resolved.display()
                );
                std::process::exit(1);
            }
            let mut entries = build_queue_file(resolved)?;
            for entry in &mut entries {
                entry.source = StatementSource::LibraryFile {
                    path: resolved.clone(),
                };
            }
            Ok(entries)
        }
        QueryName::StmtAddress { file, stmt_name } => {
            if !file.exists() {
                let bare = file.file_stem().and_then(|s| s.to_str()).unwrap_or("?");
                eprintln!(
                    "Error: query '{bare}' not found in {}\n  (looked for: {})",
                    cypher_dir.display(),
                    file.display()
                );
                std::process::exit(1);
            }
            let entries = build_queue_file(file)?;
            let filtered = match filter_by_stmt_name(entries, stmt_name, file) {
                Ok(f) => f,
                Err(e) => {
                    eprintln!("Error: {e}");
                    std::process::exit(1);
                }
            };
            Ok(filtered
                .into_iter()
                .map(|mut e| {
                    e.source = StatementSource::LibraryStatement {
                        path: file.clone(),
                        stmt_name: stmt_name.clone(),
                    };
                    e
                })
                .collect())
        }
    }
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
    Ok((name, coerce_param_value(raw)))
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

/// Parse a Cypher map literal string like `{name: "Alice", age: 30}` into a ParamMap.
///
/// Accepts both Cypher map syntax (`{name: "Alice"}`) and JSON-style quoted keys
/// (`{"name": "Alice"}`) per spec US2.4. Cypher parse is attempted first; if no
/// map_literal is recognised, the input is parsed as a JSON object as a fallback.
fn parse_map_literal(s: &str) -> Result<ParamMap> {
    let wrapped = format!("RETURN {s}");

    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(&tree_sitter::Language::from(tree_sitter_cypher::LANGUAGE))
        .map_err(|e| anyhow!("failed to initialise Cypher parser: {e}"))?;

    let tree = parser
        .parse(&wrapped, None)
        .ok_or_else(|| anyhow!("failed to parse parameter map"))?;

    let src = wrapped.as_bytes();

    // Walk the tree to find the first map_literal node.
    fn find_map_literal<'a>(node: tree_sitter::Node<'a>) -> Option<tree_sitter::Node<'a>> {
        if node.kind() == "map_literal" {
            return Some(node);
        }
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if let Some(n) = find_map_literal(child) {
                return Some(n);
            }
        }
        None
    }

    let Some(map_node) = find_map_literal(tree.root_node()) else {
        // Cypher parse didn't yield a map (e.g. JSON-style quoted keys, which
        // tree-sitter-cypher rejects). Fall back to JSON object parsing.
        return parse_json_object_as_param_map(s).map_err(|_| {
            anyhow!(
                "invalid parameter map — expected a Cypher map literal like '{{name: \"Alice\", age: 30}}'\n  \
                 Got: {s:?}\n  Hint: use --param name=value for key=value syntax"
            )
        });
    };

    let mut result = ParamMap::new();
    let mut cursor = map_node.walk();

    // tree-sitter-cypher map entries are `property_key_value` nodes.
    // Each has two named children: identifier (key) + expression (value wrapper).
    for entry in map_node.named_children(&mut cursor) {
        if entry.kind() != "property_key_value" {
            continue;
        }

        // Key: first named child — always an identifier in Cypher map syntax
        let key_node = match entry.named_child(0) {
            Some(n) => n,
            None => continue,
        };
        let key = key_node.utf8_text(src).unwrap_or("").trim().to_string();

        // Value: second named child is an `expression` node; actual value is inside it
        let expr_node = match entry.named_child(1) {
            Some(n) => n,
            None => continue,
        };
        let val_node = expr_node.named_child(0).unwrap_or(expr_node);

        let value = coerce_map_value(val_node, src)?;
        result.insert(key, value);
    }

    Ok(result)
}

/// Parse `s` as a JSON object and convert to ParamMap, preserving scalar types
/// (Number→Integer/Float, Bool, String) and wrapping nested objects/arrays/null
/// as ParamValue::Json.
fn parse_json_object_as_param_map(s: &str) -> Result<ParamMap> {
    let obj: serde_json::Map<String, serde_json::Value> = serde_json::from_str(s)?;
    Ok(obj
        .into_iter()
        .map(|(k, v)| (k, json_to_param_value(v)))
        .collect())
}

fn json_to_param_value(v: serde_json::Value) -> ParamValue {
    match v {
        serde_json::Value::String(s) => ParamValue::String(s),
        serde_json::Value::Bool(b) => ParamValue::Boolean(b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                ParamValue::Integer(i)
            } else if let Some(f) = n.as_f64() {
                ParamValue::Float(f)
            } else {
                ParamValue::Json(serde_json::Value::Number(n))
            }
        }
        other => ParamValue::Json(other),
    }
}

fn coerce_map_value(node: tree_sitter::Node, src: &[u8]) -> Result<ParamValue> {
    let text = node.utf8_text(src).unwrap_or("").trim();
    match node.kind() {
        "integer_literal" => {
            let i: i64 = text
                .parse()
                .map_err(|_| anyhow!("cannot parse integer: {text:?}"))?;
            Ok(ParamValue::Integer(i))
        }
        "float_literal" => {
            let f: f64 = text
                .parse()
                .map_err(|_| anyhow!("cannot parse float: {text:?}"))?;
            Ok(ParamValue::Float(f))
        }
        "boolean_literal" => Ok(ParamValue::Boolean(text == "true")),
        "null_literal" => Ok(ParamValue::Json(serde_json::Value::Null)),
        "string_literal" => {
            // Strip exactly one matching pair of surrounding quotes (single or double).
            let inner = text
                .strip_prefix('"')
                .and_then(|s| s.strip_suffix('"'))
                .or_else(|| text.strip_prefix('\'').and_then(|s| s.strip_suffix('\'')))
                .unwrap_or(text);
            Ok(ParamValue::String(inner.to_string()))
        }
        "map_literal" => {
            let json = map_literal_to_json(node, src)?;
            Ok(ParamValue::Json(json))
        }
        "list_literal" => {
            let json = list_literal_to_json(node, src)?;
            Ok(ParamValue::Json(json))
        }
        _ => Ok(ParamValue::String(text.to_string())),
    }
}

fn map_literal_to_json(node: tree_sitter::Node, src: &[u8]) -> Result<serde_json::Value> {
    let mut obj = serde_json::Map::new();
    let mut cursor = node.walk();
    for entry in node.named_children(&mut cursor) {
        if entry.kind() != "property_key_value" {
            continue;
        }
        let key = entry
            .named_child(0)
            .and_then(|n| n.utf8_text(src).ok())
            .unwrap_or("")
            .trim()
            .to_string();
        let val = entry
            .named_child(1)
            .and_then(|expr| expr.named_child(0))
            .map(|n| literal_to_json(n, src))
            .unwrap_or(Ok(serde_json::Value::Null))?;
        obj.insert(key, val);
    }
    Ok(serde_json::Value::Object(obj))
}

fn list_literal_to_json(node: tree_sitter::Node, src: &[u8]) -> Result<serde_json::Value> {
    let mut arr = Vec::new();
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        let item = if child.kind() == "expression" {
            child
                .named_child(0)
                .map(|n| literal_to_json(n, src))
                .unwrap_or(Ok(serde_json::Value::Null))?
        } else {
            literal_to_json(child, src)?
        };
        arr.push(item);
    }
    Ok(serde_json::Value::Array(arr))
}

fn literal_to_json(node: tree_sitter::Node, src: &[u8]) -> Result<serde_json::Value> {
    let text = node.utf8_text(src).unwrap_or("").trim();
    match node.kind() {
        "integer_literal" => Ok(serde_json::Value::Number(
            text.parse::<i64>()
                .map(serde_json::Number::from)
                .unwrap_or(serde_json::Number::from(0)),
        )),
        "float_literal" => Ok(serde_json::Value::Number(
            text.parse::<f64>()
                .ok()
                .and_then(serde_json::Number::from_f64)
                .unwrap_or(serde_json::Number::from(0)),
        )),
        "boolean_literal" => Ok(serde_json::Value::Bool(text == "true")),
        "null_literal" => Ok(serde_json::Value::Null),
        "string_literal" => {
            let inner = text
                .strip_prefix('"')
                .and_then(|s| s.strip_suffix('"'))
                .or_else(|| text.strip_prefix('\'').and_then(|s| s.strip_suffix('\'')))
                .unwrap_or(text);
            Ok(serde_json::Value::String(inner.to_string()))
        }
        "map_literal" => map_literal_to_json(node, src),
        "list_literal" => list_literal_to_json(node, src),
        _ => Ok(serde_json::Value::String(text.to_string())),
    }
}

fn build_param_map(args: &QueryArgs) -> Result<ParamMap> {
    // [PARAMS] and --params are mutually exclusive (checked in run())
    let mut params: ParamMap = if let Some(map_str) = &args.params_map {
        parse_map_literal(map_str)?
    } else if let Some(path) = &args.params {
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

fn format_cypherdoc_hint(doc: &CypherDoc) -> String {
    let mut lines = Vec::new();
    if let Some(desc) = &doc.description {
        lines.push(desc.clone());
    }
    for p in &doc.params {
        let sig = if p.required {
            format!("@param {{{}}} {}", p.type_, p.name)
        } else {
            format!(
                "@param {{{}}} [{}={}]",
                p.type_,
                p.name,
                p.default.as_deref().unwrap_or("")
            )
        };
        let desc_part = p
            .description
            .as_deref()
            .map(|d| format!(" - {d}"))
            .unwrap_or_default();
        lines.push(format!("  {sig}{desc_part}"));
    }
    lines.join("\n")
}

fn preflight_params(queue: &[StatementEntry], params: &ParamMap) {
    let mut missing = false;
    // Accumulate all $x refs as we go; reused for the unused-param warning below.
    let mut all_refs: HashSet<String> = HashSet::new();

    for entry in queue {
        let ast_refs = collect_param_refs(&entry.text);
        all_refs.extend(ast_refs.iter().cloned());

        if let Some(doc) = &entry.doc {
            // Cypherdoc-aware: use ParamDecl to classify required vs optional.
            let declared_names: HashSet<&str> =
                doc.params.iter().map(|p| p.name.as_str()).collect();

            for decl in &doc.params {
                if decl.required && !params.contains_key(&decl.name) {
                    eprintln!("Error: missing required parameter '${}'", decl.name);
                    eprintln!("  Source: {}", entry.source);
                    eprintln!("  Hint: pass --param {}=<value>", decl.name);
                    eprintln!();
                    eprintln!("  --- Documentation ---");
                    eprintln!("  {}", format_cypherdoc_hint(doc));
                    missing = true;
                }
            }

            // Also check AST refs not mentioned in cypherdoc (treat as required).
            for name in &ast_refs {
                if !declared_names.contains(name.as_str()) && !params.contains_key(name.as_str()) {
                    eprintln!("Error: missing required parameter '${name}'");
                    eprintln!("  Source: {}", entry.source);
                    eprintln!("  Hint: pass --param {name}=<value>");
                    missing = true;
                }
            }
        } else {
            // No cypherdoc: treat all $x refs as required (Milestone 1 behaviour).
            let mut sorted_refs: Vec<&String> = ast_refs.iter().collect();
            sorted_refs.sort();
            for name in sorted_refs {
                if !params.contains_key(name.as_str()) {
                    eprintln!("Error: missing required parameter '${name}'");
                    eprintln!("  Source: {}", entry.source);
                    eprintln!("  Hint: pass --param {name}=<value>");
                    missing = true;
                }
            }
        }
    }

    if missing {
        std::process::exit(1);
    }

    // Warn about params provided but not referenced in any statement.
    for key in params.keys() {
        if !all_refs.contains(key.as_str()) {
            eprintln!("Warning: parameter '{key}' is not referenced in any statement");
        }
    }
}

// ── print_list / list_library ─────────────────────────────────────────────────

/// One-line summary of a named statement, used by --list.
struct ListEntry {
    /// Addressable name: "file/stmt" for library-wide, "stmt" for single-file.
    name: String,
    description: String,
}

fn collect_list_entries(entries: &[StatementEntry], file_stem: Option<&str>) -> Vec<ListEntry> {
    entries
        .iter()
        .map(|e| {
            let stmt_name = e
                .doc
                .as_ref()
                .map(|d| d.name.as_str())
                .unwrap_or("(unnamed)");
            let name = match file_stem {
                Some(stem) => format!("{stem}/{stmt_name}"),
                None => stmt_name.to_string(),
            };
            let description = e
                .doc
                .as_ref()
                .and_then(|d| d.description.as_deref())
                .map(|s| s.lines().next().unwrap_or("").to_string())
                .unwrap_or_default();
            ListEntry { name, description }
        })
        .collect()
}

fn print_list_entries(list: &[ListEntry], json: bool) {
    if json {
        let arr: Vec<serde_json::Value> = list
            .iter()
            .map(|e| {
                serde_json::json!({
                    "name": e.name,
                    "description": e.description
                })
            })
            .collect();
        println!(
            "{}",
            serde_json::to_string_pretty(&arr).unwrap_or_else(|_| "[]".to_string())
        );
    } else {
        let col_width = list.iter().map(|e| e.name.len()).max().unwrap_or(0) + 2;
        for entry in list {
            if entry.description.is_empty() {
                println!("{}", entry.name);
            } else {
                println!("{:<col_width$}{}", entry.name, entry.description);
            }
        }
    }
}

/// List named statements for a single resolved file (--list with [QUERY]).
fn print_list(entries: &[StatementEntry], json: bool) {
    let list = collect_list_entries(entries, None);
    print_list_entries(&list, json);
}

/// List all named statements across every .cypher file in cypher_dir (--list with no [QUERY]).
fn list_library(cypher_dir: &Path, json: bool) {
    let mut files: Vec<PathBuf> = match std::fs::read_dir(cypher_dir) {
        Ok(rd) => rd
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().map(|e| e == "cypher").unwrap_or(false))
            .collect(),
        Err(_) => {
            eprintln!(
                "Error: query library directory '{}' not found",
                cypher_dir.display()
            );
            std::process::exit(1);
        }
    };
    files.sort();

    if json {
        // Collect all entries into one flat JSON array with "file/stmt" names.
        let mut all: Vec<ListEntry> = Vec::new();
        for file in &files {
            let stem = file.file_stem().and_then(|s| s.to_str()).unwrap_or("");
            match build_queue_file(file) {
                Ok(entries) => all.extend(collect_list_entries(&entries, Some(stem))),
                Err(e) => eprintln!("Warning: skipping '{}': {e}", file.display()),
            }
        }
        print_list_entries(&all, true);
    } else {
        // Group by file, one header per file.
        for file in &files {
            let stem = file.file_stem().and_then(|s| s.to_str()).unwrap_or("");
            match build_queue_file(file) {
                Ok(entries) => {
                    println!("{stem}");
                    let list = collect_list_entries(&entries, None);
                    let col_width = list.iter().map(|e| e.name.len()).max().unwrap_or(0) + 2;
                    for entry in &list {
                        if entry.description.is_empty() {
                            println!("  {}", entry.name);
                        } else {
                            println!("  {:<col_width$}{}", entry.name, entry.description);
                        }
                    }
                    println!();
                }
                Err(e) => eprintln!("Warning: skipping '{}': {e}", file.display()),
            }
        }
    }
}

// ── print_describe ─────────────────────────────────────────────────────────────

fn print_describe(entries: &[StatementEntry]) {
    const RULER_WIDTH: usize = 78;

    for entry in entries {
        // Use "source (doc_name)" when the entry has a cypherdoc name and the
        // source doesn't already include the statement name. LibraryStatement's
        // Display already renders as "path (stmt_name)", so appending doc.name
        // would duplicate it (e.g. "movies.cypher (upsert) (upsert)").
        let label = match (&entry.doc, &entry.source) {
            // LibraryStatement's Display already includes the stmt name as "path (stmt_name)";
            // appending doc.name would duplicate it (e.g. "movies.cypher (upsert) (upsert)").
            (Some(_), StatementSource::LibraryStatement { .. }) | (None, _) => {
                entry.source.to_string()
            }
            (Some(doc), _) => format!("{} ({})", entry.source, doc.name),
        };
        let ruler_fill = RULER_WIDTH.saturating_sub(label.len() + 4);
        let ruler: String = "─".repeat(ruler_fill);
        println!("── {label} {ruler}");

        if let Some(doc) = &entry.doc {
            if let Some(desc) = &doc.description {
                println!("{desc}");
                println!();
            }
            for p in &doc.params {
                let sig = if p.required {
                    format!("@param {{{}}} {}", p.type_, p.name)
                } else {
                    format!(
                        "@param {{{}}} [{}={}]",
                        p.type_,
                        p.name,
                        p.default.as_deref().unwrap_or("")
                    )
                };
                let desc_part = p
                    .description
                    .as_deref()
                    .map(|d| format!(" - {d}"))
                    .unwrap_or_default();
                println!("{sig}{desc_part}");
            }
            if let Some(ret) = &doc.returns_raw {
                println!("{ret}");
            }
        } else {
            println!("(no documentation)");
        }

        println!();
        for line in entry.text.lines() {
            println!("  {line}");
        }
        println!();
    }
}

// ── Milestone 3: Batch Execution ─────────────────────────────────────────────
//
// `--apply <FILE>` runs the queue once per row of a CSV / JSON-array / JSONL
// file with explicit transaction batching. The new types live here:
//
//   BatchMode            — Batched(N) | Atomic (with mutex validation)
//   RowReader (trait)    — abstracts the three data file formats
//   CsvRowReader         — streaming CSV reader with header support
//   JsonArrayRowReader   — fully-parsed JSON array (small data sets only)
//   JsonlRowReader       — streaming line-by-line JSON reader
//   PeekableRowReader    — wraps the trait object; caches the first row so
//                          preflight can validate against it before connecting
//   open_row_reader      — factory dispatching on file extension
//   execute_queue_apply  — async apply-mode dispatcher (US3 fills the body)

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BatchMode {
    /// Commit every N rows (N defaults to 1000).
    Batched(usize),
    /// Wrap all rows in a single transaction.
    Atomic,
}

impl BatchMode {
    /// Resolve a `BatchMode` from CLI args, enforcing the three mutex rules:
    /// `--batch` ⊥ `--atomic`, `--batch 0` rejected, `--batch`/`--atomic`
    /// require `--apply`. Default (neither flag) is `Batched(1000)`.
    fn from_args(args: &QueryArgs) -> Result<Self> {
        if args.batch.is_some() && args.atomic {
            return Err(anyhow!("--batch and --atomic are mutually exclusive"));
        }
        if args.apply.is_none() && (args.batch.is_some() || args.atomic) {
            return Err(anyhow!("--batch/--atomic require --apply"));
        }
        if let Some(n) = args.batch {
            if n == 0 {
                return Err(anyhow!("--batch must be at least 1"));
            }
            return Ok(BatchMode::Batched(n));
        }
        if args.atomic {
            return Ok(BatchMode::Atomic);
        }
        Ok(BatchMode::Batched(1000))
    }
}

/// Coerce a CSV cell (or any raw string) into a `ParamValue` using the same
/// rules as the `--param NAME=VALUE` flag: integer / float / boolean / string.
/// Used by `CsvRowReader::next_row` so coercion is identical to the M1/M2 flag.
fn coerce_param_value(raw: &str) -> ParamValue {
    if let Ok(i) = raw.parse::<i64>() {
        ParamValue::Integer(i)
    } else if let Ok(f) = raw.parse::<f64>() {
        ParamValue::Float(f)
    } else if raw == "true" {
        ParamValue::Boolean(true)
    } else if raw == "false" {
        ParamValue::Boolean(false)
    } else {
        ParamValue::String(raw.to_string())
    }
}

/// Abstraction over CSV / JSON-array / JSONL data files for `--apply`.
trait RowReader {
    /// Yields the next row as a parameter map, or None at EOF. A returned
    /// `Err` aborts the run; the caller surfaces the row/line index.
    fn next_row(&mut self) -> Option<Result<ParamMap>>;

    /// Total row count if known up front (JSON array only). Used to render
    /// progress as `[N/M]` vs. `[N/?]`.
    fn total_hint(&self) -> Option<usize> {
        None
    }
}

struct CsvRowReader {
    headers: Vec<String>,
    reader: csv::Reader<std::fs::File>,
    row_index: usize,
}

impl CsvRowReader {
    fn open(path: &Path) -> Result<Self> {
        let file = std::fs::File::open(path)
            .map_err(|e| anyhow!("cannot open CSV file '{}': {e}", path.display()))?;
        let mut reader = csv::ReaderBuilder::new()
            .has_headers(true)
            .from_reader(file);

        let raw_headers = reader
            .headers()
            .map_err(|e| anyhow!("cannot read CSV headers from '{}': {e}", path.display()))?
            .clone();
        let headers: Vec<String> = raw_headers.iter().map(|h| h.trim().to_string()).collect();

        // Validate: every header must be non-empty.
        for (i, h) in headers.iter().enumerate() {
            if h.is_empty() {
                return Err(anyhow!(
                    "invalid CSV header in '{}'\n  Empty column at position {}\n  Hint: every column must have a non-empty header name",
                    path.display(),
                    i + 1
                ));
            }
        }
        // Validate: no duplicate header names.
        let mut seen: HashSet<&str> = HashSet::new();
        for h in &headers {
            if !seen.insert(h.as_str()) {
                return Err(anyhow!(
                    "invalid CSV header in '{}'\n  Duplicate column name '{}'\n  Hint: each column must have a unique header name",
                    path.display(),
                    h
                ));
            }
        }

        Ok(Self {
            headers,
            reader,
            row_index: 0,
        })
    }
}

impl RowReader for CsvRowReader {
    fn next_row(&mut self) -> Option<Result<ParamMap>> {
        let mut record = csv::StringRecord::new();
        match self.reader.read_record(&mut record) {
            Ok(true) => {
                self.row_index += 1;
                let mut map = ParamMap::new();
                for (i, header) in self.headers.iter().enumerate() {
                    let cell = record.get(i).unwrap_or("");
                    map.insert(header.clone(), coerce_param_value(cell));
                }
                Some(Ok(map))
            }
            Ok(false) => None,
            Err(e) => Some(Err(anyhow!(
                "CSV parse error at row {}: {e}",
                self.row_index + 1
            ))),
        }
    }
}

struct JsonArrayRowReader {
    iter: std::vec::IntoIter<serde_json::Value>,
    total: usize,
}

impl JsonArrayRowReader {
    fn open(path: &Path) -> Result<Self> {
        let file = std::fs::File::open(path)
            .map_err(|e| anyhow!("cannot open JSON file '{}': {e}", path.display()))?;
        let value: serde_json::Value = serde_json::from_reader(std::io::BufReader::new(file))
            .map_err(|e| anyhow!("cannot parse JSON file '{}': {e}", path.display()))?;
        let arr = match value {
            serde_json::Value::Array(a) => a,
            other => {
                return Err(anyhow!(
                    "--apply JSON file must be a top-level array of objects\n  Got: {} at {}\n  Hint: wrap the object in [ ... ] or use a JSONL file",
                    json_top_level_kind(&other),
                    path.display()
                ))
            }
        };
        // Validate every element is an object (FR-016).
        for (i, elem) in arr.iter().enumerate() {
            if !elem.is_object() {
                return Err(anyhow!(
                    "--apply JSON array element {} must be an object\n  Got: {} at {}",
                    i,
                    json_top_level_kind(elem),
                    path.display()
                ));
            }
        }
        let total = arr.len();
        Ok(Self {
            iter: arr.into_iter(),
            total,
        })
    }
}

fn json_top_level_kind(v: &serde_json::Value) -> &'static str {
    match v {
        serde_json::Value::Null => "null",
        serde_json::Value::Bool(_) => "boolean",
        serde_json::Value::Number(_) => "number",
        serde_json::Value::String(_) => "string",
        serde_json::Value::Array(_) => "array",
        serde_json::Value::Object(_) => "object",
    }
}

impl RowReader for JsonArrayRowReader {
    fn next_row(&mut self) -> Option<Result<ParamMap>> {
        let elem = self.iter.next()?;
        let obj = match elem {
            serde_json::Value::Object(o) => o,
            _ => {
                // Validated up-front in open(); reachable only if iter was modified.
                return Some(Err(anyhow!("JSON array element is not an object")));
            }
        };
        let map: ParamMap = obj
            .into_iter()
            .map(|(k, v)| (k, json_to_param_value(v)))
            .collect();
        Some(Ok(map))
    }

    fn total_hint(&self) -> Option<usize> {
        Some(self.total)
    }
}

struct JsonlRowReader {
    lines: std::io::Lines<std::io::BufReader<std::fs::File>>,
    line_number: usize,
}

impl JsonlRowReader {
    fn open(path: &Path) -> Result<Self> {
        let file = std::fs::File::open(path)
            .map_err(|e| anyhow!("cannot open JSONL file '{}': {e}", path.display()))?;
        use std::io::BufRead;
        let lines = std::io::BufReader::new(file).lines();
        Ok(Self {
            lines,
            line_number: 0,
        })
    }
}

impl RowReader for JsonlRowReader {
    fn next_row(&mut self) -> Option<Result<ParamMap>> {
        loop {
            let line = self.lines.next()?;
            self.line_number += 1;
            let raw = match line {
                Ok(s) => s,
                Err(e) => {
                    return Some(Err(anyhow!(
                        "I/O error reading JSONL line {}: {e}",
                        self.line_number
                    )))
                }
            };
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                continue;
            }
            let value: serde_json::Value = match serde_json::from_str(trimmed) {
                Ok(v) => v,
                Err(e) => {
                    return Some(Err(anyhow!(
                        "invalid JSON on line {} of JSONL file\n  Underlying: {e}",
                        self.line_number
                    )))
                }
            };
            let obj = match value {
                serde_json::Value::Object(o) => o,
                other => {
                    return Some(Err(anyhow!(
                        "JSONL line {} is not a JSON object (got {})",
                        self.line_number,
                        json_top_level_kind(&other)
                    )))
                }
            };
            let map: ParamMap = obj
                .into_iter()
                .map(|(k, v)| (k, json_to_param_value(v)))
                .collect();
            return Some(Ok(map));
        }
    }
}

/// Factory: open a `RowReader` by file extension.
fn open_row_reader(path: &Path) -> Result<Box<dyn RowReader>> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_ascii_lowercase());
    match ext.as_deref() {
        Some("csv") => Ok(Box::new(CsvRowReader::open(path)?)),
        Some("json") => Ok(Box::new(JsonArrayRowReader::open(path)?)),
        Some("jsonl") => Ok(Box::new(JsonlRowReader::open(path)?)),
        _ => Err(anyhow!(
            "--apply requires a .csv, .json, or .jsonl file\n  Got: {}",
            path.display()
        )),
    }
}

/// Wraps a `RowReader` and caches the first row so preflight can inspect it
/// before any Bolt connection opens. The apply loop sees the cached row as
/// iteration 0, then delegates to the inner reader.
struct PeekableRowReader {
    inner: Box<dyn RowReader>,
    first: Option<ParamMap>,
    consumed_first: bool,
    total_hint: Option<usize>,
}

impl PeekableRowReader {
    fn open(path: &Path) -> Result<Self> {
        let mut inner = open_row_reader(path)?;
        let total_hint = inner.total_hint();
        let first = match inner.next_row() {
            Some(Ok(map)) => Some(map),
            Some(Err(e)) => return Err(e),
            None => None,
        };
        Ok(Self {
            inner,
            first,
            consumed_first: false,
            total_hint,
        })
    }

    fn first_row(&self) -> Option<&ParamMap> {
        self.first.as_ref()
    }

    fn next_row(&mut self) -> Option<Result<ParamMap>> {
        if !self.consumed_first {
            self.consumed_first = true;
            if let Some(first) = self.first.take() {
                return Some(Ok(first));
            }
            return None;
        }
        self.inner.next_row()
    }

    fn total_hint(&self) -> Option<usize> {
        self.total_hint
    }
}

/// Pluralise: "1 row" vs "5 rows".
fn pluralize(n: usize, singular: &str, plural: &str) -> String {
    if n == 1 {
        format!("1 {singular}")
    } else {
        format!("{n} {plural}")
    }
}

/// Decide whether to emit progress for this row index. Throttles per Decision
/// 14's "every-row up to 100, then every total/100 (or every 100 if unknown)".
fn should_emit_progress(row_index_1based: usize, total: Option<usize>) -> bool {
    let interval = match total {
        Some(t) if t <= 100 => 1,
        Some(t) => (t / 100).max(1),
        None if row_index_1based <= 100 => 1,
        None => 100,
    };
    row_index_1based % interval == 0
}

/// Apply-mode dispatcher (Milestone 3 — US3 + US4 + US5).
/// Extract `(code, message)` from a neo4rs error. The code is `Some(...)`
/// only for `Error::Neo4j` (e.g. `Neo.ClientError.Schema.ConstraintValidationFailed`);
/// for other variants the code is `None` and `message` is the Display string.
fn neo4j_error_parts(e: &neo4rs::Error) -> (Option<String>, String) {
    match e {
        neo4rs::Error::Neo4j(err) => (Some(err.code().to_string()), err.message().to_string()),
        other => (None, other.to_string()),
    }
}

/// Result of an `--apply` run: collected per-row results plus the total row
/// count. `results` is empty when JSON output was not requested, so the caller
/// uses `row_count` for the human-readable summary.
struct ApplyOutcome {
    results: Vec<QueryResult>,
    row_count: usize,
}

async fn execute_queue_apply(
    queue: &[StatementEntry],
    constants: &ParamMap,
    mut reader: PeekableRowReader,
    mode: BatchMode,
    neo4j: &Neo4jArgs,
    json: bool,
) -> Result<ApplyOutcome> {
    let password = neo4j.require_password()?;
    let graph = match Graph::new(&neo4j.uri, &neo4j.user, password) {
        Ok(g) => g,
        Err(e) => {
            eprintln!("Error: failed to connect to Neo4j: {e}");
            std::process::exit(2);
        }
    };

    let total_hint = reader.total_hint();
    // `results` is populated only when JSON output is requested. For
    // non-JSON runs we skip the allocation to avoid O(rows × statements)
    // memory pressure on large batches.
    let mut results: Vec<QueryResult> = Vec::new();
    let mut row_count: usize = 0;
    let mut row_index: usize = 0;
    let mut rows_in_batch: usize = 0;
    let mut batches_committed: usize = 0;
    let mut total_rows_committed: usize = 0;
    let mut tx: Option<neo4rs::Txn> = None;

    loop {
        let row_params = match reader.next_row() {
            Some(Ok(map)) => map,
            Some(Err(e)) => {
                if let Some(t) = tx.take() {
                    let _ = t.rollback().await;
                }
                eprintln!("Error reading row {}: {e}", row_index + 1);
                std::process::exit(2);
            }
            None => break,
        };

        // Open a transaction if needed (per BatchMode).
        if tx.is_none() {
            match graph.start_txn().await {
                Ok(t) => tx = Some(t),
                Err(e) => {
                    eprintln!("Error: failed to start transaction: {e}");
                    std::process::exit(2);
                }
            }
        }

        // Effective param set: row-derived overlaid by constants (FR-005).
        let mut effective: ParamMap = row_params;
        for (k, v) in constants {
            effective.insert(k.clone(), v.clone());
        }

        // Run every statement in the queue for this row inside the active tx.
        // Buffer per-row results locally; only append to `results` once *all*
        // statements for the row succeed. On any per-row failure, the partial
        // row results are discarded so the JSON flush reflects only fully
        // executed rows (spec US5 #2).
        let tx_ref = tx.as_mut().expect("tx was just opened");
        let mut row_results: Vec<QueryResult> = Vec::new();
        let mut failure: Option<(StatementSource, neo4rs::Error)> = None;

        'statements: for entry in queue {
            let is_write = matches!(
                classify_statement(&entry.text),
                Classification::Write { .. }
            );
            let q = bind_params(neo4rs::query(&entry.text), &effective);

            // Use the streaming API so read queries inside --apply surface
            // their rows (M1 parity). Write queries simply yield no rows.
            let mut stream = match tx_ref.execute(q).await {
                Ok(s) => s,
                Err(e) => {
                    failure = Some((entry.source.clone(), e));
                    break 'statements;
                }
            };

            let mut columns: Vec<String> = Vec::new();
            let mut rows: Vec<Vec<serde_json::Value>> = Vec::new();
            let mut first = true;
            loop {
                match stream.next(tx_ref.handle()).await {
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
                        failure = Some((entry.source.clone(), e));
                        break 'statements;
                    }
                }
            }

            if json {
                row_results.push(QueryResult {
                    source: entry.source.clone(),
                    columns,
                    rows,
                    summary: QuerySummary::default(),
                    is_write,
                    row: Some(row_index),
                });
            }
        }

        if let Some((failing_source, err)) = failure {
            // Roll back the active transaction; partial row results are dropped.
            if let Some(t) = tx.take() {
                let _ = t.rollback().await;
            }
            // Render the failure per the contract: primary line is the Neo4j
            // error code when available, otherwise the underlying message.
            let (code, message) = neo4j_error_parts(&err);
            let primary = code.unwrap_or_else(|| {
                message
                    .lines()
                    .next()
                    .unwrap_or("query execution failed")
                    .to_string()
            });
            eprintln!("Error on row {}: {primary}", row_index + 1);
            eprintln!("  Source: {failing_source}");
            match mode {
                BatchMode::Atomic => {
                    eprintln!("  Transaction rolled back. 0 rows committed.");
                }
                BatchMode::Batched(_n) => {
                    let rolled_back = rows_in_batch + 1; // includes the failing row
                    eprintln!(
                        "  {} ({}), {} in current batch rolled back.",
                        pluralize(total_rows_committed, "row committed", "rows committed"),
                        pluralize(batches_committed, "batch", "batches"),
                        pluralize(rolled_back, "row", "rows"),
                    );
                }
            }
            eprintln!("  Underlying: {message}");

            // Flush partial JSON: only rows that completed all their
            // statements appear here. The failing row's row_results are
            // dropped by virtue of never being appended.
            if json {
                print_json(&results);
            }
            std::process::exit(2);
        }

        // Row succeeded — commit results to the run-wide collection.
        if json {
            results.append(&mut row_results);
        }
        row_count += 1;

        let row_index_1based = row_index + 1;
        rows_in_batch += 1;

        // Maybe commit (Batched mode only).
        if let BatchMode::Batched(n) = mode {
            if rows_in_batch >= n {
                let to_commit = tx.take().expect("tx was open while batching");
                if let Err(e) = to_commit.commit().await {
                    eprintln!("Error: failed to commit batch: {e}");
                    std::process::exit(2);
                }
                batches_committed += 1;
                total_rows_committed += rows_in_batch;
                rows_in_batch = 0;
                if should_emit_progress(row_index_1based, total_hint) {
                    eprintln!(
                        "[{}/{}] applied row {} (batch {} committed)",
                        row_index_1based,
                        total_hint
                            .map(|t| t.to_string())
                            .unwrap_or_else(|| "?".into()),
                        row_index_1based,
                        batches_committed,
                    );
                }
            } else if should_emit_progress(row_index_1based, total_hint) {
                eprintln!(
                    "[{}/{}] applied row {} (batch {}: {}/{})",
                    row_index_1based,
                    total_hint
                        .map(|t| t.to_string())
                        .unwrap_or_else(|| "?".into()),
                    row_index_1based,
                    batches_committed + 1,
                    rows_in_batch,
                    n,
                );
            }
        } else if should_emit_progress(row_index_1based, total_hint) {
            eprintln!(
                "[{}/{}] applied row {} (atomic — 1 transaction)",
                row_index_1based,
                total_hint
                    .map(|t| t.to_string())
                    .unwrap_or_else(|| "?".into()),
                row_index_1based,
            );
        }

        row_index += 1;
    }

    // End-of-input: commit any open transaction.
    if let Some(t) = tx.take() {
        if let Err(e) = t.commit().await {
            eprintln!("Error: failed to commit final transaction: {e}");
            std::process::exit(2);
        }
    }

    let _ = total_rows_committed; // counters used only on failure paths
    let _ = batches_committed;
    Ok(ApplyOutcome { results, row_count })
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

        let _ = stream.finish().await;

        results.push(QueryResult {
            source: entry.source.clone(),
            columns,
            rows,
            summary: QuerySummary::default(),
            is_write,
            row: None,
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
    /// Per-row index from --apply; omitted from JSON when None (M1/M2 schema preserved).
    #[serde(skip_serializing_if = "Option::is_none")]
    row: Option<usize>,
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
            row: r.row,
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

pub async fn run(mut args: QueryArgs, neo4j: Neo4jArgs) -> Result<()> {
    // Disambiguate `-e '...' '{...}'`: clap binds the trailing positional to
    // [QUERY], but with --expr present the user means it for [PARAMS].
    // A leading `{` is unambiguous — bare query names and file paths can't start with one.
    if !args.expr.is_empty()
        && args.params_map.is_none()
        && args
            .query
            .as_deref()
            .is_some_and(|q| q.trim_start().starts_with('{'))
    {
        args.params_map = args.query.take();
    }

    // Mutual exclusion: [QUERY] and -e are mutually exclusive
    if args.query.is_some() && !args.expr.is_empty() {
        eprintln!("Error: [QUERY] and --expr (-e) are mutually exclusive");
        eprintln!("       provide either a file path or one or more -e flags, not both");
        std::process::exit(1);
    }

    // Mutual exclusion: [PARAMS] and --params are mutually exclusive
    if args.params_map.is_some() && args.params.is_some() {
        eprintln!("Error: [PARAMS] and --params are mutually exclusive");
        eprintln!("       provide inline parameters or a --params file, not both");
        std::process::exit(1);
    }

    // Mutual exclusion: --apply and [PARAMS] are mutually exclusive
    if args.apply.is_some() && args.params_map.is_some() {
        eprintln!("Error: --apply and [PARAMS] are mutually exclusive");
        eprintln!("       per-row data comes from --apply; use --param for constants");
        std::process::exit(1);
    }

    // Resolve batch mode (also enforces --batch ⊥ --atomic and the "requires --apply" rule).
    let batch_mode = match BatchMode::from_args(&args) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("Error: {e}");
            std::process::exit(1);
        }
    };

    // --list with no [QUERY]: enumerate the whole library and exit
    if args.list && args.query.is_none() && args.expr.is_empty() {
        list_library(&args.cypher_dir, args.json);
        return Ok(());
    }

    // Build statement queue
    let queue = if !args.expr.is_empty() {
        build_queue_inline(&args.expr)
    } else if let Some(ref query_str) = args.query {
        let name = resolve_query_source(query_str, &args.cypher_dir);
        match open_library_entries(&name, &args.cypher_dir) {
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

    // --list with [QUERY]: show statements in the resolved file and exit
    if args.list {
        print_list(&queue, args.json);
        return Ok(());
    }

    // --describe: print documentation and exit without executing
    if args.describe {
        print_describe(&queue);
        return Ok(());
    }

    // Preflight pipeline (all before opening a Bolt connection)
    preflight_lint(&queue);
    preflight_write(&queue, args.write);

    let constants = match build_param_map(&args) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Error: {e}");
            std::process::exit(1);
        }
    };

    // ── Apply path (Milestone 3) ─────────────────────────────────────────────
    if let Some(apply_path) = args.apply.clone() {
        let reader = match PeekableRowReader::open(&apply_path) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("Error: {e}");
                std::process::exit(1);
            }
        };

        // Synthesise the first-row parameter map for preflight Stage 3.
        // If the file is empty, preflight against just the constants.
        let preflight_params_map = match reader.first_row() {
            Some(first) => {
                let mut merged: ParamMap = first.clone();
                for (k, v) in &constants {
                    merged.insert(k.clone(), v.clone());
                }
                merged
            }
            None => {
                // Empty file: preflight against just constants. If the queue
                // has required params not in constants, that's the FR-017
                // "no input rows found" error.
                let mut required_missing = false;
                for entry in &queue {
                    if let Some(doc) = &entry.doc {
                        for decl in &doc.params {
                            if decl.required && !constants.contains_key(&decl.name) {
                                required_missing = true;
                            }
                        }
                    } else {
                        for name in collect_param_refs(&entry.text) {
                            if !constants.contains_key(name.as_str()) {
                                required_missing = true;
                            }
                        }
                    }
                }
                if required_missing {
                    eprintln!(
                        "Error: no input rows found in '{}'\n  Hint: the query has required parameters but the data file contains no rows.",
                        apply_path.display()
                    );
                    std::process::exit(1);
                }
                return Ok(());
            }
        };
        preflight_params(&queue, &preflight_params_map);

        let outcome =
            execute_queue_apply(&queue, &constants, reader, batch_mode, &neo4j, args.json).await?;

        if args.json {
            print_json(&outcome.results);
        } else {
            let row_count = outcome.row_count;
            match batch_mode {
                BatchMode::Atomic => {
                    println!("{row_count} row(s) applied in 1 atomic transaction.");
                }
                BatchMode::Batched(n) => {
                    let batches = row_count.div_ceil(n.max(1)).max(1);
                    println!("{row_count} row(s) applied across {batches} batch(es).");
                }
            }
        }
        return Ok(());
    }

    // ── Single-statement path (M1 / M2) ──────────────────────────────────────
    preflight_params(&queue, &constants);

    let results = execute_queue(&queue, &constants, &neo4j).await?;

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
    use std::path::Path;

    // ── Milestone 1 tests (preserved) ────────────────────────────────────────

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

    // ── T009: resolve_query_source tests (US1) ───────────────────────────────

    #[test]
    fn test_resolve_bare_name() {
        let dir = Path::new("./cypher");
        let result = resolve_query_source("find_person", dir);
        assert!(matches!(result, QueryName::BareName { ref name, .. } if name == "find_person"));
        if let QueryName::BareName { resolved, .. } = result {
            assert_eq!(resolved, dir.join("find_person.cypher"));
        }
    }

    #[test]
    fn test_resolve_explicit_path_with_extension() {
        let dir = Path::new("./cypher");
        let result = resolve_query_source("queries/find.cypher", dir);
        assert!(matches!(result, QueryName::ExplicitPath(_)));
    }

    #[test]
    fn test_resolve_explicit_path_with_separator() {
        let dir = Path::new("./cypher");
        let result = resolve_query_source("./find_person.cypher", dir);
        assert!(matches!(result, QueryName::ExplicitPath(_)));
    }

    #[test]
    fn test_resolve_dotcypher_extension() {
        let dir = Path::new("./cypher");
        let result = resolve_query_source("find_person.cypher", dir);
        assert!(matches!(result, QueryName::ExplicitPath(_)));
    }

    #[test]
    fn test_resolve_stmt_address() {
        let dir = Path::new("./cypher");
        let result = resolve_query_source("person/upsert", dir);
        assert!(
            matches!(&result, QueryName::StmtAddress { stmt_name, .. } if stmt_name == "upsert")
        );
        if let QueryName::StmtAddress { file, stmt_name } = result {
            assert_eq!(file, dir.join("person.cypher"));
            assert_eq!(stmt_name, "upsert");
        }
    }

    #[test]
    fn test_resolve_cypher_dir_override() {
        let custom_dir = Path::new("./queries");
        let result = resolve_query_source("find_person", custom_dir);
        if let QueryName::BareName { resolved, .. } = result {
            assert_eq!(resolved, custom_dir.join("find_person.cypher"));
        } else {
            panic!("expected BareName");
        }
    }

    // ── T012: parse_map_literal tests (US2) ─────────────────────────────────

    #[test]
    fn test_parse_map_literal_unquoted_keys() {
        let map = parse_map_literal(r#"{name: "Alice", age: 30}"#).unwrap();
        assert!(matches!(map.get("name"), Some(ParamValue::String(s)) if s == "Alice"));
        assert!(matches!(map.get("age"), Some(ParamValue::Integer(30))));
    }

    #[test]
    fn test_parse_map_literal_quoted_keys_accepted() {
        // Spec US2.4: JSON-style quoted keys parse identically to unquoted keys.
        // tree-sitter-cypher rejects quoted-key maps, so parse_map_literal falls
        // back to JSON parsing for this shape.
        let map = parse_map_literal(r#"{"name": "Alice", "age": 30}"#).unwrap();
        assert!(matches!(map.get("name"), Some(ParamValue::String(s)) if s == "Alice"));
        assert!(matches!(map.get("age"), Some(ParamValue::Integer(30))));
    }

    #[test]
    fn test_parse_map_literal_boolean_and_null() {
        let map = parse_map_literal(r#"{active: true, score: 3.14, nothing: null}"#).unwrap();
        assert!(
            matches!(map.get("active"), Some(ParamValue::Boolean(true))),
            "got: {:?}",
            map.get("active")
        );
        assert!(
            matches!(map.get("score"), Some(ParamValue::Float(_))),
            "got: {:?}",
            map.get("score")
        );
        assert!(
            matches!(
                map.get("nothing"),
                Some(ParamValue::Json(serde_json::Value::Null))
            ),
            "got: {:?}",
            map.get("nothing")
        );
    }

    #[test]
    fn test_parse_map_literal_invalid() {
        assert!(parse_map_literal("name=Alice").is_err());
    }

    // ── T017: parse_cypherdoc tests (US3) ────────────────────────────────────

    #[test]
    fn test_parse_cypherdoc_full_block() {
        let raw = r#"/**
 * upsert
 *
 * Create or update a person node.
 *
 * @param {string} name - Unique name for the person
 * @param {string} [home=""] - Home city or region
 * @returns {[person: node<Person>][]} - The upserted node
 */"#;
        let doc = parse_cypherdoc(raw).unwrap();
        assert_eq!(doc.name, "upsert");
        assert!(doc
            .description
            .as_deref()
            .unwrap_or("")
            .contains("Create or update"));
        assert_eq!(doc.params.len(), 2);
        assert_eq!(doc.params[0].name, "name");
        assert!(doc.params[0].required);
        assert_eq!(doc.params[1].name, "home");
        assert!(!doc.params[1].required);
        assert_eq!(doc.params[1].default.as_deref(), Some(r#""""#));
        assert!(doc.returns_raw.is_some());
    }

    #[test]
    fn test_parse_cypherdoc_name_only() {
        let raw = "/** upsert */";
        let doc = parse_cypherdoc(raw).unwrap();
        assert_eq!(doc.name, "upsert");
        assert!(doc.params.is_empty());
    }

    #[test]
    fn test_parse_cypherdoc_invalid() {
        assert!(parse_cypherdoc("/* not a cypherdoc */").is_none());
        assert!(parse_cypherdoc("// line comment").is_none());
    }

    // ── T017: filter_by_stmt_name tests (US3) ────────────────────────────────

    fn make_entry(name: &str) -> StatementEntry {
        StatementEntry {
            source: StatementSource::Inline,
            text: format!("MATCH (n:{name}) RETURN n"),
            doc: Some(CypherDoc {
                name: name.to_string(),
                description: None,
                params: vec![],
                returns_raw: None,
            }),
        }
    }

    #[test]
    fn test_filter_by_stmt_name_hit() {
        let entries = vec![make_entry("upsert"), make_entry("delete")];
        let result =
            filter_by_stmt_name(entries, "upsert", Path::new("./cypher/person.cypher")).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].doc.as_ref().unwrap().name, "upsert");
    }

    #[test]
    fn test_filter_by_stmt_name_miss_lists_available() {
        let entries = vec![make_entry("upsert"), make_entry("delete")];
        let path = Path::new("./cypher/person.cypher");
        let err = filter_by_stmt_name(entries, "by_age", path).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("by_age"),
            "error should mention the name: {msg}"
        );
        assert!(
            msg.contains("./cypher/person.cypher"),
            "error should mention the file path: {msg}"
        );
        assert!(msg.contains("upsert"), "error should list available: {msg}");
        assert!(msg.contains("delete"), "error should list available: {msg}");
    }

    // ── T021: --cypher-dir override (US5) ────────────────────────────────────

    #[test]
    fn test_cypher_dir_override_resolves_to_custom_dir() {
        let custom_dir = Path::new("/tmp/myqueries");
        if let QueryName::BareName { resolved, .. } =
            resolve_query_source("find_person", custom_dir)
        {
            assert_eq!(resolved, custom_dir.join("find_person.cypher"));
        } else {
            panic!("expected BareName");
        }
    }

    // ── Milestone 3 tests ────────────────────────────────────────────────────

    fn args_for_test() -> QueryArgs {
        QueryArgs {
            query: None,
            params_map: None,
            expr: vec![],
            param: vec![],
            params: None,
            write: false,
            list: false,
            describe: false,
            cypher_dir: PathBuf::from("./cypher/"),
            apply: None,
            batch: None,
            atomic: false,
            json: false,
        }
    }

    #[test]
    fn test_batch_mode_default_is_batched_1000() {
        let mut args = args_for_test();
        args.apply = Some(PathBuf::from("data.csv"));
        let mode = BatchMode::from_args(&args).unwrap();
        assert_eq!(mode, BatchMode::Batched(1000));
    }

    #[test]
    fn test_batch_mode_accepts_explicit_batch_size() {
        let mut args = args_for_test();
        args.apply = Some(PathBuf::from("data.csv"));
        args.batch = Some(500);
        let mode = BatchMode::from_args(&args).unwrap();
        assert_eq!(mode, BatchMode::Batched(500));
    }

    #[test]
    fn test_batch_mode_atomic() {
        let mut args = args_for_test();
        args.apply = Some(PathBuf::from("data.csv"));
        args.atomic = true;
        let mode = BatchMode::from_args(&args).unwrap();
        assert_eq!(mode, BatchMode::Atomic);
    }

    #[test]
    fn test_batch_mode_rejects_batch_zero() {
        let mut args = args_for_test();
        args.apply = Some(PathBuf::from("data.csv"));
        args.batch = Some(0);
        let err = BatchMode::from_args(&args).unwrap_err();
        assert!(err.to_string().contains("at least 1"));
    }

    #[test]
    fn test_batch_mode_rejects_batch_and_atomic_together() {
        let mut args = args_for_test();
        args.apply = Some(PathBuf::from("data.csv"));
        args.batch = Some(100);
        args.atomic = true;
        let err = BatchMode::from_args(&args).unwrap_err();
        assert!(err.to_string().contains("mutually exclusive"));
    }

    #[test]
    fn test_batch_mode_rejects_flags_without_apply() {
        let mut args = args_for_test();
        args.batch = Some(100);
        let err = BatchMode::from_args(&args).unwrap_err();
        assert!(err.to_string().contains("require --apply"));

        let mut args = args_for_test();
        args.atomic = true;
        let err = BatchMode::from_args(&args).unwrap_err();
        assert!(err.to_string().contains("require --apply"));
    }

    #[test]
    fn test_coerce_param_value_csv_rules() {
        assert!(matches!(coerce_param_value("42"), ParamValue::Integer(42)));
        assert!(matches!(coerce_param_value("3.14"), ParamValue::Float(_)));
        assert!(matches!(
            coerce_param_value("true"),
            ParamValue::Boolean(true)
        ));
        assert!(matches!(
            coerce_param_value("false"),
            ParamValue::Boolean(false)
        ));
        assert!(matches!(
            coerce_param_value("Alice"),
            ParamValue::String(ref s) if s == "Alice"
        ));
        assert!(matches!(
            coerce_param_value(""),
            ParamValue::String(ref s) if s.is_empty()
        ));
    }

    #[test]
    fn test_json_to_param_value_preserves_types() {
        use serde_json::json;
        assert!(matches!(
            json_to_param_value(json!(42)),
            ParamValue::Integer(42)
        ));
        assert!(matches!(
            json_to_param_value(json!(2.5)),
            ParamValue::Float(_)
        ));
        assert!(matches!(
            json_to_param_value(json!(true)),
            ParamValue::Boolean(true)
        ));
        assert!(matches!(
            json_to_param_value(json!("hello")),
            ParamValue::String(ref s) if s == "hello"
        ));
        assert!(matches!(
            json_to_param_value(json!(null)),
            ParamValue::Json(serde_json::Value::Null)
        ));
        assert!(matches!(
            json_to_param_value(json!([1, 2, 3])),
            ParamValue::Json(serde_json::Value::Array(_))
        ));
        assert!(matches!(
            json_to_param_value(json!({"k": 1})),
            ParamValue::Json(serde_json::Value::Object(_))
        ));
    }
}
