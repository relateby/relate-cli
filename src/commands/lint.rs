use crate::cli::{Lang, LintArgs};
use anyhow::Result;
use gram_diagnostics::{Diagnostic, Severity};
use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

// ── Regex patterns for fence extraction (compiled once) ───────────────────────

static MD_FENCE_RE: LazyLock<regex::Regex> = LazyLock::new(|| {
    // Named group `tag` uses (?i:...) for case-insensitive matching: cypher, Cypher, openCypher, GRAM, etc.
    regex::Regex::new(
        r"(?ms)^```[ \t]*(?P<tag>(?i:cypher|openCypher|gram))[ \t]*\n(?P<body>.*?)^```[ \t]*$",
    )
    .expect("valid MD fence regex")
});

static ADOC_FENCE_RE: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(
        r"(?ms)^\[source,[ \t]*(?P<tag>(?i:cypher|openCypher|gram))\]\n----\n(?P<body>.*?)\n----",
    )
    .expect("valid AsciiDoc fence regex")
});

// ── Internal types ─────────────────────────────────────────────────────────────

struct LintDiagnostic {
    #[allow(dead_code)]
    lang: Lang,
    source_file: Option<PathBuf>,
    inner: Diagnostic,
}

struct Snippet {
    lang: Lang,
    source: String,
    /// 0-based line of the first content line in the parent document.
    fence_start_line: u32,
}

// ── JSON output types ──────────────────────────────────────────────────────────

#[derive(serde::Serialize)]
struct JsonDiagnostic<'a> {
    severity: &'a str,
    rule: &'a str,
    message: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    code: Option<&'a str>,
    file: Option<&'a str>,
    range: JsonRange,
}

#[derive(serde::Serialize)]
struct JsonRange {
    start: JsonPosition,
    end: JsonPosition,
}

#[derive(serde::Serialize)]
struct JsonPosition {
    line: u32,
    column: u32, // renamed from gram_diagnostics::Position::character
}

// ── Helpers ────────────────────────────────────────────────────────────────────

fn severity_str(s: &Severity) -> &'static str {
    match s {
        Severity::Error => "error",
        Severity::Warning => "warning",
        Severity::Information => "information",
        Severity::Hint => "hint",
    }
}

/// Convert a (line, character) pair to a UTF-8 byte offset within `source`.
///
/// Splits on `\n` so the length of each segment includes any trailing `\r`,
/// making the sum correct for both LF and CRLF line endings.
fn to_byte_offset(source: &str, line: u32, character: u32) -> usize {
    let line_start: usize = source
        .split('\n')
        .take(line as usize)
        .map(|l| l.len() + 1) // +1 for the \n byte; \r is already in l.len() for CRLF
        .sum();
    (line_start + character as usize).min(source.len())
}

fn lang_from_tag(tag: &str) -> Lang {
    if tag.eq_ignore_ascii_case("gram") {
        Lang::Gram
    } else {
        Lang::Cypher
    }
}

// cypher_data::lint::lint_source returns cypher_data::types::Diagnostic (its own type),
// not gram_diagnostics::Diagnostic. The conversion below is therefore not redundant —
// cypher-data does not yet re-export from gram-diagnostics. See gram-data/tree-sitter-cypher#8.
fn from_cypher_diagnostic(d: cypher_data::types::Diagnostic) -> Diagnostic {
    Diagnostic {
        severity: match d.severity {
            cypher_data::types::Severity::Error => Severity::Error,
            cypher_data::types::Severity::Warning => Severity::Warning,
            cypher_data::types::Severity::Information => Severity::Information,
            cypher_data::types::Severity::Hint => Severity::Hint,
        },
        rule: d.rule,
        message: d.message,
        code: d.code,
        range: gram_diagnostics::Range {
            start: gram_diagnostics::Position {
                line: d.range.start.line,
                character: d.range.start.character,
            },
            end: gram_diagnostics::Position {
                line: d.range.end.line,
                character: d.range.end.character,
            },
        },
    }
}

// ── Fence extraction ───────────────────────────────────────────────────────────

