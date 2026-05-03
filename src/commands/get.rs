use crate::config::Config;
use crate::graph::{Graph, Node};
use crate::util;
use anyhow::Result;
use serde::Serialize;
use std::collections::HashMap;

#[derive(Serialize)]
pub struct NodeJson {
    pub uuid: String,
    pub title: String,
    pub path: String,
    pub filetags: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
}

impl NodeJson {
    fn from_node(node: &Node, content: Option<&str>) -> Self {
        NodeJson {
            uuid: node.uuid.clone(),
            title: node.title.clone(),
            path: util::path_string(&node.path),
            filetags: node.filetags.clone(),
            content: content.map(std::string::ToString::to_string),
        }
    }
}

#[derive(Serialize)]
pub struct GetOutput {
    pub node: NodeJson,
    pub neighbors: HashMap<u32, NeighborOutput>,
}

#[derive(Serialize)]
pub struct NeighborOutput {
    pub outgoing: Vec<NodeJson>,
    pub incoming: Vec<NodeJson>,
}

#[allow(clippy::too_many_lines)]
pub fn run(
    config: &Config,
    json: bool,
    verbose: bool,
    target: Option<&str>,
    depth: u32,
    show_content: bool,
    show_graph: bool,
    db_cli: Option<&std::path::Path>,
) -> Result<()> {
    let target = target.ok_or_else(|| anyhow::anyhow!("No target specified"))?;

    let graph = Graph::load(config, db_cli, verbose)?;

    let node = graph.resolve_target(target)?.clone();
    let neighbors = graph.get_neighbors(&node.uuid, depth);

    let node_content = if show_content {
        std::fs::read_to_string(&node.path).ok()
    } else {
        None
    };

    if json {
        let node_json = NodeJson::from_node(&node, node_content.as_deref());
        let mut neigh_json = HashMap::new();
        for (d, ns) in &neighbors {
            let outgoing: Vec<NodeJson> = ns
                .outgoing
                .iter()
                .map(|n| NodeJson::from_node(n, None))
                .collect();
            let incoming: Vec<NodeJson> = ns
                .incoming
                .iter()
                .map(|n| NodeJson::from_node(n, None))
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
        if show_content && let Some(content) = node_content {
            println!();
            println!("--- Content ---");
            println!("{content}");
            println!("--- End ---");
        }

        if show_graph {
            print_graph(&node, &neighbors, depth);
        } else {
            for d in 1..=depth {
                if let Some(ns) = neighbors.get(&d) {
                    println!();
                    println!("Depth {d}:");
                    if !ns.outgoing.is_empty() {
                        println!("  Forward links:");
                        for n in &ns.outgoing {
                            print_node_short(n, "    ");
                        }
                    }
                    if !ns.incoming.is_empty() {
                        println!("  Backlinks:");
                        for n in &ns.incoming {
                            print_node_short(n, "    ");
                        }
                    }
                    if ns.outgoing.is_empty() && ns.incoming.is_empty() {
                        println!("  (no connections at this depth)");
                    }
                }
            }
        }
    }

    Ok(())
}

fn print_node_short(n: &crate::graph::Node, indent: &str) {
    println!("{}{} ({})", indent, n.title, util::short_uuid(&n.uuid));
}

fn print_graph(
    node: &crate::graph::Node,
    neighbors: &std::collections::HashMap<u32, crate::graph::NeighborSet>,
    max_depth: u32,
) {
    let label = |n: &crate::graph::Node| format!("{} ({})", n.title, util::short_uuid(&n.uuid));

    println!();
    let depth1 = neighbors.get(&1);

    // Print backlinks (incoming at depth 1)
    if let Some(ns) = depth1
        && !ns.incoming.is_empty()
    {
        for (i, n) in ns.incoming.iter().enumerate() {
            let prefix = if i == ns.incoming.len() - 1 {
                "└─ "
            } else {
                "├─ "
            };
            println!("{} {}", prefix, label(n));
        }
        println!("│");
    }

    // Print current node
    println!("● {}", label(node));

    // Print outgoing
    if let Some(ns) = depth1
        && !ns.outgoing.is_empty()
    {
        println!("│");
        for (i, n) in ns.outgoing.iter().enumerate() {
            let prefix = if i == ns.outgoing.len() - 1 {
                "└─ "
            } else {
                "├─ "
            };
            println!("{} {}", prefix, label(n));
        }
    }

    // Deeper levels
    for d in 2..=max_depth {
        if let Some(ns) = neighbors.get(&d)
            && (!ns.outgoing.is_empty() || !ns.incoming.is_empty())
        {
            println!();
            println!("Depth {d}:");
            for n in &ns.outgoing {
                println!("  → {}", label(n));
            }
            for n in &ns.incoming {
                println!("  ← {}", label(n));
            }
        }
    }
}
