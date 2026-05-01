use crate::config::Config;
use crate::graph::Graph;
use anyhow::Result;
use serde::Serialize;

#[derive(Serialize)]
pub struct PathOutput {
    pub from: String,
    pub to: String,
    pub found: bool,
    pub hops: usize,
    pub path: Vec<PathNode>,
}

#[derive(Serialize)]
pub struct PathNode {
    pub uuid: String,
    pub title: String,
}

pub fn run(
    config: &Config,
    json: bool,
    verbose: bool,
    from: &str,
    to: &str,
    max_depth: Option<u32>,
    db_cli: Option<&std::path::Path>,
) -> Result<()> {
    let graph = Graph::load(config, db_cli, verbose)?;

    let from_node = graph.find_node(from).cloned();
    let to_node = graph.find_node(to).cloned();

    match (from_node, to_node) {
        (Some(f), Some(t)) => {
            let path_uuids = graph.find_shortest_path(&f.uuid, &t.uuid, max_depth);

            if json {
                let (found, hops, path_nodes) = match &path_uuids {
                    Some(uuids) => {
                        let nodes: Vec<PathNode> = uuids
                            .iter()
                            .filter_map(|uuid| {
                                graph.nodes.get(uuid).map(|n| PathNode {
                                    uuid: n.uuid.clone(),
                                    title: n.title.clone(),
                                })
                            })
                            .collect();
                        (true, uuids.len() - 1, nodes)
                    }
                    None => (false, 0, vec![]),
                };
                let output = PathOutput {
                    from: f.title,
                    to: t.title,
                    found,
                    hops,
                    path: path_nodes,
                };
                println!("{}", serde_json::to_string_pretty(&output)?);
            } else {
                match path_uuids {
                    Some(uuids) => {
                        let hops = uuids.len() - 1;
                        println!(
                            "Shortest path between \"{}\" and \"{}\":",
                            f.title, t.title
                        );
                        println!("  {} hop(s)", hops);
                        println!();
                        for (i, uuid) in uuids.iter().enumerate() {
                            let title = graph
                                .nodes
                                .get(uuid)
                                .map(|n| n.title.as_str())
                                .unwrap_or("?");
                            let arrow = if i < uuids.len() - 1 { " →" } else { "" };
                            println!("  {}. {}{}", i + 1, title, arrow);
                        }
                    }
                    None => {
                        println!(
                            "No path found between \"{}\" and \"{}\"",
                            f.title, t.title
                        );
                        if max_depth.is_some() {
                            println!("  (try increasing --max-depth)");
                        }
                    }
                }
            }
        }
        (None, _) => anyhow::bail!("Source note not found: {}", from),
        (_, None) => anyhow::bail!("Target note not found: {}", to),
    }

    Ok(())
}
