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

// ── relate query integration tests ─────────────────────────────────────────

mod query {
    use assert_cmd::Command;
    use predicates::str::contains;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn cmd() -> Command {
        Command::cargo_bin("relate").unwrap()
    }

    // T036: Write protection fires before any Bolt connection attempt
    #[test]
    fn write_without_flag_exits_one_before_connecting() {
        cmd()
            .args([
                "query",
                "--uri",
                "bolt://127.0.0.1:1",
                "-e",
                "CREATE (n:Test)",
            ])
            .assert()
            .failure()
            .code(1)
            .stderr(contains("write operation requires --write flag"));
    }

    // T018: Single-statement file runs and shows filename as source header
    // Requires no live Neo4j — lint and write-check pass; execution fails (port 1)
    // but we test the preflight path here.
    #[test]
    fn read_query_file_preflight_passes() {
        let mut f = NamedTempFile::with_suffix(".cypher").unwrap();
        writeln!(f, "MATCH (n) RETURN n").unwrap();
        cmd()
            .args([
                "query",
                "--uri",
                "bolt://127.0.0.1:1",
                "--password",
                "dummy",
                f.path().to_str().unwrap(),
            ])
            .assert()
            .failure()
            .code(2); // preflight passes; runtime fails on unreachable URI
    }

    // T019: Multi-statement file with lint error in second statement exits 1 before connecting
    #[test]
    fn multi_statement_lint_error_exits_one() {
        let mut f = NamedTempFile::with_suffix(".cypher").unwrap();
        writeln!(f, "MATCH (n) RETURN n").unwrap();
        writeln!(f, "THIS IS NOT CYPHER !!!").unwrap();
        cmd()
            .args([
                "query",
                "--uri",
                "bolt://127.0.0.1:1",
                f.path().to_str().unwrap(),
            ])
            .assert()
            .failure()
            .code(1); // lint preflight catches the second statement
    }

    // Empty .cypher file → exits 1 with "no statements found"
    #[test]
    fn empty_cypher_file_exits_one() {
        let f = NamedTempFile::with_suffix(".cypher").unwrap();
        cmd()
            .args([
                "query",
                "--uri",
                "bolt://127.0.0.1:1",
                f.path().to_str().unwrap(),
            ])
            .assert()
            .failure()
            .code(1)
            .stderr(contains("no statements found"));
    }

    // Missing required parameter → exits 1 before connecting
    #[test]
    fn missing_param_exits_one() {
        cmd()
            .args([
                "query",
                "--uri",
                "bolt://127.0.0.1:1",
                "-e",
                "MATCH (n {name: $name}) RETURN n",
            ])
            .assert()
            .failure()
            .code(1)
            .stderr(contains("missing required parameter '$name'"));
    }

    // --json flag: preflight still works (exits 1 on lint error)
    #[test]
    fn json_flag_with_lint_error_exits_one() {
        cmd()
            .args([
                "query",
                "--uri",
                "bolt://127.0.0.1:1",
                "--json",
                "-e",
                "NOT VALID CYPHER",
            ])
            .assert()
            .failure()
            .code(1);
    }

    // T035: --json with valid read query exits 2 (preflight passes, runtime fails on port 1)
    // This verifies the JSON output path is reachable
    #[test]
    fn json_flag_preflight_passes_for_valid_read() {
        cmd()
            .args([
                "query",
                "--uri",
                "bolt://127.0.0.1:1",
                "--password",
                "dummy",
                "--json",
                "-e",
                "MATCH (n) RETURN count(n) AS total",
            ])
            .assert()
            .failure()
            .code(2); // preflight passes; runtime fails on unreachable URI
    }

    // Mutual exclusion: [QUERY] and -e together → exits 1
    #[test]
    fn mutual_exclusion_exits_one() {
        let f = NamedTempFile::with_suffix(".cypher").unwrap();
        cmd()
            .args([
                "query",
                f.path().to_str().unwrap(),
                "-e",
                "MATCH (n) RETURN n",
            ])
            .assert()
            .failure()
            .code(1)
            .stderr(contains("mutually exclusive"));
    }

    // ── Milestone 2 integration tests ────────────────────────────────────────

