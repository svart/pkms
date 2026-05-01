use crate::config::Config;
use crate::discovery::discover_files;
use crate::graph::{FileScanResult, Graph};
use crate::parser::{parse_note, Link};
use anyhow::Result;

pub fn run(
    config: &Config,
    json: bool,
    verbose: bool,
    target: &str,
    db_cli: Option<&std::path::Path>,
) -> Result<()> {
    let db_root = config.resolve_db_root(db_cli)?;
    let ignore = config.resolve_ignore_patterns();

    let files = discover_files(&db_root, &ignore)?;

    let results: Vec<FileScanResult> = files
        .into_iter()
        .map(|entry| {
            let path = entry.path.clone();
            match std::fs::read_to_string(&path) {
                Ok(content) => {
                    let parsed = parse_note(&content);
                    FileScanResult {
                        path,
                        parsed,
                        parse_error: None,
                    }
                }
                Err(e) => FileScanResult {
                    path,
                    parsed: crate::parser::ParsedNote {
                        uuid: None,
                        title: None,
                        filetags: vec![],
                        roam_aliases: vec![],
                        roam_refs: vec![],
                        outgoing: vec![],
                        headings: vec![],
                        content_hash: String::new(),
                    },
                    parse_error: Some(format!("IO error: {}", e)),
                },
            }
        })
        .collect();

    let graph = Graph::build(results);

    let node = graph.find_node(target).map(|n| n.clone());

    match node {
        Some(node) => {
            let outgoing_internal: Vec<&Link> = node
                .outgoing
                .iter()
                .filter(|l| matches!(l, Link::Internal(_)))
                .collect();
            let broken: Vec<&Link> = outgoing_internal
                .iter()
                .filter(|l| {
                    if let Link::Internal(uuid) = l {
                        !graph.nodes.contains_key(uuid)
                    } else {
                        false
                    }
                })
                .copied()
                .collect();
            let incoming = graph.backlinks.get(&node.uuid).cloned().unwrap_or_default();
            let broken_count = broken.len();
            let incoming_count = incoming.len();
            let outgoing_count = node.outgoing.len();

            if json {
                let output = serde_json::json!({
                    "uuid": node.uuid,
                    "title": node.title,
                    "path": node.path.to_string_lossy(),
                    "filetags": node.filetags,
                    "aliases": node.aliases,
                    "refs": node.refs,
                    "headings": node.headings_count,
                    "outgoing_links": outgoing_count,
                    "incoming_links": incoming_count,
                    "broken_links": broken_count,
                    "healthy": broken_count == 0,
                });
                println!("{}", serde_json::to_string_pretty(&output)?);
            } else {
                println!("Note: {}", node.title);
                println!("  UUID:   {}", node.uuid);
                println!("  Path:   {}", node.path.display());
                if !node.filetags.is_empty() {
                    println!("  Tags:   {}", node.filetags.join(", "));
                }
                if !node.aliases.is_empty() {
                    println!("  Aliases: {}", node.aliases.join(", "));
                }
                println!("  Headings: {}", node.headings_count);
                println!();
                println!("Links:");
                println!("  Outgoing: {} ({} internal)", outgoing_count, outgoing_internal.len());
                println!("  Incoming: {}", incoming_count);
                println!("  Broken:   {}", broken_count);

                if !broken.is_empty() {
                    println!();
                    println!("Broken links:");
                    for l in &broken {
                        if let Link::Internal(uuid) = l {
                            println!("  -> {}", uuid);
                        }
                    }
                }

                if verbose && !incoming.is_empty() {
                    println!();
                    println!("Backlinks:");
                    for uuid in &incoming {
                        let title = graph
                            .nodes
                            .get(uuid)
                            .map(|n| n.title.as_str())
                            .unwrap_or("?");
                        println!("  {} ({})", title, uuid);
                    }
                }

                if broken_count == 0 {
                    println!();
                    println!("Status: healthy");
                } else {
                    println!();
                    println!("Status: {} broken link(s)", broken_count);
                }
            }
        }
        None => {
            anyhow::bail!("Note not found: {}", target);
        }
    }

    Ok(())
}
