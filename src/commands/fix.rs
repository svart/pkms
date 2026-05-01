use crate::config::Config;
use crate::graph::Graph;
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

pub fn run(
    config: &Config,
    json: bool,
    verbose: bool,
    broken_uuid: &str,
    target: &str,
    apply: bool,
    db_cli: Option<&std::path::Path>,
) -> Result<()> {
    // Load graph to find the replacement note
    let graph = Graph::load(config, db_cli, false)?;
    let db_root = config.resolve_db_root(db_cli)?;

    // Validate broken UUID format
    let broken = if broken_uuid.contains('-') {
        broken_uuid.to_string()
    } else if broken_uuid.len() == 8 {
        // Check existing nodes first
        let matches: Vec<&String> = graph.nodes.keys().filter(|u| u.starts_with(broken_uuid)).collect();
        if matches.len() == 1 {
            matches[0].clone()
        } else if matches.len() > 1 {
            anyhow::bail!("Multiple existing UUIDs match prefix '{}': {:?}", broken_uuid, matches);
        } else {
            // Check broken links (deduplicate by target UUID)
            let mut seen = std::collections::HashSet::new();
            let broken_matches: Vec<&String> = graph.broken_links.iter().filter_map(|(_, tgt)| {
                if tgt.starts_with(broken_uuid) && seen.insert(tgt.as_str()) { Some(tgt) } else { None }
            }).collect();
            if broken_matches.len() == 1 {
                broken_matches[0].clone()
            } else if broken_matches.is_empty() {
                anyhow::bail!("No UUID matches prefix '{}' in nodes or broken links", broken_uuid);
            } else {
                anyhow::bail!("Multiple broken UUIDs match prefix '{}': {:?}", broken_uuid, broken_matches);
            }
        }
    } else {
        anyhow::bail!("Invalid UUID format: {}", broken_uuid);
    };

    // Resolve replacement — support UUID, title, path, or prefix
    let replacement = graph.find_node(target).map(|n| (n.uuid.clone(), n.title.clone()))
        .or_else(|| {
            // Try prefix matching in existing nodes
            let prefix_matches: Vec<&String> = graph.nodes.keys().filter(|u| u.starts_with(target)).collect();
            if prefix_matches.len() == 1 {
                let uuid = prefix_matches[0].clone();
                graph.nodes.get(&uuid).map(|n| (n.uuid.clone(), n.title.clone()))
            } else {
                None
            }
        });
    let (replacement_uuid, replacement_title) = match replacement {
        Some((u, t)) => (u, t),
        None => anyhow::bail!("Replacement target not found: {}", target),
    };

    // Find all files containing the broken UUID
    let broken_str = &broken;
    let mut files_affected = Vec::new();
    let mut total_replacements = 0;

    for entry in walkdir::WalkDir::new(&db_root)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| !e.file_name().to_string_lossy().starts_with('.'))
    {
        let entry = match entry {
            Ok(e) => e,
            _ => continue,
        };
        if !entry.file_type().is_file() || entry.path().extension().map_or(true, |e| e != "org") {
            continue;
        }

        let path = entry.path();
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            _ => continue,
        };

        let count = content.matches(broken_str).count();
        if count > 0 {
            files_affected.push(path.to_string_lossy().to_string());
            total_replacements += count;

            if apply {
                let new_content = content.replace(broken_str, &replacement_uuid);
                std::fs::write(path, &new_content)?;
            }
        }
    }

    let output = FixOutput {
        broken_uuid: broken.clone(),
        replacement_uuid: replacement_uuid.clone(),
        replacement_title,
        files_affected: files_affected.clone(),
        total_replacements,
        applied: apply,
    };

    if json {
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        if apply {
            println!("Fixed {} broken link(s) in {} file(s):", total_replacements, files_affected.len());
        } else {
            println!("Would fix {} broken link(s) in {} file(s):", total_replacements, files_affected.len());
            println!("  Broken UUID: {}", broken);
            println!("  Replace with: {} ({})", output.replacement_title, replacement_uuid);
            println!("  (use --apply to apply)");
        }
        for f in &files_affected {
            println!("  {}", f);
        }
        if verbose {
            println!("  ({} replacement(s) total)", total_replacements);
        }
    }

    Ok(())
}
