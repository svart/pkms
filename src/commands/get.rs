use crate::config::Config;
use crate::graph::{Graph, Node};
use crate::output::OutputContext;
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

pub struct GetOptions<'a> {
    pub target: Option<&'a str>,
    pub depth: u32,
    pub no_content: bool,
}

pub fn run(
    config: &Config,
    ctx: &OutputContext,
    opts: &GetOptions,
    db_cli: Option<&std::path::Path>,
) -> Result<()> {
    let target = opts
        .target
        .ok_or_else(|| anyhow::anyhow!("No target specified"))?;
    let depth = opts.depth;
    let no_content = opts.no_content;

    let graph = Graph::load(config, db_cli)?;

    let node = graph.resolve_target(target)?.clone();
    let neighbors = graph.get_neighbors(&node.uuid, depth);

    let node_content = if no_content {
        None
    } else {
        std::fs::read_to_string(&node.path).ok()
    };

    if ctx.is_json() {
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
        ctx.print_json(&output)?;
    } else {
        println!("Note: {}", node.title);
        println!("  UUID:   {}", node.uuid);
        println!("  Path:   {}", node.path.display());
        if !node.filetags.is_empty() {
            println!("  Tags:   {}", node.filetags.join(", "));
        }
        if let Some(content) = node_content {
            println!();
            println!("--- Content ---");
            println!("{content}");
            println!("--- End ---");
        }

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

    Ok(())
}

fn print_node_short(n: &crate::graph::Node, indent: &str) {
    println!("{}{} ({})", indent, n.title, util::short_uuid(&n.uuid));
}
