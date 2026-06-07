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
fn lists_all_agents_by_default() {
    let mut cmd = Command::cargo_bin("agent-sessions").unwrap();

    cmd.arg("list")
        .assert()
        .success()
        .stdout(predicate::str::contains("AGENT\tSESSION_ID\tMESSAGES"));
}

#[test]
fn lists_all_agents_when_command_is_omitted() {
    let mut cmd = Command::cargo_bin("agent-sessions").unwrap();

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("AGENT\tSESSION_ID\tMESSAGES"));
}

#[test]
fn accepts_list_options_when_command_is_omitted() {
    let mut cmd = Command::cargo_bin("agent-sessions").unwrap();

    cmd.args(["--agent", "codex", "--output", "json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"sessions\""));
}

#[test]
fn rejects_agent_and_all_agents_together() {
    let mut cmd = Command::cargo_bin("agent-sessions").unwrap();

    cmd.args(["list", "--agent", "pi", "--all-agents"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("cannot be used with"));
}

#[test]
fn allows_all_agents_for_path() {
    let mut cmd = Command::cargo_bin("agent-sessions").unwrap();

    cmd.args(["list", "--all-agents", "--path", "."])
        .assert()
        .success()
        .stdout(predicate::str::contains("AGENT\tSESSION_ID\tMESSAGES"));
}

#[test]
fn allows_all_paths() {
    let mut cmd = Command::cargo_bin("agent-sessions").unwrap();

    cmd.args(["list", "--agent", "codex", "--all-paths"])
        .assert()
        .success()
        .stdout(predicate::str::contains("AGENT\tSESSION_ID\tMESSAGES"));
}

#[test]
fn rejects_path_and_all_paths_together() {
    let mut cmd = Command::cargo_bin("agent-sessions").unwrap();

    cmd.args(["list", "--path", ".", "--all-paths"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("cannot be used with"));
}

#[test]
fn rejects_removed_all_option() {
    let mut cmd = Command::cargo_bin("agent-sessions").unwrap();

    cmd.args(["list", "--all"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("unexpected argument"));
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

#[test]
fn prints_rm_help() {
    let mut cmd = Command::cargo_bin("agent-sessions").unwrap();

    cmd.args(["rm", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--session-id"));
}

#[test]
fn rm_missing_session_fails_without_deleting() {
    let mut cmd = Command::cargo_bin("agent-sessions").unwrap();

    cmd.args([
        "rm",
        "--agent",
        "pi",
        "--session-id",
        "missing-session-id-for-test",
        "--dry-run",
    ])
    .assert()
    .failure()
    .stderr(predicate::str::contains("session not found"));
}
