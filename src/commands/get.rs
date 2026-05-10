use crate::cli::OutputFormat;
use crate::config::Config;
use crate::graph::{Graph, Node};
use crate::output::OutputContext;
use crate::parser::HEADING_RE;
use crate::tokens;
use crate::util;
use anyhow::Result;
use regex::Regex;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::LazyLock;

static HEADING_UUID_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r":ID:\s+([a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12})").unwrap()
});

#[derive(Serialize)]
pub struct HeadingJson {
    pub level: usize,
    pub title: String,
    pub todo_state: Option<String>,
    pub tags: Vec<String>,
    pub raw: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uuid: Option<String>,
    pub priority: Option<char>,
    pub scheduled: Option<String>,
    pub deadline: Option<String>,
}

fn parse_headings_from_content(content: &str) -> Vec<HeadingJson> {
    let mut headings: Vec<HeadingJson> = Vec::new();
    let mut current_heading_idx: Option<usize> = None;
    let mut in_properties = false;
    let mut just_saw_heading = false;

    for line in content.lines() {
        let trimmed = line.trim();

        if trimmed == ":PROPERTIES:" {
            in_properties = true;
            continue;
        }
        if trimmed == ":END:" {
            in_properties = false;
            continue;
        }

        if in_properties {
            if let Some(cap) = HEADING_UUID_RE.captures(line)
                && let Some(idx) = current_heading_idx
            {
                headings[idx].uuid = Some(cap[1].to_string());
            }
            continue;
        }

        if let Some(cap) = HEADING_RE.captures(line) {
            let level = cap[1].len();
            let todo_state = cap.get(2).map(|m| m.as_str().to_string());
            let priority = cap.get(3).and_then(|m| m.as_str().chars().next());
            let heading_title = cap.get(4).map_or("", |m| m.as_str()).to_string();
            if level > 1 || !heading_title.is_empty() {
                let tags = cap
                    .get(5)
                    .map(|m| {
                        m.as_str()
                            .split(':')
                            .filter(|t| !t.is_empty())
                            .map(std::string::ToString::to_string)
                            .collect()
                    })
                    .unwrap_or_default();
                headings.push(HeadingJson {
                    level,
                    title: heading_title,
                    todo_state,
                    tags,
                    raw: line.to_string(),
                    uuid: None,
                    priority,
                    scheduled: None,
                    deadline: None,
                });
                current_heading_idx = Some(headings.len() - 1);
                just_saw_heading = true;
                continue;
            }
        }

        if just_saw_heading && !trimmed.is_empty() {
            if let Some(cap) = crate::parser::SCHEDULED_RE.captures(line)
                && let Some(idx) = current_heading_idx
            {
                headings[idx].scheduled = cap.get(1).map(|m| m.as_str().to_string());
            }
            if let Some(cap) = crate::parser::DEADLINE_RE.captures(line)
                && let Some(idx) = current_heading_idx
            {
                headings[idx].deadline = cap.get(1).map(|m| m.as_str().to_string());
            }
            just_saw_heading = false;
        }
    }

    headings
}

#[derive(Serialize)]
pub struct NodeJson {
    pub uuid: String,
    pub title: String,
    pub path: String,
    pub filetags: Vec<String>,
    pub categories: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub headings: Option<Vec<HeadingJson>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub headings_count: Option<usize>,
}

impl NodeJson {
    fn from_node(node: &Node, content: Option<&str>, headings: Option<Vec<HeadingJson>>) -> Self {
        NodeJson {
            uuid: node.uuid.clone(),
            title: node.title.clone(),
            path: util::path_string(&node.path),
            filetags: node.filetags.clone(),
            categories: node.categories.clone(),
            content: content.map(std::string::ToString::to_string),
            headings_count: headings.as_ref().map(|h| h.len()),
            headings,
        }
    }
}

#[derive(Serialize)]
pub struct GetOutput {
    pub node: NodeJson,
    pub neighbors: HashMap<u32, NeighborOutput>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub estimated_tokens: Option<usize>,
}

#[derive(Serialize)]
pub struct NeighborOutput {
    pub outgoing: Vec<NodeJson>,
    pub incoming: Vec<NodeJson>,
}

pub struct GetOptions<'a> {
    pub target: Option<&'a str>,
    pub show_links: bool,
    pub show_headings: bool,
    pub no_content: bool,
    pub from_stdin: bool,
}

