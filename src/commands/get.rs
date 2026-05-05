use crate::cli::OutputFormat;
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
    pub categories: Vec<String>,
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
            categories: node.categories.clone(),
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
    pub show_links: bool,
    pub no_content: bool,
    pub from_stdin: bool,
}

fn get_neighbor_map(graph: &Graph, uuid: &str) -> HashMap<u32, NeighborOutput> {
    let mut map = HashMap::new();
    if let Some(ns) = graph.get_neighbors(uuid, 1).get(&1) {
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
        map.insert(1, NeighborOutput { outgoing, incoming });
    }
    map
}

fn process_one_get(
    graph: &Graph,
    target: &str,
    show_links: bool,
    no_content: bool,
) -> Result<GetOutput> {
    let node = graph.resolve_target(target)?.clone();
    let neighbors = if show_links {
        get_neighbor_map(graph, &node.uuid)
    } else {
        HashMap::new()
    };

    let node_content = if no_content {
        None
    } else {
        std::fs::read_to_string(&node.path).ok()
    };

    let node_json = NodeJson::from_node(&node, node_content.as_deref());

    Ok(GetOutput {
        node: node_json,
        neighbors,
    })
}

fn print_one_get_text(
    graph: &Graph,
    target: &str,
    show_links: bool,
    no_content: bool,
) -> Result<()> {
    let node = graph.resolve_target(target)?.clone();
    let neighbors = if show_links {
        Some(graph.get_neighbors(&node.uuid, 1))
    } else {
        None
    };

    let node_content = if no_content {
        None
    } else {
        std::fs::read_to_string(&node.path).ok()
    };

    println!("Note: {}", node.title);
    println!("  UUID:   {}", node.uuid);
    println!("  Path:   {}", node.path.display());
    if !node.filetags.is_empty() {
        println!("  Tags:   {}", node.filetags.join(", "));
    }
    if !node.categories.is_empty() {
        println!("  Cats:   {}", node.categories.join(", "));
    }
    if let Some(content) = node_content {
        println!();
        println!("--- Content ---");
        println!("{content}");
        println!("--- End Content ---");
    }

    if let Some(ns) = neighbors
        && let Some(ns) = ns.get(&1)
    {
        println!();
        if !ns.outgoing.is_empty() {
            println!("Forward links:");
            for n in &ns.outgoing {
                println!("  {} ({})", n.title, n.uuid);
            }
        }
        if !ns.incoming.is_empty() {
            println!("Backlinks:");
            for n in &ns.incoming {
                println!("  {} ({})", n.title, n.uuid);
            }
        }
        if ns.outgoing.is_empty() && ns.incoming.is_empty() {
            println!("(no connections)");
        }
    }

    Ok(())
}

pub fn run(
    config: &Config,
    ctx: &OutputContext,
    opts: &GetOptions,
    db_cli: Option<&std::path::Path>,
) -> Result<()> {
    let targets: Vec<String> = if opts.from_stdin
        || (opts.target.is_none() && util::is_stdin_piped())
    {
        util::read_stdin_ndjson()?
    } else if let Some(t) = opts.target {
        vec![t.to_string()]
    } else {
        anyhow::bail!(
            "No target specified and no stdin pipe detected. Provide a target or use --from-stdin."
        );
    };

    let show_links = opts.show_links;
    let no_content = opts.no_content;

    let graph = Graph::load(config, db_cli)?;

    match ctx.format {
        OutputFormat::Text => {
            for target in &targets {
                print_one_get_text(&graph, target, show_links, no_content)?;
                if targets.len() > 1 {
                    println!();
                }
            }
        }
        OutputFormat::Json => {
            let mut all_outputs = Vec::new();
            for target in &targets {
                all_outputs.push(process_one_get(&graph, target, show_links, no_content)?);
            }
            if all_outputs.len() == 1 {
                ctx.print_json(&all_outputs[0])?;
            } else {
                ctx.print_json(&all_outputs)?;
            }
        }
        OutputFormat::Ndjson => {
            for target in &targets {
                let output = process_one_get(&graph, target, show_links, no_content)?;
                println!("{}", serde_json::to_string(&output)?);
            }
        }
    }

    Ok(())
}
