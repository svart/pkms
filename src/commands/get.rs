use crate::config::Config;
use crate::discovery::discover_files;
use crate::graph::{FileScanResult, Graph};
use crate::parser::parse_note;
use anyhow::Result;
use serde::Serialize;

#[derive(Serialize)]
pub struct GetOutput {
    pub node: serde_json::Value,
    pub neighbors: std::collections::HashMap<u32, NeighborOutput>,
}

#[derive(Serialize)]
pub struct NeighborOutput {
    pub outgoing: Vec<serde_json::Value>,
    pub incoming: Vec<serde_json::Value>,
}

pub fn run(
    config: &Config,
    json: bool,
    _verbose: bool,
    target: &str,
    depth: u32,
    show_content: bool,
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
            let neighbors = graph.get_neighbors(&node.uuid, depth);

            let node_content = if show_content {
                std::fs::read_to_string(&node.path).ok()
            } else {
                None
            };

            if json {
                let node_json = node_to_json(&node, node_content.as_deref());
                let mut neigh_json = std::collections::HashMap::new();
                for (d, ns) in &neighbors {
                    let outgoing: Vec<serde_json::Value> = ns
                        .outgoing
                        .iter()
                        .map(|n| node_to_json(n, None))
                        .collect();
                    let incoming: Vec<serde_json::Value> = ns
                        .incoming
                        .iter()
                        .map(|n| node_to_json(n, None))
                        .collect();
                    neigh_json.insert(*d, NeighborOutput { outgoing, incoming });
                }
                let output = GetOutput {
                    node: node_json,
                    neighbors: neigh_json,
                };
                println!("{}", serde_json::to_string_pretty(&output)?);
            } else {
                println!("Note: {}", node.title);
                println!("  UUID:   {}", node.uuid);
                println!("  Path:   {}", node.path.display());
                if !node.filetags.is_empty() {
                    println!("  Tags:   {}", node.filetags.join(", "));
                }
                if show_content {
                    if let Some(content) = node_content {
                        println!();
                        println!("--- Content ---");
                        println!("{}", content);
                        println!("--- End ---");
                    }
                }

                for d in 1..=depth {
                    if let Some(ns) = neighbors.get(&d) {
                        println!();
                        println!("Depth {}:", d);
                        if !ns.outgoing.is_empty() {
                            println!("  Forward links:");
                            for n in &ns.outgoing {
                                println!("    {} ({})", n.title, n.uuid);
                            }
                        }
                        if !ns.incoming.is_empty() {
                            println!("  Backlinks:");
                            for n in &ns.incoming {
                                println!("    {} ({})", n.title, n.uuid);
                            }
                        }
                        if ns.outgoing.is_empty() && ns.incoming.is_empty() {
                            println!("  (no connections at this depth)");
                        }
                    }
                }
            }
        }
        None => {
            anyhow::bail!("Note not found: {}", target);
        }
    }

    Ok(())
}

fn node_to_json(node: &crate::graph::Node, content: Option<&str>) -> serde_json::Value {
    let mut map = serde_json::Map::new();
    map.insert("uuid".to_string(), serde_json::Value::String(node.uuid.clone()));
    map.insert("title".to_string(), serde_json::Value::String(node.title.clone()));
    map.insert(
        "path".to_string(),
        serde_json::Value::String(node.path.to_string_lossy().to_string()),
    );
    map.insert(
        "filetags".to_string(),
        serde_json::Value::Array(
            node.filetags.iter().map(|t| serde_json::Value::String(t.clone())).collect(),
        ),
    );
    if let Some(c) = content {
        map.insert("content".to_string(), serde_json::Value::String(c.to_string()));
    }
    serde_json::Value::Object(map)
}