fn extract_snippets(source: &str) -> Vec<Snippet> {
    let mut snippets = Vec::new();

    for cap in MD_FENCE_RE.captures_iter(source) {
        let tag = cap.name("tag").unwrap().as_str();
        let content = cap.name("body").unwrap().as_str().to_owned();
        let start_byte = cap.get(0).unwrap().start();
        let fence_line = source[..start_byte].bytes().filter(|&b| b == b'\n').count() as u32;
        snippets.push(Snippet {
            lang: lang_from_tag(tag),
            source: content,
            fence_start_line: fence_line + 1, // +1: skip the opening ``` line
        });
    }

    for cap in ADOC_FENCE_RE.captures_iter(source) {
        let tag = cap.name("tag").unwrap().as_str();
        let content = cap.name("body").unwrap().as_str().to_owned();
        let start_byte = cap.get(0).unwrap().start();
        let fence_line = source[..start_byte].bytes().filter(|&b| b == b'\n').count() as u32;
        snippets.push(Snippet {
            lang: lang_from_tag(tag),
            source: content,
            fence_start_line: fence_line + 2, // +2: skip [source,...] and ---- lines
        });
    }

    snippets.sort_by_key(|s| s.fence_start_line);
    snippets
}

fn offset_diagnostic(mut d: Diagnostic, offset: u32) -> Diagnostic {
    d.range.start.line += offset;
    d.range.end.line += offset;
    d
}

// ── Dispatch ───────────────────────────────────────────────────────────────────

fn lint_cypher(source: &str, strict: bool) -> Vec<Diagnostic> {
    let opts = cypher_data::lint::LintOptions { strict };
    cypher_data::lint::lint_source(source, &opts)
        .into_iter()
        .map(from_cypher_diagnostic)
        .collect()
}

fn lint_gram(source: &str, strict: bool) -> Vec<Diagnostic> {
    let opts = gram_data::lint::LintOptions { strict };
    gram_data::lint::lint_source(source, &opts)
}

fn lint_source_for_lang(lang: Lang, source: &str, strict: bool) -> Vec<Diagnostic> {
    match lang {
        Lang::Cypher => lint_cypher(source, strict),
        Lang::Gram => lint_gram(source, strict),
    }
}

/// Lint a file and return (diagnostics, source_text).
fn lint_path(path: &Path, strict: bool) -> anyhow::Result<(Vec<LintDiagnostic>, String)> {
    let source = std::fs::read_to_string(path)?;
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");

    let diags: Vec<LintDiagnostic> = match ext {
        "cypher" => lint_cypher(&source, strict)
            .into_iter()
            .map(|inner| LintDiagnostic {
                lang: Lang::Cypher,
                source_file: Some(path.to_owned()),
                inner,
            })
            .collect(),

        "gram" => lint_gram(&source, strict)
            .into_iter()
            .map(|inner| LintDiagnostic {
                lang: Lang::Gram,
                source_file: Some(path.to_owned()),
                inner,
            })
            .collect(),

        "md" | "adoc" => {
            let snippets = extract_snippets(&source);
            let mut all = Vec::new();
            for snippet in snippets {
                for d in lint_source_for_lang(snippet.lang, &snippet.source, strict) {
                    all.push(LintDiagnostic {
                        lang: snippet.lang,
                        source_file: Some(path.to_owned()),
                        inner: offset_diagnostic(d, snippet.fence_start_line),
                    });
                }
            }
            all
        }

        _ => anyhow::bail!("unsupported file type: {}", path.display()),
    };

    Ok((diags, source))
}

/// Collect paths from files/directories, filtering by supported extension.
fn collect_paths(inputs: &[PathBuf]) -> anyhow::Result<Vec<PathBuf>> {
    let mut paths = Vec::new();
    for input in inputs {
        if input.is_dir() {
            for entry in walkdir::WalkDir::new(input)
                .into_iter()
                .filter_map(|e| e.ok())
            {
                let p = entry.path();
                if matches!(
                    p.extension().and_then(|e| e.to_str()),
                    Some("cypher" | "gram" | "md" | "adoc")
                ) {
                    paths.push(p.to_owned());
                }
            }
        } else {
            paths.push(input.clone());
        }
    }
    Ok(paths)
}

// ── Output ─────────────────────────────────────────────────────────────────────

