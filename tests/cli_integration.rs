use assert_cmd::Command;
use predicates::prelude::PredicateBooleanExt;
use predicates::str::contains;

fn fixtures() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures")
}

// ── Existing smoke tests ───────────────────────────────────────────────────────

#[test]
fn help_exits_zero() {
    Command::cargo_bin("relate")
        .unwrap()
        .arg("--help")
        .assert()
        .success();
}

#[test]
fn help_mentions_lint() {
    Command::cargo_bin("relate")
        .unwrap()
        .arg("--help")
        .assert()
        .success()
        .stdout(contains("lint"));
}

#[test]
fn help_mentions_mcp() {
    Command::cargo_bin("relate")
        .unwrap()
        .arg("--help")
        .assert()
        .success()
        .stdout(contains("mcp"));
}

#[test]
fn help_mentions_parse() {
    Command::cargo_bin("relate")
        .unwrap()
        .arg("--help")
        .assert()
        .success()
        .stdout(contains("parse"));
}

// ── lint --help ────────────────────────────────────────────────────────────────

#[test]
fn lint_help_shows_lang_flag() {
    Command::cargo_bin("relate")
        .unwrap()
        .args(["lint", "--help"])
        .assert()
        .success()
        .stdout(contains("--lang"));
}

// ── Phase 3: US1 — Cypher file linting ────────────────────────────────────────

#[test]
fn lint_cypher_clean_exits_zero() {
    Command::cargo_bin("relate")
        .unwrap()
        .arg("lint")
        .arg(fixtures().join("valid.cypher"))
        .assert()
        .success();
}

#[test]
fn lint_cypher_violation_exits_one() {
    Command::cargo_bin("relate")
        .unwrap()
        .arg("lint")
        .arg(fixtures().join("invalid.cypher"))
        .assert()
        .code(1);
}

#[test]
fn lint_cypher_violation_output_contains_location() {
    // ariadne output includes the filename and a line reference
    Command::cargo_bin("relate")
        .unwrap()
        .arg("lint")
        .arg(fixtures().join("invalid.cypher"))
        .assert()
        .stdout(contains("invalid.cypher"));
}

#[test]
fn lint_unsupported_explicit_exits_two() {
    Command::cargo_bin("relate")
        .unwrap()
        .arg("lint")
        .arg(fixtures().join("not-a-query.txt"))
        .assert()
        .code(2);
}

// ── Phase 3: US2 — Gram file linting ──────────────────────────────────────────

#[test]
fn lint_gram_clean_exits_zero() {
    Command::cargo_bin("relate")
        .unwrap()
        .arg("lint")
        .arg(fixtures().join("valid.gram"))
        .assert()
        .success();
}

#[test]
fn lint_gram_violation_exits_one() {
    Command::cargo_bin("relate")
        .unwrap()
        .arg("lint")
        .arg(fixtures().join("invalid.gram"))
        .assert()
        .code(1);
}

// ── Phase 4: US3 — Directory walk ─────────────────────────────────────────────

#[test]
fn lint_directory_finds_cypher_violations() {
    // fixtures/ contains invalid.cypher which has violations
    Command::cargo_bin("relate")
        .unwrap()
        .arg("lint")
        .arg(fixtures())
        .assert()
        .code(1);
}

#[test]
fn lint_directory_skips_unsupported() {
    // fixtures/ contains not-a-query.txt; walking it should not error on .txt
    Command::cargo_bin("relate")
        .unwrap()
        .arg("lint")
        .arg(fixtures())
        .assert()
        .stderr(predicates::str::contains("not-a-query.txt").not());
}

// ── Phase 5: US4 — Inline expression & stdin ──────────────────────────────────

#[test]
fn lint_expr_cypher_valid() {
    Command::cargo_bin("relate")
        .unwrap()
        .args(["lint", "--expr", "MATCH (n:Person) RETURN n"])
        .assert()
        .success();
}

#[test]
fn lint_expr_cypher_invalid_exits_one() {
    Command::cargo_bin("relate")
        .unwrap()
        .args(["lint", "--expr", "MATCH (n"])
        .assert()
        .code(1);
}

