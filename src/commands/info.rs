use crate::config::{Config, ConfigInfo};
use anyhow::Result;
use serde::Serialize;

#[derive(Serialize)]
pub struct InfoOutput {
    pub config: ConfigInfo,
    pub config_path: String,
    pub cli_overrides: CliOverrides,
}

#[derive(Serialize)]
pub struct CliOverrides {
    pub db_override: bool,
}

pub fn run(config: &Config, json: bool, db_cli: Option<&std::path::Path>) -> Result<()> {
    let db_root = config.resolve_db_root(db_cli)?;
    let new_notes_dir = config.resolve_new_notes_dir(&db_root);
    let info = config.resolved_info(&db_root, &new_notes_dir);

    if json {
        let output = InfoOutput {
            config: info,
            config_path: dirs::config_dir()
                .unwrap_or_default()
                .join("pkms.toml")
                .to_string_lossy()
                .to_string(),
            cli_overrides: CliOverrides {
                db_override: db_cli.is_some(),
            },
        };
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        println!("pkms configuration");
        println!(
            "  config file:  ~/.config/pkms.toml ({})",
            if info.has_config_file {
                "found"
            } else {
                "not found"
            }
        );
        println!("  db_root:      {}", info.db_root.display());
        println!("  new_notes:    {}", info.new_notes_dir.display());
        if !info.ignore_patterns.is_empty() {
            println!("  ignore:       {}", info.ignore_patterns.join(", "));
        }
        if db_cli.is_some() {
            println!("  (db_root overridden via --db flag)");
        }
    }

    Ok(())
}