fn print_human(diagnostics: &[LintDiagnostic], sources: &HashMap<PathBuf, String>) {
    for diag in diagnostics {
        let file_name = diag
            .source_file
            .as_ref()
            .and_then(|p| p.to_str())
            .unwrap_or("<expr>");

        let source_text = diag
            .source_file
            .as_ref()
            .and_then(|p| sources.get(p))
            .map(String::as_str)
            .unwrap_or("");

        let start = &diag.inner.range.start;
        let end = &diag.inner.range.end;
        let start_offset = to_byte_offset(source_text, start.line, start.character);
        let end_offset = to_byte_offset(source_text, end.line, end.character).max(start_offset + 1);

        let kind = match diag.inner.severity {
            Severity::Error => ariadne::ReportKind::Error,
            Severity::Warning => ariadne::ReportKind::Warning,
            _ => ariadne::ReportKind::Advice,
        };

        let rule = &diag.inner.rule;
        let message = &diag.inner.message;

        let result = ariadne::Report::build(kind, (file_name, start_offset..end_offset))
            .with_message(format!("[{rule}] {message}"))
            .with_label(
                ariadne::Label::new((file_name, start_offset..end_offset))
                    .with_message(message.as_str()),
            )
            .finish()
            .print((file_name, ariadne::Source::from(source_text)));

        if result.is_err() {
            // Fallback if ariadne fails (e.g., non-UTF-8 terminal)
            eprintln!(
                "{}:{}:{}: {} [{}] {}",
                file_name,
                start.line + 1,
                start.character + 1,
                severity_str(&diag.inner.severity),
                rule,
                message
            );
        }
    }
}

fn print_json(diagnostics: &[LintDiagnostic]) {
    let output: Vec<JsonDiagnostic> = diagnostics
        .iter()
        .map(|d| JsonDiagnostic {
            severity: severity_str(&d.inner.severity),
            rule: &d.inner.rule,
            message: &d.inner.message,
            code: d.inner.code.as_deref(),
            // Synthetic keys used for ariadne source lookup — emit null in JSON per CLI contract.
            file: d.source_file.as_ref().and_then(|p| match p.to_str() {
                Some("<expr>" | "<stdin>") => None,
                other => other,
            }),
            range: JsonRange {
                start: JsonPosition {
                    line: d.inner.range.start.line,
                    column: d.inner.range.start.character,
                },
                end: JsonPosition {
                    line: d.inner.range.end.line,
                    column: d.inner.range.end.character,
                },
            },
        })
        .collect();

    println!(
        "{}",
        serde_json::to_string_pretty(&output).unwrap_or_else(|_| "[]".to_string())
    );
}

fn has_exit_one(diagnostics: &[LintDiagnostic], strict: bool) -> bool {
    diagnostics.iter().any(|d| match d.inner.severity {
        Severity::Error => true,
        _ => strict,
    })
}

// ── Entry point ────────────────────────────────────────────────────────────────

pub fn run(args: LintArgs) -> Result<()> {
    let mut diagnostics: Vec<LintDiagnostic> = Vec::new();
    let mut sources: HashMap<PathBuf, String> = HashMap::new();

    if let Some(expr) = &args.expr {
        // --expr: lint an inline expression
        let key = PathBuf::from("<expr>");
        for inner in lint_source_for_lang(args.lang, expr, args.strict) {
            diagnostics.push(LintDiagnostic {
                lang: args.lang,
                source_file: Some(key.clone()),
                inner,
            });
        }
        sources.insert(key, expr.clone());
    } else if args.files.is_empty() {
        // stdin: no files given, read from stdin
        let key = PathBuf::from("<stdin>");
        let mut input = String::new();
        std::io::stdin().read_to_string(&mut input)?;
        for inner in lint_source_for_lang(args.lang, &input, args.strict) {
            diagnostics.push(LintDiagnostic {
                lang: args.lang,
                source_file: Some(key.clone()),
                inner,
            });
        }
        sources.insert(key, input);
    } else {
        // file/directory mode
        let paths = collect_paths(&args.files)?;
        for path in paths {
            let (diags, source) = lint_path(&path, args.strict)?;
            sources.insert(path, source);
            diagnostics.extend(diags);
        }
    }

    if args.json {
        print_json(&diagnostics);
    } else {
        print_human(&diagnostics, &sources);
    }

    if has_exit_one(&diagnostics, args.strict) {
        std::process::exit(1);
    }

    Ok(())
}