#[test]
fn lint_expr_gram_valid() {
    // gram is stubbed; exits 0 regardless
    Command::cargo_bin("relate")
        .unwrap()
        .args(["lint", "--lang", "gram", "--expr", "(a)-[:KNOWS]->(b)"])
        .assert()
        .success();
}

#[test]
fn lint_stdin_valid() {
    Command::cargo_bin("relate")
        .unwrap()
        .arg("lint")
        .write_stdin("MATCH (n:Person) RETURN n\n")
        .assert()
        .success();
}

#[test]
fn lint_stdin_invalid_exits_one() {
    Command::cargo_bin("relate")
        .unwrap()
        .arg("lint")
        .write_stdin("MATCH (n\n")
        .assert()
        .code(1);
}

// ── Phase 6: US5 — Markdown/AsciiDoc fence extraction ─────────────────────────

#[test]
fn lint_markdown_exits_one_on_violation() {
    Command::cargo_bin("relate")
        .unwrap()
        .arg("lint")
        .arg(fixtures().join("doc_with_cypher.md"))
        .assert()
        .code(1);
}

#[test]
fn lint_markdown_line_offset_in_json() {
    // The violation in doc_with_cypher.md is inside a fence that starts at line 12
    // (0-indexed). The diagnostic must NOT be at line 0 (snippet-relative).
    let output = Command::cargo_bin("relate")
        .unwrap()
        .args(["lint", "--json"])
        .arg(fixtures().join("doc_with_cypher.md"))
        .assert()
        .code(1)
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).expect("valid JSON");
    let line = json[0]["range"]["start"]["line"]
        .as_u64()
        .expect("line number");
    assert!(
        line > 0,
        "diagnostic line should be offset into the Markdown file, got {line}"
    );
}

#[test]
fn lint_markdown_non_cypher_fences_skipped() {
    // doc_python_only.md has only ```python fences — should produce no diagnostics
    Command::cargo_bin("relate")
        .unwrap()
        .arg("lint")
        .arg(fixtures().join("doc_python_only.md"))
        .assert()
        .success();
}

#[test]
fn lint_asciidoc_gram_fence_extraction() {
    Command::cargo_bin("relate")
        .unwrap()
        .arg("lint")
        .arg(fixtures().join("doc_with_gram.adoc"))
        .assert()
        .code(1);
}

// ── Phase 7: US6 — JSON output ────────────────────────────────────────────────

#[test]
fn lint_json_clean_outputs_empty_array() {
    Command::cargo_bin("relate")
        .unwrap()
        .args(["lint", "--json"])
        .arg(fixtures().join("valid.cypher"))
        .assert()
        .success()
        .stdout(contains("[]"));
}

#[test]
fn lint_json_violation_outputs_array() {
    let output = Command::cargo_bin("relate")
        .unwrap()
        .args(["lint", "--json"])
        .arg(fixtures().join("invalid.cypher"))
        .assert()
        .code(1)
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).expect("valid JSON");
    assert!(json.is_array(), "output should be a JSON array");
    assert!(
        !json.as_array().unwrap().is_empty(),
        "array should not be empty"
    );

    let first = &json[0];
    assert!(first["severity"].is_string(), "missing severity");
    assert!(first["rule"].is_string(), "missing rule");
    assert!(first["message"].is_string(), "missing message");
    assert!(first["file"].is_string(), "missing file");
    assert!(
        first["range"]["start"]["line"].is_number(),
        "missing range.start.line"
    );
    assert!(
        first["range"]["start"]["column"].is_number(),
        "missing range.start.column"
    );
}

// ── Phase 8: US7 — Strict mode ────────────────────────────────────────────────

#[test]
fn lint_strict_warning_exits_one() {
    // warning-only.cypher produces only UnlabelledNode warnings, no errors.
    // --strict should treat warnings as exit 1.
    Command::cargo_bin("relate")
        .unwrap()
        .args(["lint", "--strict"])
        .arg(fixtures().join("warning-only.cypher"))
        .assert()
        .code(1);
}

#[test]
fn lint_no_strict_warning_exits_zero() {
    // Without --strict, warnings alone do not cause exit 1.
    Command::cargo_bin("relate")
        .unwrap()
        .arg("lint")
        .arg(fixtures().join("warning-only.cypher"))
        .assert()
        .success();
}
