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
    from: Option<&str>,
    to: Option<&str>,
    max_depth: Option<u32>,
    db_cli: Option<&std::path::Path>,
) -> Result<()> {
    let from = from.ok_or_else(|| {
        anyhow::anyhow!("No source specified. Provide --from or use --input-json")
    })?;
    let to =
        to.ok_or_else(|| anyhow::anyhow!("No target specified. Provide --to or use --input-json"))?;

    let graph = Graph::load(config, db_cli, verbose)?;

    let from_node = graph.find_node(from);
    let to_node = graph.find_node(to);

    let (from_str, to_str) = (from.to_string(), to.to_string());

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
                    from: f.title.clone(),
                    to: t.title.clone(),
                    found,
                    hops,
                    path: path_nodes,
                };
                println!("{}", serde_json::to_string_pretty(&output)?);
            } else if let Some(uuids) = path_uuids {
                let hops = uuids.len() - 1;
                println!("Shortest path between \"{}\" and \"{}\":", f.title, t.title);
                println!("  {hops} hop(s)");
                println!();
                for (i, uuid) in uuids.iter().enumerate() {
                    let title = graph.nodes.get(uuid).map_or("?", |n| n.title.as_str());
                    let arrow = if i < uuids.len() - 1 { " →" } else { "" };
                    println!("  {}. {}{}", i + 1, title, arrow);
                }
            } else {
                println!("No path found between \"{}\" and \"{}\"", f.title, t.title);
                if max_depth.is_some() {
                    println!("  (try increasing --max-depth)");
                }
            }
        }
        (None, _) => anyhow::bail!("Source note not found: {from_str}"),
        (_, None) => anyhow::bail!("Target note not found: {to_str}"),
    }

    Ok(())
}
