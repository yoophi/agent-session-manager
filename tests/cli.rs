use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn prints_help() {
    let mut cmd = Command::cargo_bin("agent-sessions").unwrap();

    cmd.arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("agent-sessions"));
}

#[test]
fn lists_claude_sessions_by_default_all_scope() {
    let mut cmd = Command::cargo_bin("agent-sessions").unwrap();

    cmd.args(["list", "--agent", "claude"])
        .assert()
        .success()
        .stdout(predicate::str::contains("AGENT\tSESSION_ID\tMESSAGES"));
}

#[test]
fn rejects_path_and_all_together() {
    let mut cmd = Command::cargo_bin("agent-sessions").unwrap();

    cmd.args(["list", "--agent", "pi", "--all", "--path", "."])
        .assert()
        .failure()
        .stderr(predicate::str::contains("cannot be used with"));
}

#[test]
fn lists_sessions_as_json() {
    let mut cmd = Command::cargo_bin("agent-sessions").unwrap();

    cmd.args(["list", "--agent", "codex", "--output", "json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"sessions\""));
}

#[test]
fn lists_sessions_as_csv() {
    let mut cmd = Command::cargo_bin("agent-sessions").unwrap();

    cmd.args(["list", "--agent", "pi", "--output", "csv"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "agent,session_id,title,cwd,file_path,message_count",
        ));
}