fn get_neighbor_map(graph: &Graph, uuid: &str) -> HashMap<u32, NeighborOutput> {
    let mut map = HashMap::new();
    if let Some(ns) = graph.get_neighbors(uuid, 1).get(&1) {
        let outgoing: Vec<NodeJson> = ns
            .outgoing
            .iter()
            .map(|n| NodeJson::from_node(n, None, None))
            .collect();
        let incoming: Vec<NodeJson> = ns
            .incoming
            .iter()
            .map(|n| NodeJson::from_node(n, None, None))
            .collect();
        map.insert(1, NeighborOutput { outgoing, incoming });
    }
    map
}

fn process_one_get(
    graph: &Graph,
    target: &str,
    show_links: bool,
    show_headings: bool,
    no_content: bool,
) -> Result<GetOutput> {
    let node = graph.resolve_target(target)?.clone();
    let neighbors = if show_links {
        get_neighbor_map(graph, &node.uuid)
    } else {
        HashMap::new()
    };

    let head_content = if show_headings {
        std::fs::read_to_string(&node.path).ok()
    } else {
        None
    };

    let node_content = if no_content {
        None
    } else {
        head_content.clone()
    };

    let headings = head_content.as_deref().map(parse_headings_from_content);

    let node_json = NodeJson::from_node(&node, node_content.as_deref(), headings);

    let full_content = std::fs::read_to_string(&node.path).ok();
    let estimated_tokens = full_content
        .as_deref()
        .map(|c| tokens::count_tokens(c, tokens::Encoding::Cl100kBase));

    Ok(GetOutput {
        node: node_json,
        neighbors,
        estimated_tokens,
    })
}

fn print_one_get_text(
    graph: &Graph,
    target: &str,
    show_links: bool,
    show_headings: bool,
    no_content: bool,
) -> Result<()> {
    let node = graph.resolve_target(target)?.clone();
    let neighbors = if show_links {
        Some(graph.get_neighbors(&node.uuid, 1))
    } else {
        None
    };

    let full_content = std::fs::read_to_string(&node.path).ok();
    let content_tokens = full_content
        .as_deref()
        .map(|c| tokens::count_tokens(c, tokens::Encoding::Cl100kBase));

    let node_content = if no_content {
        None
    } else {
        full_content.clone()
    };

    println!("Note: {}", node.title);
    println!("  UUID:   {}", node.uuid);
    println!("  Path:   {}", node.path.display());
    if let Some(t) = content_tokens {
        println!("  Content tokens: {t}");
    }
    if !node.filetags.is_empty() {
        println!("  Tags:   {}", node.filetags.join(", "));
    }
    if !node.categories.is_empty() {
        println!("  Cats:   {}", node.categories.join(", "));
    }

    if show_headings && let Some(ref content) = full_content {
        let headings = parse_headings_from_content(content);
        if !headings.is_empty() {
            println!();
            println!("--- Headings ---");
            for h in &headings {
                if let Some(ref uuid) = h.uuid {
                    println!("{} ({})", h.raw, uuid);
                } else {
                    println!("{}", h.raw);
                }
            }
            println!("--- End Headings ---");
        }
    }

    if let Some(content) = node_content
        && !no_content
    {
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
    let show_headings = opts.show_headings;
    let no_content = opts.no_content;

    let graph = Graph::load(config, db_cli)?;

    match ctx.format {
        OutputFormat::Text => {
            for target in &targets {
                print_one_get_text(&graph, target, show_links, show_headings, no_content)?;
                if targets.len() > 1 {
                    println!();
                }
            }
        }
        OutputFormat::Json => {
            let mut all_outputs = Vec::new();
            for target in &targets {
                all_outputs.push(process_one_get(
                    &graph,
                    target,
                    show_links,
                    show_headings,
                    no_content,
                )?);
            }
            if all_outputs.len() == 1 {
                ctx.print_json(&all_outputs[0])?;
            } else {
                ctx.print_json(&all_outputs)?;
            }
        }
        OutputFormat::Ndjson => {
            for target in &targets {
                let output =
                    process_one_get(&graph, target, show_links, show_headings, no_content)?;
                println!("{}", serde_json::to_string(&output)?);
            }
        }
    }

    Ok(())
}
