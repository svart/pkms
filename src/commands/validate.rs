use crate::cli::OutputFormat;
use crate::config::Config;
use crate::graph::{Graph, file_link_target_exists, resolve_file_link_path};
use crate::output::OutputContext;
use crate::parser::{ID_PROPERTY_RE, Link, UUID_FORMAT_RE, validate_filetags_format};
use anyhow::Result;
use serde::Serialize;
use std::collections::HashMap;
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
    pub heading_uuids: Vec<String>,
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

impl From<&crate::graph::Node> for BacklinkEntry {
    fn from(n: &crate::graph::Node) -> Self {
        BacklinkEntry {
            uuid: n.uuid.clone(),
            title: n.title.clone(),
        }
    }
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
        path: node.path.display().to_string(),
        filetags: node.filetags.clone(),
        categories: node.categories.clone(),
        aliases: node.aliases.clone(),
        refs: node.refs.clone(),
        headings: node.headings_count,
        heading_uuids: node.heading_uuids.clone(),
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
    if !node.heading_uuids.is_empty() {
        println!("  Heading UUIDs: {}", node.heading_uuids.join(", "));
    }
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

    if !issues.is_empty() {
        println!();
        for i in issues {
            println!("Issue: {i}");
        }
    }

    println!();
    if healthy {
        println!("Status: healthy");
    } else {
        println!("Status: {} issue(s)", issues.len());
    }
}

fn check_uuid_format(node: &crate::graph::Node, issues: &mut Vec<String>) {
    let uuid_parts: Vec<&str> = node.uuid.split('-').collect();
    if uuid_parts.len() != 5 {
        issues.push(format!("Invalid UUID format: {}", node.uuid));
    }
}

fn check_title_presence(content: &str, issues: &mut Vec<String>) {
    if !content.contains("#+title:") {
        issues.push("Missing #+title: property".to_string());
    }
}

fn check_filetags_formatting(content: &str, issues: &mut Vec<String>) {
    for (raw, reason) in validate_filetags_format(content) {
        issues.push(format!("Invalid filetags format '{}': {}", raw, reason));
    }
}

fn check_agenda_tag(content: &str, filetags: &[String], issues: &mut Vec<String>) {
    let parsed = crate::parser::parse_note(content);
    let has_planned_todos = parsed
        .headings
        .iter()
        .any(|h| h.todo_state.is_some() && (h.scheduled.is_some() || h.deadline.is_some()));
    if has_planned_todos && !filetags.iter().any(|t| t == "agenda") {
        issues.push("Issue: File has planned TODO headings (SCHEDULED/DEADLINE) but missing :agenda: filetag".to_string());
    }
}

fn check_duplicate_uuids(
    content: &str,
    graph: &Graph,
    node: &crate::graph::Node,
    issues: &mut Vec<String>,
) {
    let all_ids: Vec<String> = ID_PROPERTY_RE
        .captures_iter(content)
        .filter_map(|c| c.get(1))
        .map(|m| m.as_str().to_string())
        .collect();
    if all_ids.len() > 1 {
        let primary = &all_ids[0];
        let mut seen_heading_ids = std::collections::HashSet::new();
        for id in all_ids.iter().skip(1) {
            if id == primary {
                issues.push(format!(
                    "Duplicate UUID: heading-level :ID: {} matches the note's primary :ID:",
                    id
                ));
            } else if !seen_heading_ids.insert(id.clone()) {
                issues.push(format!(
                    "Duplicate UUID: heading-level :ID: {} is used by multiple headings in this note",
                    id
                ));
            } else if let Some(other) = graph.find_node(id)
                && other.uuid != node.uuid
            {
                issues.push(format!(
                    "Duplicate UUID: heading-level :ID: {} belongs to another note \"{}\"",
                    id, other.title
                ));
            }
        }
    }
}

fn check_broken_links(
    node: &crate::graph::Node,
    graph: &Graph,
    db_root: &Path,
) -> (Vec<String>, Vec<String>) {
    let mut broken_internal = Vec::new();
    let mut broken_files = Vec::new();
    for link in &node.outgoing {
        match link {
            Link::Internal(uuid) if !graph.nodes.contains_key(uuid) => {
                broken_internal.push(uuid.clone());
            }
            Link::File(path_str) if !file_link_target_exists(path_str, &node.path, db_root) => {
                broken_files.push(path_str.clone());
            }
            _ => {}
        }
    }
    (broken_internal, broken_files)
}

fn check_self_links(
    node: &crate::graph::Node,
    target: &str,
    target_is_uuid: bool,
    is_heading_node: bool,
    graph: &Graph,
    db_root: &Path,
    issues: &mut Vec<String>,
) {
    for link in &node.outgoing {
        match link {
            Link::Internal(uuid) if uuid == target || (!target_is_uuid && uuid == &node.uuid) => {
                issues.push(format!("Self-link via id link: {}", uuid));
            }
            Link::File(path) => {
                let resolved = resolve_file_link_path(path, &node.path, db_root);
                if resolved == node.path {
                    if target == node.uuid || !target_is_uuid || is_heading_node {
                        issues.push(if is_heading_node {
                            let primary_uuid = graph
                                .heading_uuid_to_primary
                                .get(&node.uuid)
                                .cloned()
                                .unwrap_or_else(|| node.uuid.clone());
                            format!(
                                "File link to own file; consider using id:{} instead of file:{}",
                                primary_uuid, path
                            )
                        } else {
                            format!("Self-link via file link to own file: {}", path)
                        });
                    } else {
                        issues.push(format!(
                            "File link to own file; consider using id:{} instead of file:{}",
                            node.uuid, path
                        ));
                    }
                }
            }
            _ => {}
        }
    }
}

