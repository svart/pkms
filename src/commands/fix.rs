use crate::config::Config;
use crate::graph::Graph;
use crate::output::OutputContext;
use anyhow::Result;
use regex::Regex;
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

fn print_fix_output(ctx: &OutputContext, output: &FixOutput) -> Result<()> {
    if ctx.is_json() {
        ctx.print_json(output)?;
    } else {
        if output.applied {
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

    let escaped = regex::escape(broken_str);
    let re = Regex::new(&format!(r"(id:){escaped}")).unwrap();

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

        let count = re.find_iter(&content).count();
        if count > 0 {
            files_affected.push(path.to_string_lossy().to_string());
            total_replacements += count;

            if apply {
                let new_content = re.replace_all(&content, |caps: &regex::Captures| {
                    format!("{}{}", &caps[1], replacement_uuid)
                });
                std::fs::write(path, new_content.as_ref())?;
            }
        }
    }

    Ok((files_affected, total_replacements))
}

pub struct FixOptions {
    pub broken_uuid: String,
    pub target_uuid: String,
    pub apply: bool,
}

pub fn run(config: &Config, ctx: &OutputContext, opts: &FixOptions) -> Result<()> {
    let graph = Graph::load(config)?;
    let db_root = config.resolved_db_root()?;

    let (replacement_uuid, replacement_title) = graph
        .nodes
        .get(&opts.target_uuid)
        .map(|n| (n.uuid.clone(), n.title.clone()))
        .ok_or_else(|| {
            anyhow::anyhow!(
                "Replacement UUID not found in database: {}",
                opts.target_uuid
            )
        })?;

    let (files_affected, total_replacements) =
        find_and_replace_links(db_root, &opts.broken_uuid, &replacement_uuid, opts.apply)?;

    let output = FixOutput {
        broken_uuid: opts.broken_uuid.clone(),
        replacement_uuid: replacement_uuid.clone(),
        replacement_title,
        files_affected: files_affected.clone(),
        total_replacements,
        applied: opts.apply,
    };

    print_fix_output(ctx, &output)
}
