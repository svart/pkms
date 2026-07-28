use super::{pkms_binary, setup_db, setup_test_config_home};
use std::process::Command;

#[test]
fn test_pkms_log_writes_to_stderr_without_polluting_json_stdout() {
    let (_dir, root) = setup_db();
    let config_home = setup_test_config_home();
    let output = Command::new(pkms_binary())
        .args([
            "--db",
            root.to_str().unwrap(),
            "--output-format",
            "json",
            "info",
        ])
        .env("PKMS_LOG", "debug")
        .env("XDG_CONFIG_HOME", config_home.path())
        .env_remove("PKMS_DB_ROOT")
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    serde_json::from_str::<serde_json::Value>(stdout.trim()).unwrap();
    assert!(stderr.contains("dispatching command"), "stderr: {stderr}");
}

#[test]
fn test_pkms_log_json_format_writes_json_to_stderr() {
    let (_dir, root) = setup_db();
    let config_home = setup_test_config_home();
    let output = Command::new(pkms_binary())
        .args(["--db", root.to_str().unwrap(), "info"])
        .env("PKMS_LOG", "debug")
        .env("PKMS_LOG_FORMAT", "json")
        .env("XDG_CONFIG_HOME", config_home.path())
        .env_remove("PKMS_DB_ROOT")
        .output()
        .unwrap();

    assert!(output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    let first = stderr.lines().next().expect("expected log line");
    let event: serde_json::Value = serde_json::from_str(first).unwrap();
    assert_eq!(event["fields"]["message"], "dispatching command");
}

#[test]
fn test_pkms_log_can_target_config_resolution() {
    let (_dir, root) = setup_db();
    let config_home = setup_test_config_home();
    let output = Command::new(pkms_binary())
        .args(["--db", root.to_str().unwrap(), "info"])
        .env("PKMS_LOG", "pkms::config=debug")
        .env("XDG_CONFIG_HOME", config_home.path())
        .env_remove("PKMS_DB_ROOT")
        .output()
        .unwrap();

    assert!(output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("config resolved"), "stderr: {stderr}");
    assert!(
        stderr.contains("db_root_source=\"cli\""),
        "stderr: {stderr}"
    );
}

#[test]
fn test_pkms_log_can_target_task_collection() {
    let (_dir, root) = setup_db();
    let config_home = setup_test_config_home();
    let output = Command::new(pkms_binary())
        .args(["--db", root.to_str().unwrap(), "task", "list"])
        .env("PKMS_LOG", "pkms::commands::task=debug")
        .env("XDG_CONFIG_HOME", config_home.path())
        .env_remove("PKMS_DB_ROOT")
        .output()
        .unwrap();

    assert!(output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("running task list"), "stderr: {stderr}");
}