fn check_overlinking(node: &crate::graph::Node, graph: &Graph, issues: &mut Vec<String>) {
    let mut target_counts: HashMap<String, usize> = HashMap::new();
    for link in &node.outgoing {
        if let Link::Internal(uuid) = link {
            *target_counts.entry(uuid.clone()).or_default() += 1;
        }
    }
    for (uuid, count) in target_counts {
        if count >= 2 {
            let title = graph
                .nodes
                .get(&uuid)
                .map(|n| n.title.as_str())
                .unwrap_or("<unknown>");
            issues.push(format!(
                "Overlinking: {} links to \"{}\" ({}) \u{2014} consider removing duplicate links",
                count, title, uuid
            ));
        }
    }
}

fn validate_one(graph: &Graph, target: &str, db_root: &Path) -> Result<ValidateOutput> {
    let node = graph.resolve_target(target)?.clone();
    let is_heading_node = graph.heading_uuid_to_primary.contains_key(&node.uuid);
    let target_is_uuid = UUID_FORMAT_RE.is_match(target);
    validate_node(graph, &node, target, target_is_uuid, is_heading_node, db_root)
}

fn validate_node(
    graph: &Graph,
    node: &crate::graph::Node,
    target: &str,
    target_is_uuid: bool,
    is_heading_node: bool,
    db_root: &Path,
) -> Result<ValidateOutput> {
    let mut issues = Vec::new();

    let content = graph
        .results
        .iter()
        .find(|r| r.path == node.path)
        .and_then(|r| r.raw_content.as_deref())
        .map(std::string::ToString::to_string)
        .unwrap_or_default();

    check_uuid_format(node, &mut issues);
    check_title_presence(&content, &mut issues);
    check_filetags_formatting(&content, &mut issues);
    check_agenda_tag(&content, &node.filetags, &mut issues);
    check_duplicate_uuids(&content, graph, node, &mut issues);

    let (broken_internal, broken_files) = check_broken_links(node, graph, db_root);

    check_self_links(
        node,
        target,
        target_is_uuid,
        is_heading_node,
        graph,
        db_root,
        &mut issues,
    );
    check_overlinking(node, graph, &mut issues);

    let incoming = graph.backlinks.get(&node.uuid).cloned().unwrap_or_default();

    let backlink_entries: Vec<BacklinkEntry> = incoming
        .iter()
        .filter_map(|uuid| graph.nodes.get(uuid).map(BacklinkEntry::from))
        .collect();

    if !broken_internal.is_empty() {
        issues.push(format!("{} broken internal link(s)", broken_internal.len()));
    }
    if !broken_files.is_empty() {
        issues.push(format!("{} broken file link(s)", broken_files.len()));
    }

    Ok(build_validate_output(
        node,
        &incoming,
        broken_internal,
        broken_files,
        backlink_entries,
        issues,
    ))
}

pub struct ValidateOptions {
    pub targets: Vec<String>,
}

pub fn run(config: &Config, ctx: &OutputContext, opts: &ValidateOptions) -> Result<()> {
    let graph = Graph::load(config)?;
    let db_root = config.resolved_db_root()?;

    match ctx.format {
        OutputFormat::Text => {
            for t in &opts.targets {
                let node = graph.resolve_target(t)?.clone();
                let is_heading_node = graph.heading_uuid_to_primary.contains_key(&node.uuid);
                let target_is_uuid = UUID_FORMAT_RE.is_match(t);
                let incoming = graph.backlinks.get(&node.uuid).cloned().unwrap_or_default();
                let output = validate_node(
                    &graph, &node, t, target_is_uuid, is_heading_node, db_root,
                )?;
                print_validate_text(
                    &node,
                    &output.broken_internal,
                    &output.broken_files,
                    &incoming,
                    &output.issues,
                );
                if opts.targets.len() > 1 {
                    println!();
                }
            }
        }
        OutputFormat::Json => {
            let all_outputs: Vec<ValidateOutput> = opts
                .targets
                .iter()
                .map(|t| validate_one(&graph, t, db_root))
                .collect::<Result<Vec<_>>>()?;
            ctx.print_json_adaptive(&all_outputs)?;
        }
        OutputFormat::Ndjson => {
            let all_outputs: Vec<ValidateOutput> = opts
                .targets
                .iter()
                .map(|t| validate_one(&graph, t, db_root))
                .collect::<Result<Vec<_>>>()?;
            ctx.print_ndjson(&all_outputs)?;
        }
    }

    Ok(())
}
