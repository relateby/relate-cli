use assert_cmd::Command;
use predicates::str::contains;

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
