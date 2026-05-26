use crate::config::{ConfigInfo, ResolvedConfig};
use crate::output::OutputContext;
use anyhow::Result;
use serde::Serialize;

#[derive(Serialize)]
pub struct InfoOutput {
    pub config: ConfigInfo,
    pub config_path: String,
}

pub fn run(config: &ResolvedConfig, ctx: &OutputContext) -> Result<()> {
    let info = config.resolved_info();

    if ctx.is_json() {
        let output = InfoOutput {
            config: info,
            config_path: dirs::config_dir()
                .unwrap_or_default()
                .join("pkms.toml")
                .to_string_lossy()
                .to_string(),
        };
        ctx.print_json(&output)?;
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
        println!("  daily_notes:  {}", info.daily_notes_dir.display());
        if !info.ignore_patterns.is_empty() {
            println!("  ignore:       {}", info.ignore_patterns.join(", "));
        }
    }

    Ok(())
}
