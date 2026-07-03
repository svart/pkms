use crate::command_context::CommandContext;
use crate::config::{ConfigInfo, ResolvedConfig};
use anyhow::Result;
use serde::Serialize;

#[derive(Serialize)]
pub struct InfoOutput {
    pub config: ConfigInfo,
    pub config_path: String,
}

pub fn build_output(config: &ResolvedConfig) -> InfoOutput {
    let info = config.resolved_info();
    InfoOutput {
        config: info,
        config_path: dirs::config_dir()
            .unwrap_or_default()
            .join("pkms.toml")
            .to_string_lossy()
            .to_string(),
    }
}

pub fn render_text(output: &InfoOutput) -> String {
    let info = &output.config;
    let mut lines = vec![
        "pkms configuration".to_string(),
        format!(
            "  config file:  ~/.config/pkms.toml ({})",
            if info.has_config_file {
                "found"
            } else {
                "not found"
            }
        ),
        format!("  db_root:      {}", info.db_root.display()),
        format!("  new_notes:    {}", info.new_notes_dir.display()),
        format!("  daily_notes:  {}", info.daily_notes_dir.display()),
    ];
    if !info.ignore_patterns.is_empty() {
        lines.push(format!(
            "  ignore:       {}",
            info.ignore_patterns.join(", ")
        ));
    }
    lines.join("\n")
}

pub fn run(ctx: &CommandContext<'_>) -> Result<()> {
    let output = build_output(ctx.config());
    if ctx.output().is_structured() {
        ctx.output().print_structured(&output)?;
    } else {
        println!("{}", render_text(&output));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ResolvedConfig;
    use std::path::PathBuf;

    fn config() -> ResolvedConfig {
        ResolvedConfig {
            db_root: PathBuf::from("/db"),
            new_notes_dir: Some(PathBuf::from("notes")),
            daily_notes_dir: Some(PathBuf::from("daily")),
            ignore_patterns: Some(vec!["*.bak".to_string()]),
            columns: None,
            tasks: None,
            agenda: None,
            todoist: None,
            ssh: None,
        }
    }

    #[test]
    fn builds_info_output_from_resolved_config() {
        let output = build_output(&config());
        assert_eq!(output.config.db_root, PathBuf::from("/db"));
        assert_eq!(output.config.new_notes_dir, PathBuf::from("/db/notes"));
        assert_eq!(output.config.daily_notes_dir, PathBuf::from("/db/daily"));
        assert_eq!(output.config.ignore_patterns, vec!["*.bak"]);
    }

    #[test]
    fn renders_text_without_printing() {
        let output = build_output(&config());
        let text = render_text(&output);
        assert!(text.contains("pkms configuration"));
        assert!(text.contains("  db_root:      /db"));
        assert!(text.contains("  ignore:       *.bak"));
    }
}