    // T020: --describe prints cypherdoc without executing, exits 0
    #[test]
    fn describe_prints_doc_without_executing() {
        use std::io::Write;
        let mut f = NamedTempFile::with_suffix(".cypher").unwrap();
        write!(
            f,
            r#"/**
 * find_person
 *
 * Find a person by name.
 *
 * @param {{string}} name - Person name
 * @returns {{[p: node<Person>][]}} - The matching node
 */
MATCH (p:Person {{name: $name}}) RETURN p"#
        )
        .unwrap();

        cmd()
            .args(["query", "--describe", f.path().to_str().unwrap()])
            .assert()
            .success()
            .code(0)
            .stdout(contains("find_person"))
            .stdout(contains("@param"))
            .stdout(contains("name"));
    }

    // T020: --describe with two cypherdoc-named statements shows both
    // Statements in multi-statement files must be separated by semicolons.
    #[test]
    fn describe_multi_statement_shows_all() {
        use std::io::Write;
        let mut f = NamedTempFile::with_suffix(".cypher").unwrap();
        write!(
            f,
            r#"/**
 * upsert
 * @param {{string}} name - Name
 */
MERGE (p:Person {{name: $name}}) RETURN p;

/**
 * delete
 * @param {{string}} name - Name
 */
MATCH (p:Person {{name: $name}}) DETACH DELETE p"#
        )
        .unwrap();

        let output = cmd()
            .args(["query", "--describe", f.path().to_str().unwrap()])
            .assert()
            .success()
            .code(0);
        let out = std::str::from_utf8(&output.get_output().stdout).unwrap();
        assert!(out.contains("upsert"), "should contain 'upsert': {out}");
        assert!(out.contains("delete"), "should contain 'delete': {out}");
    }

    // T020: --describe does not produce JSON output (always human-readable)
    #[test]
    fn describe_ignores_json_flag() {
        let mut f = NamedTempFile::with_suffix(".cypher").unwrap();
        write!(f, "/** find\n */\nMATCH (n) RETURN n").unwrap();

        let output = cmd()
            .args(["query", "--describe", "--json", f.path().to_str().unwrap()])
            .assert()
            .success()
            .code(0);
        let out = std::str::from_utf8(&output.get_output().stdout).unwrap();
        assert!(
            !out.starts_with('['),
            "--describe should not output JSON: {out}"
        );
    }

    // T022: --cypher-dir resolves bare name from a custom directory
    #[test]
    fn cypher_dir_override_finds_query_in_custom_dir() {
        let dir = tempfile::tempdir().unwrap();
        let query_file = dir.path().join("custom_query.cypher");
        std::fs::write(&query_file, "MATCH (n) RETURN n").unwrap();

        // Preflight passes (read-only, no params), runtime fails on port 1
        cmd()
            .args([
                "query",
                "--cypher-dir",
                dir.path().to_str().unwrap(),
                "--uri",
                "bolt://127.0.0.1:1",
                "--password",
                "dummy",
                "custom_query",
            ])
            .assert()
            .failure()
            .code(2); // preflight passes; runtime fails on unreachable URI
    }

    // T022: --cypher-dir with missing bare name shows the custom directory in error
    #[test]
    fn cypher_dir_override_missing_shows_custom_dir_in_error() {
        let dir = tempfile::tempdir().unwrap();
        cmd()
            .args([
                "query",
                "--cypher-dir",
                dir.path().to_str().unwrap(),
                "nonexistent_query",
            ])
            .assert()
            .failure()
            .code(1)
            .stderr(contains("nonexistent_query"));
    }

    // [PARAMS] and --params mutual exclusion → exits 1
    #[test]
    fn params_map_and_params_file_mutual_exclusion() {
        use std::io::Write;
        let mut f = NamedTempFile::with_suffix(".cypher").unwrap();
        write!(f, "MATCH (n) RETURN n").unwrap();
        let mut pf = NamedTempFile::with_suffix(".json").unwrap();
        write!(pf, "{{}}").unwrap();

        cmd()
            .args([
                "query",
                f.path().to_str().unwrap(),
                "{name: \"Alice\"}",
                "--params",
                pf.path().to_str().unwrap(),
            ])
            .assert()
            .failure()
            .code(1)
            .stderr(contains("mutually exclusive"));
    }

    // ── --list integration tests ──────────────────────────────────────────────

