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

fn find_replacement(graph: &Graph, target: &str) -> Result<(String, String)> {
    let replacement = graph
        .find_node(target)
        .map(|n| (n.uuid.clone(), n.title.clone()))
        .or_else(|| {
            let prefix_matches: Vec<&String> = graph
                .nodes
                .keys()
                .filter(|u| u.starts_with(target))
                .collect();
            if prefix_matches.len() == 1 {
                let uuid = prefix_matches[0].clone();
                graph
                    .nodes
                    .get(&uuid)
                    .map(|n| (n.uuid.clone(), n.title.clone()))
            } else {
                None
            }
        });
    replacement.ok_or_else(|| anyhow::anyhow!("Replacement target not found: {target}"))
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

    let broken = if broken_uuid.contains('-') {
        broken_uuid.to_string()
    } else if broken_uuid.len() == 8 {
        let matches: Vec<&String> = graph
            .nodes
            .keys()
            .filter(|u| u.starts_with(broken_uuid))
            .collect();
        match matches.len().cmp(&1) {
            std::cmp::Ordering::Equal => matches[0].clone(),
            std::cmp::Ordering::Greater => {
                anyhow::bail!("Multiple existing UUIDs match prefix '{broken_uuid}': {matches:?}")
            }
            std::cmp::Ordering::Less => {
                let mut seen = std::collections::HashSet::new();
                let broken_matches: Vec<&String> = graph
                    .broken_links
                    .iter()
                    .filter_map(|(_, tgt)| {
                        if tgt.starts_with(broken_uuid) && seen.insert(tgt.as_str()) {
                            Some(tgt)
                        } else {
                            None
                        }
                    })
                    .collect();
                if broken_matches.len() == 1 {
                    broken_matches[0].clone()
                } else if broken_matches.is_empty() {
                    anyhow::bail!(
                        "No UUID matches prefix '{broken_uuid}' in nodes or broken links"
                    );
                } else {
                    anyhow::bail!(
                        "Multiple broken UUIDs match prefix '{broken_uuid}': {broken_matches:?}"
                    );
                }
            }
        }
    } else {
        anyhow::bail!("Invalid UUID format: {broken_uuid}");
    };

    let (replacement_uuid, replacement_title) = find_replacement(&graph, target)?;

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
