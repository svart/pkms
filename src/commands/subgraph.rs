use crate::config::Config;
use crate::graph::Graph;
use anyhow::Result;
use serde::Serialize;

#[derive(Serialize)]
pub struct SubgraphNode {
    pub uuid: String,
    pub title: String,
    pub path: String,
    pub filetags: Vec<String>,
}

impl From<&crate::graph::Node> for SubgraphNode {
    fn from(n: &crate::graph::Node) -> Self {
        SubgraphNode {
            uuid: n.uuid.clone(),
            title: n.title.clone(),
            path: n.path.to_string_lossy().to_string(),
            filetags: n.filetags.clone(),
        }
    }
}

#[derive(Serialize)]
pub struct SubgraphOutput {
    pub root_uuid: String,
    pub root_title: String,
    pub depth: u32,
    pub vertex_count: usize,
    pub edge_count: usize,
    pub avg_vertex_order: f64,
    pub nodes: Vec<SubgraphNode>,
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
    target: Option<&str>,
    depth: u32,
    input_json: Option<&std::path::PathBuf>,
    db_cli: Option<&std::path::Path>,
) -> Result<()> {
    let target = match (target, input_json) {
        (Some(t), _) => t.to_string(),
        (None, Some(path)) => {
            let content = std::fs::read_to_string(path)?;
            let params: serde_json::Value = serde_json::from_str(&content)?;
            params.get("target").and_then(|v| v.as_str().map(|s| s.to_string()))
                .ok_or_else(|| anyhow::anyhow!("No target specified in JSON"))?
        }
        (None, None) => anyhow::bail!("No target specified. Provide a target or use --input-json"),
    };

    let graph = Graph::load(config, db_cli, verbose)?;

    let root = graph.resolve_target(&target)?.clone();
    let sub = graph.collect_subgraph(&root.uuid, depth);

    if json {
        let nodes: Vec<SubgraphNode> = sub.nodes.iter().map(SubgraphNode::from).collect();

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

    Ok(())
}
