pub mod external;
pub mod lint;
pub mod mcp;
pub mod parse;
pub mod query;
pub mod read;
pub mod write;

/// Convert a cypher-data diagnostic to the shared gram_diagnostics type.
/// Both lint.rs and query.rs use this; it lives here to avoid duplication.
pub fn from_cypher_diagnostic(d: cypher_data::types::Diagnostic) -> gram_diagnostics::Diagnostic {
    use gram_diagnostics::Severity;
    gram_diagnostics::Diagnostic {
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
        help: None,
    }
}
