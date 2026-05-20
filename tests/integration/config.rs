use super::*;
use std::process::Command;

fn parse_json_stdout(output: std::process::Output) -> serde_json::Value {
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    assert!(
        output.status.success(),
        "command failed\nstdout: {stdout}\nstderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_str(stdout.trim()).unwrap_or_else(|e| panic!("invalid JSON: {stdout}\n{e}"))
}

fn write_config(config_home: &std::path::Path, db_root: &std::path::Path) {
    std::fs::write(
        config_home.join("pkms.toml"),
        format!("db_root = \"{}\"\n", db_root.display()),
    )
    .unwrap();
}

#[test]
fn test_db_root_from_config_file() {
    let (_db_dir, root) = setup_db();
    let config_home = tempfile::tempdir().unwrap();
    write_config(config_home.path(), &root);

    let output = Command::new(pkms_binary())
        .args(["--output-format", "json", "stats"])
        .env("XDG_CONFIG_HOME", config_home.path())
        .env_remove("PKMS_DB_ROOT")
        .output()
        .unwrap();
    let v = parse_json_stdout(output);

    assert!(v["total_notes"].as_u64().unwrap_or(0) > 0);
}

#[test]
fn test_env_db_root_overrides_config_file() {
    let (_configured_dir, configured_root) = setup_empty_db();
    let (_env_dir, env_root) = setup_db();
    let config_home = tempfile::tempdir().unwrap();
    write_config(config_home.path(), &configured_root);

    let output = Command::new(pkms_binary())
        .args(["--output-format", "json", "stats"])
        .env("XDG_CONFIG_HOME", config_home.path())
        .env("PKMS_DB_ROOT", &env_root)
        .output()
        .unwrap();
    let v = parse_json_stdout(output);

    assert!(v["total_notes"].as_u64().unwrap_or(0) > 0);
}

#[test]
fn test_cli_db_root_overrides_env_and_config_file() {
    let (_configured_dir, configured_root) = setup_empty_db();
    let (_env_dir, env_root) = setup_empty_db();
    let (_cli_dir, cli_root) = setup_db();
    let config_home = tempfile::tempdir().unwrap();
    write_config(config_home.path(), &configured_root);

    let output = Command::new(pkms_binary())
        .args([
            "--db",
            cli_root.to_str().unwrap(),
            "--output-format",
            "json",
            "stats",
        ])
        .env("XDG_CONFIG_HOME", config_home.path())
        .env("PKMS_DB_ROOT", &env_root)
        .output()
        .unwrap();
    let v = parse_json_stdout(output);

    assert!(v["total_notes"].as_u64().unwrap_or(0) > 0);
}
