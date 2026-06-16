use crate::cli::OutputFormat;
use crate::config::ResolvedConfig;
use crate::graph::{Graph, file_link_target_exists, resolve_file_link_path};
use crate::output::OutputContext;
use crate::parser::{ID_PROPERTY_RE, Link, TITLE_RE, UUID_FORMAT_RE, validate_filetags_format};
use anyhow::Result;
use serde::Serialize;
use std::collections::HashMap;
use std::fmt::Write;
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

fn check_uuid_format(node: &crate::graph::Node, issues: &mut Vec<String>) {
    let uuid_parts: Vec<&str> = node.uuid.split('-').collect();
    if uuid_parts.len() != 5 {
        issues.push(format!("Invalid UUID format: {}", node.uuid));
    }
}

fn check_title_presence(content: &str, issues: &mut Vec<String>) {
    if !TITLE_RE.is_match(content) {
        issues.push("Missing #+title: property".to_string());
    }
}

fn check_filetags_formatting(content: &str, issues: &mut Vec<String>) {
    for (raw, reason) in validate_filetags_format(content) {
        issues.push(format!("Invalid filetags format '{}': {}", raw, reason));
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
    validate_node(
        graph,
        &node,
        target,
        target_is_uuid,
        is_heading_node,
        db_root,
    )
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

pub fn run(config: &ResolvedConfig, ctx: &OutputContext, opts: &ValidateOptions) -> Result<()> {
    let outputs = execute(config, opts)?;
    render(ctx, &outputs)
}

pub fn execute(config: &ResolvedConfig, opts: &ValidateOptions) -> Result<Vec<ValidateOutput>> {
    let graph = Graph::load(config)?;
    let db_root = config.resolved_db_root();

    opts.targets
        .iter()
        .map(|target| validate_one(&graph, target, db_root))
        .collect()
}

pub fn render(ctx: &OutputContext, outputs: &[ValidateOutput]) -> Result<()> {
    match ctx.format {
        OutputFormat::Text => {
            print!("{}", render_text(outputs));
        }
        OutputFormat::Json => {
            ctx.print_json_adaptive(outputs)?;
        }
        OutputFormat::Ndjson => {
            ctx.print_ndjson(outputs)?;
        }
    }

    Ok(())
}

pub fn render_text(outputs: &[ValidateOutput]) -> String {
    outputs
        .iter()
        .map(render_one_text)
        .collect::<Vec<_>>()
        .join("\n")
}

fn render_one_text(output: &ValidateOutput) -> String {
    let mut text = String::new();

    let _ = writeln!(text, "Note: {}", output.title);
    let _ = writeln!(text, "  UUID:   {}", output.uuid);
    let _ = writeln!(text, "  Path:   {}", output.path);
    if !output.filetags.is_empty() {
        let _ = writeln!(text, "  Tags:   {}", output.filetags.join(", "));
    }
    if !output.categories.is_empty() {
        let _ = writeln!(text, "  Cats:   {}", output.categories.join(", "));
    }
    if !output.aliases.is_empty() {
        let _ = writeln!(text, "  Aliases: {}", output.aliases.join(", "));
    }
    if !output.refs.is_empty() {
        let _ = writeln!(text, "  Refs:   {}", output.refs.join(", "));
    }
    let _ = writeln!(text, "  Headings: {}", output.headings);
    if !output.heading_uuids.is_empty() {
        let _ = writeln!(text, "  Heading UUIDs: {}", output.heading_uuids.join(", "));
    }
    text.push('\n');
    text.push_str("Links:\n");
    let _ = writeln!(
        text,
        "  Outgoing: {} ({} internal)",
        output.outgoing, output.outgoing_internal,
    );
    let _ = writeln!(text, "  Incoming: {}", output.incoming);
    let _ = writeln!(
        text,
        "  Broken:   {} internal, {} file",
        output.broken_internal.len(),
        output.broken_files.len()
    );

    if !output.broken_internal.is_empty() {
        text.push('\n');
        text.push_str("Broken internal links:\n");
        for uuid in &output.broken_internal {
            let _ = writeln!(text, "  -> {uuid}");
        }
    }

    if !output.broken_files.is_empty() {
        text.push('\n');
        text.push_str("Broken file links:\n");
        for path in &output.broken_files {
            let _ = writeln!(text, "  -> {path}");
        }
    }

    if !output.issues.is_empty() {
        text.push('\n');
        for issue in &output.issues {
            let _ = writeln!(text, "Issue: {issue}");
        }
    }

    text.push('\n');
    if output.healthy {
        text.push_str("Status: healthy\n");
    } else {
        let _ = writeln!(text, "Status: {} issue(s)", output.issues.len());
    }

    text
}

#[cfg(test)]
mod tests {
    use super::*;

    fn healthy_output() -> ValidateOutput {
        ValidateOutput {
            uuid: "11111111-1111-4111-8111-111111111111".to_string(),
            title: "Note A".to_string(),
            path: "/notes/a.org".to_string(),
            filetags: vec!["tag".to_string()],
            categories: vec!["cat".to_string()],
            aliases: vec!["Alias A".to_string()],
            refs: vec!["ref-a".to_string()],
            headings: 2,
            heading_uuids: vec!["22222222-2222-4222-8222-222222222222".to_string()],
            outgoing: 3,
            incoming: 1,
            outgoing_internal: 2,
            broken_internal: vec![],
            broken_files: vec![],
            backlinks: vec![],
            issues: vec![],
            healthy: true,
        }
    }

    #[test]
    fn renders_healthy_validate_text_from_typed_output() {
        let text = render_text(&[healthy_output()]);

        assert!(text.contains("Note: Note A"));
        assert!(text.contains("  Tags:   tag"));
        assert!(text.contains("  Heading UUIDs: 22222222-2222-4222-8222-222222222222"));
        assert!(text.contains("  Outgoing: 3 (2 internal)"));
        assert!(text.ends_with("Status: healthy\n"));
    }

    #[test]
    fn renders_unhealthy_validate_text_from_typed_output() {
        let mut output = healthy_output();
        output.broken_internal = vec!["missing-id".to_string()];
        output.broken_files = vec!["missing.org".to_string()];
        output.issues = vec!["1 broken internal link(s)".to_string()];
        output.healthy = false;

        let text = render_text(&[output]);

        assert!(text.contains("Broken internal links:\n  -> missing-id"));
        assert!(text.contains("Broken file links:\n  -> missing.org"));
        assert!(text.contains("Issue: 1 broken internal link(s)"));
        assert!(text.ends_with("Status: 1 issue(s)\n"));
    }
}
