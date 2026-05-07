use crate::cli::OutputFormat;
use crate::config::Config;
use crate::graph::Graph;
use crate::output::OutputContext;
use crate::parser::{Link, validate_filetags_format};
use crate::util;
use anyhow::Result;
use serde::Serialize;
use std::path::Path;

#[derive(Serialize)]
pub struct ValidateOutput {
    pub uuid: String,
    pub title: String,
    pub path: String,
    pub filetags: Vec<String>,
    pub categories: Vec<String>,
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

fn build_validate_output(
    node: &crate::graph::Node,
    incoming: &[String],
    broken_internal: Vec<String>,
    broken_files: Vec<String>,
    backlink_entries: Vec<BacklinkEntry>,
    issues: Vec<String>,
) -> ValidateOutput {
    let healthy = issues.is_empty();
    ValidateOutput {
        uuid: node.uuid.clone(),
        title: node.title.clone(),
        path: node.path.to_string_lossy().to_string(),
        filetags: node.filetags.clone(),
        categories: node.categories.clone(),
        aliases: node.aliases.clone(),
        refs: node.refs.clone(),
        headings: node.headings_count,
        outgoing: node.outgoing.len(),
        incoming: incoming.len(),
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
    }
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
    if !node.categories.is_empty() {
        println!("  Cats:   {}", node.categories.join(", "));
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

fn validate_one(graph: &Graph, target: &str, db_root: &Path) -> Result<ValidateOutput> {
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

    for (raw, reason) in validate_filetags_format(&content) {
        issues.push(format!("Invalid filetags format '{}': {}", raw, reason));
    }

    let mut broken_internal = Vec::new();
    let mut broken_files = Vec::new();

    for link in &node.outgoing {
        match link {
            Link::Internal(uuid) if !graph.nodes.contains_key(uuid) => {
                broken_internal.push(uuid.clone());
            }
            Link::File(path_str) => {
                let expanded = if path_str.starts_with('~') {
                    if let Some(home) = dirs::home_dir() {
                        path_str.replacen('~', &home.to_string_lossy(), 1)
                    } else {
                        path_str.clone()
                    }
                } else {
                    path_str.clone()
                };
                let clean_path = expanded.split("::").next().unwrap_or(&expanded);
                let file_path = Path::new(clean_path);
                let full_path = if file_path.is_absolute() {
                    file_path.to_path_buf()
                } else {
                    db_root.join(file_path)
                };
                if !full_path.exists() {
                    broken_files.push(path_str.clone());
                    continue;
                }
                if let Some(line_spec) = expanded.split_once("::").map(|x| x.1)
                    && !line_spec.is_empty()
                {
                    if let Ok(content) = std::fs::read_to_string(&full_path) {
                        if !content.lines().any(|l| l.contains(line_spec)) {
                            broken_files.push(path_str.clone());
                        }
                    } else {
                        broken_files.push(path_str.clone());
                    }
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

    Ok(build_validate_output(
        &node,
        &incoming,
        broken_internal,
        broken_files,
        backlink_entries,
        issues,
    ))
}

pub fn run(
    config: &Config,
    ctx: &OutputContext,
    target: Option<&str>,
    from_stdin: bool,
    db_cli: Option<&std::path::Path>,
) -> Result<()> {
    let targets: Vec<String> = if from_stdin || (target.is_none() && util::is_stdin_piped()) {
        util::read_stdin_ndjson()?
    } else if let Some(t) = target {
        vec![t.to_string()]
    } else {
        anyhow::bail!(
            "No target specified and no stdin pipe detected. Provide a target or use --from-stdin."
        );
    };

    let graph = Graph::load(config, db_cli)?;
    let db_root = config.resolve_db_root(db_cli)?;

    match ctx.format {
        OutputFormat::Text => {
            for t in &targets {
                let node = graph.resolve_target(t)?.clone();
                let incoming = graph.backlinks.get(&node.uuid).cloned().unwrap_or_default();
                let output = validate_one(&graph, t, &db_root)?;
                print_validate_text(
                    &node,
                    &output.broken_internal,
                    &output.broken_files,
                    &incoming,
                    &output.backlinks,
                    &output.issues,
                );
                if targets.len() > 1 {
                    println!();
                }
            }
        }
        OutputFormat::Json => {
            let mut all_outputs = Vec::new();
            for t in &targets {
                all_outputs.push(validate_one(&graph, t, &db_root)?);
            }
            if all_outputs.len() == 1 {
                ctx.print_json(&all_outputs[0])?;
            } else {
                ctx.print_json(&all_outputs)?;
            }
        }
        OutputFormat::Ndjson => {
            for t in &targets {
                let output = validate_one(&graph, t, &db_root)?;
                println!("{}", serde_json::to_string(&output)?);
            }
        }
    }

    Ok(())
}
