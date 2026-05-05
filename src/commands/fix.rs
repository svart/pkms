use crate::config::Config;
use crate::graph::Graph;
use crate::output::OutputContext;
use anyhow::Result;
use serde::Serialize;

#[derive(Serialize)]
pub struct FixOutput {
    pub broken_uuid: String,
    pub replacement_uuid: String,
    pub replacement_title: String,
    pub files_affected: Vec<String>,
    pub total_replacements: usize,
    pub applied: bool,
}

fn validate_uuid(s: &str) -> Result<String> {
    Ok(uuid::Uuid::parse_str(s)
        .map_err(|_| anyhow::anyhow!("Invalid UUID format: {s}"))?
        .to_string())
}

fn print_fix_output(ctx: &OutputContext, output: &FixOutput, apply: bool) -> Result<()> {
    if ctx.is_json() {
        ctx.print_json(output)?;
    } else {
        if apply {
            println!(
                "Fixed {} broken link(s) in {} file(s):",
                output.total_replacements,
                output.files_affected.len()
            );
        } else {
            println!(
                "Would fix {} broken link(s) in {} file(s):",
                output.total_replacements,
                output.files_affected.len()
            );
            println!("  Broken UUID: {}", output.broken_uuid);
            println!(
                "  Replace with: {} ({})",
                output.replacement_title, output.replacement_uuid
            );
            println!("  (use --apply to apply)");
        }
        for f in &output.files_affected {
            println!("  {f}");
        }
    }

    Ok(())
}

fn find_and_replace_links(
    db_root: &std::path::Path,
    broken_str: &str,
    replacement_uuid: &str,
    apply: bool,
) -> Result<(Vec<String>, usize)> {
    let mut files_affected = Vec::new();
    let mut total_replacements = 0;

    for entry in walkdir::WalkDir::new(db_root)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| !e.file_name().to_string_lossy().starts_with('.'))
    {
        let Ok(entry) = entry else { continue };
        if !entry.file_type().is_file() || entry.path().extension().is_none_or(|e| e != "org") {
            continue;
        }

        let path = entry.path();
        let Ok(content) = std::fs::read_to_string(path) else {
            continue;
        };

        let count = content.matches(broken_str).count();
        if count > 0 {
            files_affected.push(path.to_string_lossy().to_string());
            total_replacements += count;

            if apply {
                let new_content = content.replace(broken_str, replacement_uuid);
                std::fs::write(path, &new_content)?;
            }
        }
    }

    Ok((files_affected, total_replacements))
}

pub fn run(
    config: &Config,
    ctx: &OutputContext,
    broken_uuid: &str,
    target: &str,
    apply: bool,
    db_cli: Option<&std::path::Path>,
) -> Result<()> {
    let graph = Graph::load(config, db_cli)?;
    let db_root = config.resolve_db_root(db_cli)?;

    let broken = validate_uuid(broken_uuid)?;
    let target_uuid = validate_uuid(target)?;

    let (replacement_uuid, replacement_title) = graph
        .nodes
        .get(&target_uuid)
        .map(|n| (n.uuid.clone(), n.title.clone()))
        .ok_or_else(|| anyhow::anyhow!("Replacement UUID not found in database: {target}"))?;

    let (files_affected, total_replacements) =
        find_and_replace_links(&db_root, &broken, &replacement_uuid, apply)?;

    let output = FixOutput {
        broken_uuid: broken.clone(),
        replacement_uuid: replacement_uuid.clone(),
        replacement_title,
        files_affected: files_affected.clone(),
        total_replacements,
        applied: apply,
    };

    print_fix_output(ctx, &output, apply)
}
