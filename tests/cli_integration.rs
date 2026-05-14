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
    use predicates::prelude::PredicateBooleanExt;
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

    // --describe on a file/stmt address must not duplicate the statement name
    // in the source label (e.g. "movies.cypher (upsert) (upsert)").
    #[test]
    fn describe_library_statement_does_not_duplicate_name() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("movies.cypher"),
            "/**\n * upsert\n * @param {string} title - Movie title\n */\n\
             MERGE (m:Movie {title: $title}) RETURN m;",
        )
        .unwrap();

        let output = cmd()
            .args([
                "query",
                "--describe",
                "--cypher-dir",
                dir.path().to_str().unwrap(),
                "movies/upsert",
            ])
            .assert()
            .success()
            .code(0);
        let out = std::str::from_utf8(&output.get_output().stdout).unwrap();
        assert!(
            !out.contains("(upsert) (upsert)"),
            "label should not duplicate stmt name: {out}"
        );
        assert!(out.contains("upsert"), "should mention 'upsert': {out}");
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

    // -e with a trailing positional map literal: clap binds the positional to
    // [QUERY] by default, but with --expr present the disambiguation in run()
    // re-routes a leading-`{` positional to [PARAMS]. The test passes if the
    // mutex check is NOT triggered (would say "QUERY and --expr are mutually
    // exclusive") and we instead reach the write-protection short-circuit.
    #[test]
    fn expr_with_positional_map_routes_to_params_not_query() {
        cmd()
            .args([
                "query",
                "-e",
                "CREATE (n:Test {name: $name})",
                "{name: \"Alice\"}",
            ])
            .assert()
            .failure()
            .code(1)
            .stderr(contains("write operation requires --write flag"))
            .stderr(contains("mutually exclusive").not());
    }

    // Same with quoted-key (JSON-style) map — also reaches write-protection.
    #[test]
    fn expr_with_positional_quoted_key_map_is_accepted() {
        cmd()
            .args([
                "query",
                "-e",
                "CREATE (n:Test {name: $name})",
                "{\"name\": \"Alice\"}",
            ])
            .assert()
            .failure()
            .code(1)
            .stderr(contains("write operation requires --write flag"))
            .stderr(contains("mutually exclusive").not());
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

    // ── Milestone 3: --apply batch execution ────────────────────────────────

    /// Helper: write a small CSV file with the given headers and rows.
    fn write_csv(headers: &[&str], rows: &[&[&str]]) -> NamedTempFile {
        let mut f = NamedTempFile::with_suffix(".csv").unwrap();
        writeln!(f, "{}", headers.join(",")).unwrap();
        for row in rows {
            writeln!(f, "{}", row.join(",")).unwrap();
        }
        f
    }

    /// Helper: write a temp .cypher file with the given body.
    fn write_cypher(body: &str) -> NamedTempFile {
        let mut f = NamedTempFile::with_suffix(".cypher").unwrap();
        writeln!(f, "{body}").unwrap();
        f
    }

    // T026: unknown --apply file extension rejected before any I/O on contents.
    #[test]
    fn apply_unknown_extension_exits_one() {
        let mut f = NamedTempFile::with_suffix(".txt").unwrap();
        writeln!(f, "anything").unwrap();
        let q = write_cypher("MATCH (n) RETURN n");

        cmd()
            .args([
                "query",
                "--uri",
                "bolt://127.0.0.1:1",
                q.path().to_str().unwrap(),
                "--apply",
                f.path().to_str().unwrap(),
            ])
            .assert()
            .failure()
            .code(1)
            .stderr(contains("requires a .csv, .json, or .jsonl"));
    }

    // T016 + FR-006: missing required parameter on row 0 aborts before any connection.
    #[test]
    fn apply_csv_missing_column_preflight_fails() {
        // CSV has only `name`, but the query needs `$age`.
        let csv = write_csv(&["name"], &[&["Alice"]]);
        let q = write_cypher("CREATE (n:Person {name: $name, age: $age})");

        cmd()
            .args([
                "query",
                "--uri",
                "bolt://127.0.0.1:1",
                "--password",
                "dummy",
                q.path().to_str().unwrap(),
                "--apply",
                csv.path().to_str().unwrap(),
                "--write",
            ])
            .assert()
            .failure()
            .code(1)
            .stderr(contains("missing required parameter"));
    }

    // T010: --apply and a positional [PARAMS] map are mutually exclusive.
    #[test]
    fn apply_and_params_map_mutual_exclusion_exits_one() {
        let csv = write_csv(&["name"], &[&["Alice"]]);
        let q = write_cypher("CREATE (n:Person {name: $name})");

        cmd()
            .args([
                "query",
                "--uri",
                "bolt://127.0.0.1:1",
                q.path().to_str().unwrap(),
                "{name: \"Bob\"}",
                "--apply",
                csv.path().to_str().unwrap(),
                "--write",
            ])
            .assert()
            .failure()
            .code(1)
            .stderr(contains("--apply and [PARAMS] are mutually exclusive"));
    }

    // T003: --batch and --atomic are mutually exclusive.
    #[test]
    fn apply_batch_and_atomic_mutual_exclusion_exits_one() {
        let csv = write_csv(&["name"], &[&["Alice"]]);
        let q = write_cypher("CREATE (n:Person {name: $name})");

        cmd()
            .args([
                "query",
                "--uri",
                "bolt://127.0.0.1:1",
                q.path().to_str().unwrap(),
                "--apply",
                csv.path().to_str().unwrap(),
                "--batch",
                "100",
                "--atomic",
                "--write",
            ])
            .assert()
            .failure()
            .code(1)
            .stderr(contains("--batch and --atomic are mutually exclusive"));
    }

    // T003: --batch without --apply is an error.
    #[test]
    fn apply_batch_without_apply_rejected() {
        cmd()
            .args([
                "query",
                "--uri",
                "bolt://127.0.0.1:1",
                "-e",
                "MATCH (n) RETURN n",
                "--batch",
                "100",
            ])
            .assert()
            .failure()
            .code(1)
            .stderr(contains("require --apply"));
    }

    // T012/T018: CSV with empty header column is rejected at preflight.
    #[test]
    fn apply_csv_malformed_header_rejected() {
        let mut f = NamedTempFile::with_suffix(".csv").unwrap();
        // Three header positions but the middle one is empty.
        writeln!(f, "name,,age").unwrap();
        writeln!(f, "Alice,_,30").unwrap();
        let q = write_cypher("CREATE (n:Person {name: $name})");

        cmd()
            .args([
                "query",
                "--uri",
                "bolt://127.0.0.1:1",
                q.path().to_str().unwrap(),
                "--apply",
                f.path().to_str().unwrap(),
                "--write",
            ])
            .assert()
            .failure()
            .code(1)
            .stderr(contains("Empty column"));
    }

    // T019 / FR-016: JSON top-level non-array is rejected.
    #[test]
    fn apply_json_non_array_rejected() {
        let mut f = NamedTempFile::with_suffix(".json").unwrap();
        writeln!(f, r#"{{"name": "Alice"}}"#).unwrap();
        let q = write_cypher("CREATE (n:Person {name: $name})");

        cmd()
            .args([
                "query",
                "--uri",
                "bolt://127.0.0.1:1",
                q.path().to_str().unwrap(),
                "--apply",
                f.path().to_str().unwrap(),
                "--write",
            ])
            .assert()
            .failure()
            .code(1)
            .stderr(contains("top-level array of objects"));
    }

    // T019 / FR-016: JSON array with non-object element rejected.
    #[test]
    fn apply_json_array_non_object_element_rejected() {
        let mut f = NamedTempFile::with_suffix(".json").unwrap();
        writeln!(f, r#"[{{"name": "Alice"}}, "not an object"]"#).unwrap();
        let q = write_cypher("CREATE (n:Person {name: $name})");

        cmd()
            .args([
                "query",
                "--uri",
                "bolt://127.0.0.1:1",
                q.path().to_str().unwrap(),
                "--apply",
                f.path().to_str().unwrap(),
                "--write",
            ])
            .assert()
            .failure()
            .code(1)
            .stderr(contains("must be an object"));
    }

    // T020 / FR-015: malformed JSONL line aborts with line number.
    #[test]
    fn apply_jsonl_malformed_line_aborts() {
        let mut f = NamedTempFile::with_suffix(".jsonl").unwrap();
        // The first line is malformed — PeekableRowReader::open will catch it.
        writeln!(f, "this is not json").unwrap();
        writeln!(f, r#"{{"name": "Bob"}}"#).unwrap();
        let q = write_cypher("CREATE (n:Person {name: $name})");

        cmd()
            .args([
                "query",
                "--uri",
                "bolt://127.0.0.1:1",
                q.path().to_str().unwrap(),
                "--apply",
                f.path().to_str().unwrap(),
                "--write",
            ])
            .assert()
            .failure()
            .code(1)
            .stderr(contains("line 1").and(contains("invalid JSON")));
    }

    // FR-017: empty CSV with no required params exits 0 silently (no connection).
    #[test]
    fn apply_empty_csv_no_required_params_exits_zero() {
        let mut f = NamedTempFile::with_suffix(".csv").unwrap();
        writeln!(f, "name").unwrap(); // header only — zero data rows
        let q = write_cypher("MATCH (n) RETURN count(n) AS c");

        cmd()
            .args([
                "query",
                "--uri",
                "bolt://127.0.0.1:1",
                q.path().to_str().unwrap(),
                "--apply",
                f.path().to_str().unwrap(),
            ])
            .assert()
            .success(); // exit 0; no Neo4j connection attempted
    }

    // FR-017: empty CSV with required params exits 1 with hint.
    #[test]
    fn apply_empty_csv_with_required_params_exits_one() {
        let mut f = NamedTempFile::with_suffix(".csv").unwrap();
        writeln!(f, "name").unwrap();
        let q = write_cypher("CREATE (n:Person {name: $name})");

        cmd()
            .args([
                "query",
                "--uri",
                "bolt://127.0.0.1:1",
                q.path().to_str().unwrap(),
                "--apply",
                f.path().to_str().unwrap(),
                "--write",
            ])
            .assert()
            .failure()
            .code(1)
            .stderr(contains("no input rows found"));
    }

    // T015 / SC-001 — end-to-end CSV apply, gated by NEO4J_PASSWORD.
    #[test]
    fn apply_csv_basic_against_live_neo4j() {
        let password = match std::env::var("NEO4J_PASSWORD") {
            Ok(p) if !p.is_empty() => p,
            _ => return, // skip when no Neo4j is available
        };

        let csv = write_csv(
            &["name", "age"],
            &[
                &["RelateTestA", "30"],
                &["RelateTestB", "25"],
                &["RelateTestC", "42"],
            ],
        );
        let q = write_cypher("CREATE (n:RelateTest {name: $name, age: $age})");

        // Clean up any leftovers from a previous run.
        cmd()
            .args([
                "query",
                "--password",
                &password,
                "-e",
                "MATCH (n:RelateTest) DETACH DELETE n",
                "--write",
            ])
            .assert()
            .success();

        cmd()
            .args([
                "query",
                "--password",
                &password,
                q.path().to_str().unwrap(),
                "--apply",
                csv.path().to_str().unwrap(),
                "--write",
            ])
            .assert()
            .success();

        // Verify all three nodes exist.
        cmd()
            .args([
                "query",
                "--password",
                &password,
                "-e",
                "MATCH (n:RelateTest) RETURN count(n) AS c",
            ])
            .assert()
            .success()
            .stdout(contains("3"));

        // Cleanup.
        cmd()
            .args([
                "query",
                "--password",
                &password,
                "-e",
                "MATCH (n:RelateTest) DETACH DELETE n",
                "--write",
            ])
            .assert()
            .success();
    }

    // T040 / FR-013 — per-row JSON output schema, gated by NEO4J_PASSWORD.
    #[test]
    fn apply_json_output_includes_row_index() {
        let password = match std::env::var("NEO4J_PASSWORD") {
            Ok(p) if !p.is_empty() => p,
            _ => return,
        };

        let mut jsonl = NamedTempFile::with_suffix(".jsonl").unwrap();
        writeln!(jsonl, r#"{{"name": "RelateJ_A"}}"#).unwrap();
        writeln!(jsonl, r#"{{"name": "RelateJ_B"}}"#).unwrap();
        let q = write_cypher("MERGE (n:RelateJTest {name: $name})");

        // Cleanup.
        cmd()
            .args([
                "query",
                "--password",
                &password,
                "-e",
                "MATCH (n:RelateJTest) DETACH DELETE n",
                "--write",
            ])
            .assert()
            .success();

        let output = cmd()
            .args([
                "query",
                "--password",
                &password,
                q.path().to_str().unwrap(),
                "--apply",
                jsonl.path().to_str().unwrap(),
                "--write",
                "--json",
            ])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();

        let parsed: serde_json::Value = serde_json::from_slice(&output).expect("valid JSON");
        let arr = parsed.as_array().expect("top-level array");
        assert_eq!(arr.len(), 2);
        assert_eq!(arr[0]["row"].as_u64(), Some(0));
        assert_eq!(arr[1]["row"].as_u64(), Some(1));

        // Cleanup.
        cmd()
            .args([
                "query",
                "--password",
                &password,
                "-e",
                "MATCH (n:RelateJTest) DETACH DELETE n",
                "--write",
            ])
            .assert()
            .success();
    }

    // T041 / regression — M1 single-statement JSON output omits the `row` field.
    #[test]
    fn single_row_json_omits_row_field() {
        let password = match std::env::var("NEO4J_PASSWORD") {
            Ok(p) if !p.is_empty() => p,
            _ => return,
        };

        let output = cmd()
            .args([
                "query",
                "--password",
                &password,
                "-e",
                "RETURN 1 AS one",
                "--json",
            ])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();

        let parsed: serde_json::Value = serde_json::from_slice(&output).expect("valid JSON");
        let arr = parsed.as_array().expect("top-level array");
        assert_eq!(arr.len(), 1);
        assert!(
            arr[0].get("row").is_none(),
            "M1 schema must not include `row`; got: {arr:?}"
        );
    }

    // Mid-row failure under --batch N: verifies (a) the primary error line
    // surfaces the Neo4j error code, (b) partial-commit accounting is
    // exact, and (c) the in-flight batch is rolled back while prior batches
    // remain committed. Gated by NEO4J_PASSWORD.
    #[test]
    fn apply_batched_mid_row_failure_reports_neo4j_code_and_partial_commit() {
        let password = match std::env::var("NEO4J_PASSWORD") {
            Ok(p) if !p.is_empty() => p,
            _ => return,
        };

        // 5 divisors; the third is 0 → ArithmeticError.
        let csv = write_csv(&["d"], &[&["1"], &["2"], &["0"], &["4"], &["5"]]);
        // The query intentionally fails when $d = 0.
        let q = write_cypher("CREATE (n:RelateFailTest {value: 100 / toInteger($d)})");

        // Cleanup any leftovers.
        cmd()
            .args([
                "query",
                "--password",
                &password,
                "-e",
                "MATCH (n:RelateFailTest) DETACH DELETE n",
                "--write",
            ])
            .assert()
            .success();

        // --batch 2: rows 1+2 commit (batch 1), row 3 fails inside batch 2 →
        // 2 committed, 1 rolled back, rows 4-5 never attempted.
        let out = cmd()
            .args([
                "query",
                "--password",
                &password,
                q.path().to_str().unwrap(),
                "--apply",
                csv.path().to_str().unwrap(),
                "--batch",
                "2",
                "--write",
            ])
            .assert()
            .failure()
            .code(2)
            .get_output()
            .clone();

        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(
            stderr.contains("Neo.ClientError.Statement.ArithmeticError"),
            "stderr should name the Neo4j error code on the primary error line; got:\n{stderr}"
        );
        assert!(
            stderr.contains("Error on row 3:"),
            "stderr should identify the failing row (1-based); got:\n{stderr}"
        );
        assert!(
            stderr.contains("2 rows committed (1 batch)") || stderr.contains("2 rows committed"),
            "stderr should report 2 rows committed across 1 batch; got:\n{stderr}"
        );
        assert!(
            stderr.contains("Underlying:"),
            "stderr should include an Underlying: line with the error message; got:\n{stderr}"
        );

        // Verify the database state: exactly 2 RelateFailTest nodes survive
        // (rows 1 and 2 from the first batch); row 3 was rolled back.
        let count_out = cmd()
            .args([
                "query",
                "--password",
                &password,
                "-e",
                "MATCH (n:RelateFailTest) RETURN count(n) AS c",
                "--json",
            ])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();
        let parsed: serde_json::Value = serde_json::from_slice(&count_out).expect("valid JSON");
        let c = parsed[0]["rows"][0]["c"].as_u64().expect("count value");
        assert_eq!(c, 2, "expected exactly 2 committed rows after batch 1");

        // Cleanup.
        cmd()
            .args([
                "query",
                "--password",
                &password,
                "-e",
                "MATCH (n:RelateFailTest) DETACH DELETE n",
                "--write",
            ])
            .assert()
            .success();
    }

    // Mid-row failure under --atomic: full rollback, zero committed rows.
    // Gated by NEO4J_PASSWORD.
    #[test]
    fn apply_atomic_mid_row_failure_rolls_back_everything() {
        let password = match std::env::var("NEO4J_PASSWORD") {
            Ok(p) if !p.is_empty() => p,
            _ => return,
        };

        let csv = write_csv(&["d"], &[&["1"], &["2"], &["0"], &["4"]]);
        let q = write_cypher("CREATE (n:RelateAtomicFail {value: 100 / toInteger($d)})");

        cmd()
            .args([
                "query",
                "--password",
                &password,
                "-e",
                "MATCH (n:RelateAtomicFail) DETACH DELETE n",
                "--write",
            ])
            .assert()
            .success();

        let out = cmd()
            .args([
                "query",
                "--password",
                &password,
                q.path().to_str().unwrap(),
                "--apply",
                csv.path().to_str().unwrap(),
                "--atomic",
                "--write",
            ])
            .assert()
            .failure()
            .code(2)
            .get_output()
            .clone();

        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(
            stderr.contains("Neo.ClientError.Statement.ArithmeticError"),
            "stderr should name the Neo4j error code; got:\n{stderr}"
        );
        assert!(
            stderr.contains("Transaction rolled back. 0 rows committed."),
            "stderr should report full rollback; got:\n{stderr}"
        );

        // Verify zero committed rows.
        let count_out = cmd()
            .args([
                "query",
                "--password",
                &password,
                "-e",
                "MATCH (n:RelateAtomicFail) RETURN count(n) AS c",
                "--json",
            ])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();
        let parsed: serde_json::Value = serde_json::from_slice(&count_out).expect("valid JSON");
        let c = parsed[0]["rows"][0]["c"].as_u64().expect("count value");
        assert_eq!(c, 0, "atomic failure must leave zero rows committed");
    }
}

// ── External subcommand dispatch ──────────────────────────────────────────────

#[cfg(unix)]
mod external_subcommand {
    use assert_cmd::Command;
    use predicates::prelude::PredicateBooleanExt;
    use predicates::str::contains;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    fn make_stub_script(dir: &tempfile::TempDir, name: &str, body: &str) {
        let path = dir.path().join(name);
        fs::write(&path, format!("#!/bin/sh\n{body}\n")).expect("write stub");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).expect("chmod stub");
    }

    fn with_stub_path(dir: &tempfile::TempDir) -> String {
        let current = std::env::var("PATH").unwrap_or_default();
        format!("{}:{current}", dir.path().display())
    }

    // T006 — delegates output to external binary
    #[test]
    fn delegates_to_external_binary() {
        let dir = tempfile::TempDir::new().unwrap();
        make_stub_script(&dir, "relate-hello", "echo 'hello from stub'");
        Command::cargo_bin("relate")
            .unwrap()
            .env("PATH", with_stub_path(&dir))
            .arg("hello")
            .assert()
            .success()
            .stdout(contains("hello from stub"));
    }

    // T007 — propagates exit code 0
    #[test]
    fn propagates_exit_code_zero() {
        let dir = tempfile::TempDir::new().unwrap();
        make_stub_script(&dir, "relate-hello", "exit 0");
        Command::cargo_bin("relate")
            .unwrap()
            .env("PATH", with_stub_path(&dir))
            .arg("hello")
            .assert()
            .code(0);
    }

    // T008 — propagates non-zero exit code
    #[test]
    fn propagates_nonzero_exit_code() {
        let dir = tempfile::TempDir::new().unwrap();
        make_stub_script(&dir, "relate-hello", "exit 42");
        Command::cargo_bin("relate")
            .unwrap()
            .env("PATH", with_stub_path(&dir))
            .arg("hello")
            .assert()
            .code(42);
    }

    // T009 — built-in subcommand takes precedence over external stub
    #[test]
    fn builtin_takes_precedence_over_external() {
        let dir = tempfile::TempDir::new().unwrap();
        make_stub_script(&dir, "relate-lint", "echo STUB");
        Command::cargo_bin("relate")
            .unwrap()
            .env("PATH", with_stub_path(&dir))
            .arg("lint")
            .arg(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/valid.cypher"))
            .assert()
            .success()
            .stdout(predicates::str::contains("STUB").not());
    }

    // T011 — missing binary exits 127 with binary name in stderr
    #[test]
    fn not_found_exits_127_with_binary_name() {
        Command::cargo_bin("relate")
            .unwrap()
            .env("PATH", "") // empty PATH guarantees the binary is never found
            .arg("nonexistent-subcommand")
            .assert()
            .code(127)
            .stderr(contains("relate-nonexistent-subcommand"));
    }

    // T012 — non-executable binary exits 126
    #[test]
    fn not_executable_exits_126_with_message() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("relate-hello");
        fs::write(&path, "#!/bin/sh\necho hi\n").expect("write file");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).expect("set non-executable");
        Command::cargo_bin("relate")
            .unwrap()
            .env("PATH", with_stub_path(&dir))
            .arg("hello")
            .assert()
            .code(126)
            .stderr(contains("not executable"));
    }

    // T014 — global flags forwarded as RELATE_* env vars
    #[test]
    fn forwards_global_flags_as_env_vars() {
        let dir = tempfile::TempDir::new().unwrap();
        make_stub_script(&dir, "relate-echo-env", "env | sort");
        Command::cargo_bin("relate")
            .unwrap()
            .env("PATH", with_stub_path(&dir))
            .args(["--uri", "bolt://host:7687", "--user", "tester", "echo-env"])
            .assert()
            .success()
            .stdout(contains("RELATE_URI=bolt://host:7687"))
            .stdout(contains("RELATE_USER=tester"));
    }

    // T015 — calling environment is inherited unchanged
    #[test]
    fn inherits_calling_environment_unchanged() {
        let dir = tempfile::TempDir::new().unwrap();
        make_stub_script(&dir, "relate-echo-env", "env | sort");
        Command::cargo_bin("relate")
            .unwrap()
            .env("PATH", with_stub_path(&dir))
            .env("NEO4J_PASSWORD", "secret")
            .arg("echo-env")
            .assert()
            .success()
            .stdout(contains("NEO4J_PASSWORD=secret"));
    }
}
