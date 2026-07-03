use crate::cli::OutputFormat;
use crate::command_context::CommandContext;
use crate::config::DbCommandConfig;
use crate::output::OutputContext;
use crate::util;
use anyhow::{Context, Result};
use pkms_org::graph::{Graph, Node};
use pkms_org::org_edit::parsed_heading_subtree_end_index;
use pkms_org::parser;
use pkms_org::tokens;
use serde::Serialize;
use std::collections::HashMap;
use std::fmt::Write;

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

impl From<&parser::Heading> for HeadingJson {
    fn from(h: &parser::Heading) -> Self {
        HeadingJson {
            level: h.level,
            title: h.title.clone(),
            todo_state: h.todo_state.as_ref().map(|state| state.to_string()),
            tags: h.tags.clone(),
            raw: h.raw.clone(),
            uuid: h.uuid.as_ref().map(|uuid| uuid.to_string()),
            priority: h.priority.map(|priority| priority.as_char()),
            scheduled: h.scheduled.clone(),
            deadline: h.deadline.clone(),
        }
    }
}

fn headings_from_content(content: &str) -> Vec<HeadingJson> {
    parser::parse_note(content)
        .headings
        .iter()
        .map(HeadingJson::from)
        .collect()
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
            uuid: node.uuid.to_string(),
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
    #[serde(skip)]
    pub text_content: Option<String>,
}

#[derive(Serialize)]
pub struct NeighborOutput {
    pub outgoing: Vec<NodeJson>,
    pub incoming: Vec<NodeJson>,
}

