use crate::cli::PathArgs;
use crate::command_context::CommandContext;
use crate::graph::Graph;
use crate::output::OutputContext;
use anyhow::Result;
use serde::Serialize;

#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct PathOutput {
    pub from: String,
    pub to: String,
    pub found: bool,
    pub hops: usize,
    pub path: Vec<PathNode>,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct PathNode {
    pub uuid: String,
    pub title: String,
}

pub struct PathOptions {
    pub from: String,
    pub to: String,
}

impl From<&PathArgs> for PathOptions {
    fn from(args: &PathArgs) -> Self {
        PathOptions {
            from: args.from.clone(),
            to: args.to.clone(),
        }
    }
}

pub fn run(ctx: &CommandContext<'_>, opts: &PathOptions) -> Result<()> {
    let output = execute(ctx, opts)?;
    render(ctx.output(), &output)
}

fn execute(ctx: &CommandContext<'_>, opts: &PathOptions) -> Result<PathOutput> {
    let graph = ctx.load_graph()?;
    build_output(&graph, opts)
}

fn build_output(graph: &Graph, opts: &PathOptions) -> Result<PathOutput> {
    let from_node = graph.find_node(&opts.from);
    let to_node = graph.find_node(&opts.to);

    let (from_str, to_str) = (opts.from.clone(), opts.to.clone());

    match (from_node, to_node) {
        (Some(f), Some(t)) => {
            let path_uuids = graph.find_shortest_path(&f.uuid, &t.uuid, None);
            let (found, hops, path_nodes) = match &path_uuids {
                Some(uuids) => {
                    let nodes: Vec<PathNode> = uuids
                        .iter()
                        .filter_map(|uuid| {
                            graph.nodes.get(uuid).map(|n| PathNode {
                                uuid: n.uuid.to_string(),
                                title: n.title.clone(),
                            })
                        })
                        .collect();
                    (true, uuids.len() - 1, nodes)
                }
                None => (false, 0, vec![]),
            };
            Ok(PathOutput {
                from: f.title.clone(),
                to: t.title.clone(),
                found,
                hops,
                path: path_nodes,
            })
        }
        (None, _) => anyhow::bail!("Source note not found: {from_str}"),
        (_, None) => anyhow::bail!("Target note not found: {to_str}"),
    }
}

fn render(ctx: &OutputContext, output: &PathOutput) -> Result<()> {
    if ctx.is_structured() {
        ctx.print_structured(output)
    } else {
        print!("{}", render_text(output));
        Ok(())
    }
}

fn render_text(output: &PathOutput) -> String {
    if !output.found {
        return format!(
            "No path found between \"{}\" and \"{}\"\n",
            output.from, output.to
        );
    }

    let mut text = format!(
        "Shortest path between \"{}\" and \"{}\":\n  {} hop(s)\n\n",
        output.from, output.to, output.hops
    );
    for (i, node) in output.path.iter().enumerate() {
        text.push_str(&format!("  {}. {}\n", i + 1, node.title));
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_found_path_text_from_output() {
        let output = PathOutput {
            from: "Start".to_string(),
            to: "End".to_string(),
            found: true,
            hops: 2,
            path: vec![
                PathNode {
                    uuid: "a".to_string(),
                    title: "Start".to_string(),
                },
                PathNode {
                    uuid: "b".to_string(),
                    title: "Middle".to_string(),
                },
                PathNode {
                    uuid: "c".to_string(),
                    title: "End".to_string(),
                },
            ],
        };

        assert_eq!(
            render_text(&output),
            "Shortest path between \"Start\" and \"End\":\n  2 hop(s)\n\n  1. Start\n  2. Middle\n  3. End\n"
        );
    }

    #[test]
    fn renders_missing_path_text_from_output() {
        let output = PathOutput {
            from: "Start".to_string(),
            to: "End".to_string(),
            found: false,
            hops: 0,
            path: vec![],
        };

        assert_eq!(
            render_text(&output),
            "No path found between \"Start\" and \"End\"\n"
        );
    }
}