    // --list on a single file: shows statement names and one-line descriptions
    #[test]
    fn list_single_file_shows_names() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("ops.cypher"),
            "/** create\n * Make a node.\n */\nCREATE (n) RETURN n;\n\
             /** delete\n * Remove a node.\n */\nMATCH (n) DELETE n",
        )
        .unwrap();

        let output = cmd()
            .args([
                "query",
                "--list",
                dir.path().join("ops.cypher").to_str().unwrap(),
            ])
            .assert()
            .success()
            .code(0);
        let out = std::str::from_utf8(&output.get_output().stdout).unwrap();
        assert!(out.contains("create"), "should list 'create': {out}");
        assert!(out.contains("delete"), "should list 'delete': {out}");
        assert!(
            out.contains("Make a node"),
            "should show description: {out}"
        );
    }

    // --list --json on a single file: produces a JSON array with name and description fields
    #[test]
    fn list_single_file_json() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("ops.cypher"),
            "/** find\n * Locate a node.\n */\nMATCH (n) RETURN n",
        )
        .unwrap();

        let output = cmd()
            .args([
                "query",
                "--list",
                "--json",
                dir.path().join("ops.cypher").to_str().unwrap(),
            ])
            .assert()
            .success()
            .code(0);
        let stdout = output.get_output().stdout.clone();
        let arr: serde_json::Value = serde_json::from_slice(&stdout).expect("valid JSON array");
        assert!(arr.is_array(), "should be a JSON array");
        let first = &arr[0];
        assert_eq!(first["name"], "find");
        assert!(
            first["description"]
                .as_str()
                .unwrap_or("")
                .contains("Locate"),
            "description should contain 'Locate': {first}"
        );
    }

    // --list with no [QUERY]: lists all files in the library directory
    #[test]
    fn list_library_wide_shows_all_files() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("alpha.cypher"),
            "/** find_alpha */\nMATCH (n:Alpha) RETURN n",
        )
        .unwrap();
        std::fs::write(
            dir.path().join("beta.cypher"),
            "/** find_beta */\nMATCH (n:Beta) RETURN n",
        )
        .unwrap();

        let output = cmd()
            .args([
                "query",
                "--list",
                "--cypher-dir",
                dir.path().to_str().unwrap(),
            ])
            .assert()
            .success()
            .code(0);
        let out = std::str::from_utf8(&output.get_output().stdout).unwrap();
        assert!(out.contains("alpha"), "should list 'alpha': {out}");
        assert!(out.contains("beta"), "should list 'beta': {out}");
        assert!(
            out.contains("find_alpha"),
            "should list 'find_alpha': {out}"
        );
        assert!(out.contains("find_beta"), "should list 'find_beta': {out}");
    }

    // --list --json with no [QUERY]: flat JSON array with file/stmt names
    #[test]
    fn list_library_wide_json() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("things.cypher"),
            "/** create_thing\n * Make a thing.\n */\nCREATE (n:Thing) RETURN n",
        )
        .unwrap();

        let output = cmd()
            .args([
                "query",
                "--list",
                "--json",
                "--cypher-dir",
                dir.path().to_str().unwrap(),
            ])
            .assert()
            .success()
            .code(0);
        let stdout = output.get_output().stdout.clone();
        let arr: serde_json::Value = serde_json::from_slice(&stdout).expect("valid JSON array");
        assert!(arr.is_array());
        let first = &arr[0];
        // Library-wide JSON uses "file/stmt" form
        assert_eq!(first["name"], "things/create_thing");
        assert!(
            first["description"].as_str().unwrap_or("").contains("Make"),
            "description should contain 'Make': {first}"
        );
    }

    // --list on an empty directory: exits 0 with no output
    #[test]
    fn list_empty_library_exits_zero() {
        let dir = tempfile::tempdir().unwrap();
        cmd()
            .args([
                "query",
                "--list",
                "--cypher-dir",
                dir.path().to_str().unwrap(),
            ])
            .assert()
            .success()
            .code(0);
    }

    // --list on a missing directory: exits 1 with an error
    #[test]
    fn list_missing_library_exits_one() {
        cmd()
            .args([
                "query",
                "--list",
                "--cypher-dir",
                "/tmp/nonexistent_relate_dir",
            ])
            .assert()
            .failure()
            .code(1)
            .stderr(contains("not found"));
    }
}