pub struct GetOptions {
    pub targets: Vec<String>,
    pub show_links: bool,
    pub show_headings: bool,
    pub heading: Option<String>,
    pub no_content: bool,
    pub encoding: tokens::Encoding,
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

fn heading_block_from_content(content: &str, heading_title: &str) -> Option<String> {
    let parsed = parser::parse_note(content);
    let heading_idx = parsed
        .headings
        .iter()
        .position(|heading| heading.title == heading_title)?;
    let heading = &parsed.headings[heading_idx];
    let start = heading.line_number.saturating_sub(1);
    let lines: Vec<&str> = content.lines().collect();
    let end = parsed_heading_subtree_end_index(&parsed.headings, heading, lines.len());

    Some(lines[start..end].join("\n"))
}

fn process_one_get(graph: &Graph, target: &str, opts: &GetOptions) -> Result<GetOutput> {
    let node = graph.resolve_target(target)?.clone();
    let neighbors = if opts.show_links {
        get_neighbor_map(graph, &node.uuid)
    } else {
        HashMap::new()
    };

    let full_content = match std::fs::read_to_string(&node.path) {
        Ok(content) => Some(content),
        Err(err) if opts.heading.is_some() => {
            return Err(err).with_context(|| {
                format!(
                    "Failed to read note content: {}",
                    util::path_string(&node.path)
                )
            });
        }
        Err(_) => None,
    };
    let heading_content = opts
        .heading
        .as_deref()
        .map(|title| {
            let content = full_content
                .as_deref()
                .expect("heading content requires note content");
            heading_block_from_content(content, title)
                .ok_or_else(|| anyhow::anyhow!("Heading not found: {title}"))
        })
        .transpose()?;

    let node_content = if opts.no_content {
        None
    } else if let Some(content) = &heading_content {
        Some(content.clone())
    } else if opts.show_headings {
        full_content.clone()
    } else {
        None
    };
    let text_content = if opts.no_content {
        None
    } else {
        heading_content.clone().or_else(|| full_content.clone())
    };

    let headings = if opts.show_headings {
        full_content.as_deref().map(headings_from_content)
    } else {
        None
    };

    let node_json = NodeJson::from_node(&node, node_content.as_deref(), headings);

    let estimated_tokens = heading_content
        .as_deref()
        .or(full_content.as_deref())
        .map(|c| tokens::count_tokens(c, opts.encoding));

    Ok(GetOutput {
        node: node_json,
        neighbors,
        estimated_tokens,
        text_content,
    })
}

pub fn execute(config: &DbCommandConfig, opts: &GetOptions) -> Result<Vec<GetOutput>> {
    let graph = Graph::load(&config.org)?;
    opts.targets
        .iter()
        .map(|target| process_one_get(&graph, target, opts))
        .collect()
}

pub fn render(ctx: &OutputContext, outputs: &[GetOutput]) -> Result<()> {
    match ctx.format {
        OutputFormat::Text => {
            print!("{}", render_text(outputs));
        }
        OutputFormat::Json => {
            if outputs.len() == 1 {
                ctx.print_json(&outputs[0])?;
            } else {
                ctx.print_json(outputs)?;
            }
        }
        OutputFormat::Ndjson => ctx.print_ndjson(outputs)?,
    }

    Ok(())
}

pub fn render_text(outputs: &[GetOutput]) -> String {
    outputs
        .iter()
        .map(render_one_text)
        .collect::<Vec<_>>()
        .join("\n")
}

fn render_one_text(output: &GetOutput) -> String {
    let mut text = String::new();

    let _ = writeln!(text, "Note: {}", output.node.title);
    let _ = writeln!(text, "  UUID:   {}", output.node.uuid);
    let _ = writeln!(text, "  Path:   {}", output.node.path);
    if let Some(t) = output.estimated_tokens {
        let _ = writeln!(text, "  Content tokens: {t}");
    }
    if !output.node.filetags.is_empty() {
        let _ = writeln!(text, "  Tags:   {}", output.node.filetags.join(", "));
    }
    if !output.node.categories.is_empty() {
        let _ = writeln!(text, "  Cats:   {}", output.node.categories.join(", "));
    }

    if let Some(headings) = &output.node.headings
        && !headings.is_empty()
    {
        text.push('\n');
        text.push_str("--- Headings ---\n");
        for h in headings {
            if let Some(ref uuid) = h.uuid {
                let _ = writeln!(text, "{} ({})", h.raw, uuid);
            } else {
                let _ = writeln!(text, "{}", h.raw);
            }
        }
        text.push_str("--- End Headings ---\n");
    }

    if let Some(content) = &output.text_content {
        text.push('\n');
        text.push_str("--- Content ---\n");
        let _ = writeln!(text, "{content}");
        text.push_str("--- End Content ---\n");
    }

    if let Some(ns) = output.neighbors.get(&1) {
        text.push('\n');
        if !ns.outgoing.is_empty() {
            text.push_str("Forward links:\n");
            for n in &ns.outgoing {
                let _ = writeln!(text, "  {} ({})", n.title, n.uuid);
            }
        }
        if !ns.incoming.is_empty() {
            text.push_str("Backlinks:\n");
            for n in &ns.incoming {
                let _ = writeln!(text, "  {} ({})", n.title, n.uuid);
            }
        }
        if ns.outgoing.is_empty() && ns.incoming.is_empty() {
            text.push_str("(no connections)\n");
        }
    }

    text
}

pub fn run(ctx: &CommandContext<'_>, opts: &GetOptions) -> Result<()> {
    let config = ctx.config().db_command_config();
    let outputs = execute(&config, opts)?;
    render(ctx.output(), &outputs)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(title: &str) -> NodeJson {
        NodeJson {
            uuid: "11111111-1111-4111-8111-111111111111".to_string(),
            title: title.to_string(),
            path: "/notes/a.org".to_string(),
            filetags: vec!["tag".to_string()],
            categories: vec!["cat".to_string()],
            content: None,
            headings: None,
            headings_count: None,
        }
    }

    #[test]
    fn renders_get_text_from_typed_output() {
        let output = GetOutput {
            node: node("Note A"),
            neighbors: HashMap::new(),
            estimated_tokens: Some(12),
            text_content: Some("Body text".to_string()),
        };

        let text = render_text(&[output]);

        assert!(text.contains("Note: Note A"));
        assert!(text.contains("  Content tokens: 12"));
        assert!(text.contains("  Tags:   tag"));
        assert!(text.contains("--- Content ---\nBody text\n--- End Content ---"));
    }

    #[test]
    fn renders_headings_and_links_from_typed_output() {
        let mut note = node("Note A");
        note.headings = Some(vec![HeadingJson {
            level: 1,
            title: "Heading".to_string(),
            todo_state: None,
            tags: vec![],
            raw: "* Heading".to_string(),
            uuid: Some("22222222-2222-4222-8222-222222222222".to_string()),
            priority: None,
            scheduled: None,
            deadline: None,
        }]);
        let mut neighbors = HashMap::new();
        neighbors.insert(
            1,
            NeighborOutput {
                outgoing: vec![node("Note B")],
                incoming: vec![],
            },
        );
        let output = GetOutput {
            node: note,
            neighbors,
            estimated_tokens: None,
            text_content: None,
        };

        let text = render_text(&[output]);

        assert!(text.contains("* Heading (22222222-2222-4222-8222-222222222222)"));
        assert!(text.contains("Forward links:\n  Note B"));
    }
}
