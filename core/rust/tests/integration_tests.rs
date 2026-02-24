use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn test_diagnostic_basic() {
    let mut cmd = Command::cargo_bin("koad").unwrap();
    cmd.arg("diagnostic");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("--- KoadOS Diagnostic ---"))
        .stdout(predicate::str::contains("Status: OPERATIONAL"));
}

#[test]
fn test_diagnostic_full() {
    let mut cmd = Command::cargo_bin("koad").unwrap();
    cmd.arg("diagnostic").arg("--full");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("--- Full Integration Check ---"))
        .stdout(predicate::str::contains("Bridge skill gemini/remember.py found"))
        .stdout(predicate::str::contains("--- Tool Availability ---"))
        .stdout(predicate::str::contains("[PASS] git is available"));
}

#[test]
fn test_diagnostic_degraded() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path();
    
    let mut cmd = Command::cargo_bin("koad").unwrap();
    cmd.env("KOAD_HOME", home);
    cmd.arg("diagnostic");
    
    // It should fail for koad.json but still run
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("[FAIL] koad.json is missing"))
        .stdout(predicate::str::contains("Status: DEGRADED"));
}

#[test]
fn test_whoami() {
    let mut cmd = Command::cargo_bin("koad").unwrap();
    cmd.arg("whoami");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Persona:"));
}

#[test]
fn test_spec_cycle() {
    let mut cmd = Command::cargo_bin("koad").unwrap();
    cmd.arg("spec").arg("set").arg("Testing Spec").arg("--status").arg("In-Test");
    cmd.assert().success();

    let mut cmd = Command::cargo_bin("koad").unwrap();
    cmd.arg("spec").arg("read");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Testing Spec"))
        .stdout(predicate::str::contains("In-Test"));
}

#[test]
fn test_query_limits() {
    let mut cmd = Command::cargo_bin("koad").unwrap();
    cmd.arg("query").arg("").arg("--limit").arg("2");
    cmd.assert().success();
}
