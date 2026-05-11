use crate::config::Config;
use crate::graph::Graph;
use crate::output::OutputContext;
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

pub struct PathOptions {
    pub from: String,
    pub to: String,
}

pub fn run(config: &Config, ctx: &OutputContext, opts: &PathOptions) -> Result<()> {
    let graph = Graph::load(config)?;

    let from_node = graph.find_node(&opts.from);
    let to_node = graph.find_node(&opts.to);

    let (from_str, to_str) = (opts.from.clone(), opts.to.clone());

    match (from_node, to_node) {
        (Some(f), Some(t)) => {
            let path_uuids = graph.find_shortest_path(&f.uuid, &t.uuid, None);

            if ctx.is_json() {
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
                ctx.print_json(&output)?;
            } else if let Some(uuids) = path_uuids {
                let hops = uuids.len() - 1;
                println!("Shortest path between \"{}\" and \"{}\":", f.title, t.title);
                println!("  {hops} hop(s)");
                println!();
                for (i, uuid) in uuids.iter().enumerate() {
                    let title = graph.nodes.get(uuid).map_or("?", |n| n.title.as_str());
                    println!("  {}. {}", i + 1, title);
                }
            } else {
                println!("No path found between \"{}\" and \"{}\"", f.title, t.title);
            }
        }
        (None, _) => anyhow::bail!("Source note not found: {from_str}"),
        (_, None) => anyhow::bail!("Target note not found: {to_str}"),
    }

    Ok(())
}
