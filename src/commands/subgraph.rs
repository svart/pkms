use crate::config::Config;
use crate::graph::Graph;
use anyhow::Result;
use serde::Serialize;

#[derive(Serialize)]
pub struct SubgraphOutput {
    pub root_uuid: String,
    pub root_title: String,
    pub depth: u32,
    pub vertex_count: usize,
    pub edge_count: usize,
    pub avg_vertex_order: f64,
    pub nodes: Vec<serde_json::Value>,
    pub edges: Vec<EdgeEntry>,
}

#[derive(Serialize)]
pub struct EdgeEntry {
    pub source: String,
    pub target: String,
}

pub fn run(
    config: &Config,
    json: bool,
    verbose: bool,
    target: &str,
    depth: u32,
    db_cli: Option<&std::path::Path>,
) -> Result<()> {
    let graph = Graph::load(config, db_cli, verbose)?;

    let root = graph.find_node(target).cloned();

    match root {
        Some(root) => {
            let sub = graph.collect_subgraph(&root.uuid, depth);

            if json {
                let nodes: Vec<serde_json::Value> = sub
                    .nodes
                    .iter()
                    .map(|n| {
                        let mut map = serde_json::Map::new();
                        map.insert(
                            "uuid".to_string(),
                            serde_json::Value::String(n.uuid.clone()),
                        );
                        map.insert(
                            "title".to_string(),
                            serde_json::Value::String(n.title.clone()),
                        );
                        map.insert(
                            "path".to_string(),
                            serde_json::Value::String(n.path.to_string_lossy().to_string()),
                        );
                        map.insert(
                            "filetags".to_string(),
                            serde_json::Value::Array(
                                n.filetags
                                    .iter()
                                    .map(|t| serde_json::Value::String(t.clone()))
                                    .collect(),
                            ),
                        );
                        serde_json::Value::Object(map)
                    })
                    .collect();

                let edges: Vec<EdgeEntry> = sub
                    .edges
                    .iter()
                    .map(|(s, t)| EdgeEntry {
                        source: s.clone(),
                        target: t.clone(),
                    })
                    .collect();

                let output = SubgraphOutput {
                    root_uuid: sub.root_uuid,
                    root_title: root.title,
                    depth,
                    vertex_count: sub.vertex_count,
                    edge_count: sub.edge_count,
                    avg_vertex_order: sub.avg_vertex_order,
                    nodes,
                    edges,
                };
                println!("{}", serde_json::to_string_pretty(&output)?);
            } else {
                println!("Subgraph around \"{}\" (depth: {})", root.title, depth);
                println!("  Vertices: {}", sub.vertex_count);
                println!("  Edges:    {}", sub.edge_count);
                println!("  Avg vertex order: {:.2}", sub.avg_vertex_order);
                println!();
                if !sub.nodes.is_empty() {
                    println!("Nodes:");
                    for n in &sub.nodes {
                        let short = if n.uuid.len() > 8 { &n.uuid[..8] } else { &n.uuid };
                        println!("  {} ({})", n.title, short);
                    }
                }
                if !sub.edges.is_empty() {
                    println!();
                    println!("Edges:");
                    for (s, t) in &sub.edges {
                        let st = graph.nodes.get(s).map(|n| n.title.as_str()).unwrap_or("?");
                        let tt = graph.nodes.get(t).map(|n| n.title.as_str()).unwrap_or("?");
                        println!("  {} -> {}", st, tt);
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
