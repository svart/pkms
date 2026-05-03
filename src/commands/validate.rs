use crate::config::Config;
use crate::graph::Graph;
use crate::output::OutputContext;
use crate::parser::Link;
use anyhow::Result;
use serde::Serialize;
use std::path::Path;

#[derive(Serialize)]
pub struct ValidateOutput {
    pub uuid: String,
    pub title: String,
    pub path: String,
    pub filetags: Vec<String>,
    pub aliases: Vec<String>,
    pub refs: Vec<String>,
    pub headings: usize,
    pub outgoing: usize,
    pub incoming: usize,
    pub outgoing_internal: usize,
    pub broken_internal: Vec<String>,
    pub broken_files: Vec<String>,
    pub backlinks: Vec<BacklinkEntry>,
    pub issues: Vec<String>,
    pub healthy: bool,
}

#[derive(Serialize)]
pub struct BacklinkEntry {
    pub uuid: String,
    pub title: String,
}

fn print_validate_json(
    ctx: &OutputContext,
    node: crate::graph::Node,
    incoming: &[String],
    broken_internal: Vec<String>,
    broken_files: Vec<String>,
    backlink_entries: Vec<BacklinkEntry>,
    issues: Vec<String>,
) -> Result<()> {
    let incoming_len = incoming.len();
    let healthy = issues.is_empty();
    let output = ValidateOutput {
        uuid: node.uuid,
        title: node.title,
        path: node.path.to_string_lossy().to_string(),
        filetags: node.filetags,
        aliases: node.aliases,
        refs: node.refs,
        headings: node.headings_count,
        outgoing: node.outgoing.len(),
        incoming: incoming_len,
        outgoing_internal: node
            .outgoing
            .iter()
            .filter(|l| matches!(l, crate::parser::Link::Internal(_)))
            .count(),
        broken_internal,
        broken_files,
        backlinks: backlink_entries,
        issues,
        healthy,
    };
    ctx.print_json(&output)
}

fn print_validate_text(
    node: &crate::graph::Node,
    broken_internal: &[String],
    broken_files: &[String],
    incoming: &[String],
    _backlink_entries: &[BacklinkEntry],
    issues: &[String],
) {
    let outgoing_internal_len = node
        .outgoing
        .iter()
        .filter(|l| matches!(l, crate::parser::Link::Internal(_)))
        .count();
    let healthy = issues.is_empty();
    println!("Note: {}", node.title);
    println!("  UUID:   {}", node.uuid);
    println!("  Path:   {}", node.path.display());
    if !node.filetags.is_empty() {
        println!("  Tags:   {}", node.filetags.join(", "));
    }
    if !node.aliases.is_empty() {
        println!("  Aliases: {}", node.aliases.join(", "));
    }
    if !node.refs.is_empty() {
        println!("  Refs:   {}", node.refs.join(", "));
    }
    println!("  Headings: {}", node.headings_count);
    println!();
    println!("Links:");
    println!(
        "  Outgoing: {} ({} internal)",
        node.outgoing.len(),
        outgoing_internal_len,
    );
    println!("  Incoming: {}", incoming.len());
    println!(
        "  Broken:   {} internal, {} file",
        broken_internal.len(),
        broken_files.len()
    );

    if !broken_internal.is_empty() {
        println!();
        println!("Broken internal links:");
        for uuid in broken_internal {
            println!("  -> {uuid}");
        }
    }

    if !broken_files.is_empty() {
        println!();
        println!("Broken file links:");
        for path in broken_files {
            println!("  -> {path}");
        }
    }

    if !issues.is_empty()
        && issues
            .iter()
            .any(|i| i.starts_with("Invalid") || i.starts_with("Missing"))
    {
        println!();
        for i in issues {
            if i.starts_with("Invalid") || i.starts_with("Missing") {
                println!("Issue: {i}");
            }
        }
    }

    println!();
    if healthy {
        println!("Status: healthy");
    } else {
        println!("Status: {} issue(s)", issues.len());
    }
}

pub fn run(
    config: &Config,
    ctx: &OutputContext,
    target: Option<&str>,
    db_cli: Option<&std::path::Path>,
) -> Result<()> {
    let target = target.ok_or_else(|| anyhow::anyhow!("No target specified. Provide a target"))?;

    let graph = Graph::load(config, db_cli)?;

    let node = graph.resolve_target(target)?.clone();
    let mut issues = Vec::new();

    let uuid_parts: Vec<&str> = node.uuid.split('-').collect();
    if uuid_parts.len() != 5 {
        issues.push(format!("Invalid UUID format: {}", node.uuid));
    }

    let content = std::fs::read_to_string(&node.path).unwrap_or_default();
    if !content.contains("#+title:") {
        issues.push("Missing #+title: property".to_string());
    }

    let mut broken_internal = Vec::new();
    let mut broken_files = Vec::new();

    for link in &node.outgoing {
        match link {
            Link::Internal(uuid) if !graph.nodes.contains_key(uuid) => {
                broken_internal.push(uuid.clone());
            }
            Link::File(path_str) => {
                let file_path = Path::new(path_str);
                let db_root = config.resolve_db_root(db_cli).ok();
                let exists = if file_path.is_absolute() {
                    file_path.exists()
                } else if let Some(ref root) = db_root {
                    root.join(file_path).exists()
                } else {
                    false
                };
                if !exists {
                    broken_files.push(path_str.clone());
                }
            }
            _ => {}
        }
    }

    let incoming = graph.backlinks.get(&node.uuid).cloned().unwrap_or_default();

    let backlink_entries: Vec<BacklinkEntry> = incoming
        .iter()
        .filter_map(|uuid| {
            graph.nodes.get(uuid).map(|n| BacklinkEntry {
                uuid: n.uuid.clone(),
                title: n.title.clone(),
            })
        })
        .collect();

    if !broken_internal.is_empty() {
        issues.push(format!("{} broken internal link(s)", broken_internal.len()));
    }
    if !broken_files.is_empty() {
        issues.push(format!("{} broken file link(s)", broken_files.len()));
    }

    if ctx.is_json() {
        print_validate_json(
            ctx,
            node,
            &incoming,
            broken_internal,
            broken_files,
            backlink_entries,
            issues,
        )?;
    } else {
        print_validate_text(
            &node,
            &broken_internal,
            &broken_files,
            &incoming,
            &backlink_entries,
            &issues,
        );
    }

    Ok(())
}
