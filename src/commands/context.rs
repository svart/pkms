use crate::cli::OutputFormat;
use crate::config::Config;
use crate::graph::Graph;
use crate::output::OutputContext;
use crate::tokens;
use anyhow::Result;
use handlebars::Handlebars;
use serde::Serialize;
use std::fmt::Write;
use std::sync::LazyLock;

const DEFAULT_TEMPLATE: &str = "# {{title}}\nUUID: {{uuid}}\nPath: {{path}}\n{{#if tags}}Tags: {{tags}}\n{{/if}}{{#if categories}}Categories: {{categories}}\n{{/if}}{{#if aliases}}Aliases: {{aliases}}\n{{/if}}\n--- Content ---\n{{content}}--- End Content ---\n\n{{neighbors}}{{backlinks}}";

#[derive(Serialize)]
pub struct ContextOutput {
    pub target: String,
    pub context: String,
    pub estimated_tokens: usize,
    pub encoding: String,
    pub depth: u32,
}

pub struct ContextOptions {
    pub targets: Vec<String>,
    pub depth: u32,
    pub max_tokens: Option<usize>,
    pub encoding: tokens::Encoding,
}

fn build_context_output(
    graph: &Graph,
    target: &str,
    depth: u32,
    max_tokens: Option<usize>,
    encoding: tokens::Encoding,
) -> Result<ContextOutput> {
    let node = graph.resolve_target(target)?.clone();
    let content = std::fs::read_to_string(&node.path).unwrap_or_default();
    let neighbors = graph.get_neighbors(&node.uuid, depth);

    let mut neighbors_text = String::new();
    let mut backlinks_text = String::new();

    for d in 1..=depth {
        if let Some(ns) = neighbors.get(&d) {
            if !ns.outgoing.is_empty() {
                let _ = writeln!(neighbors_text, "=== Depth {d} ===");
                neighbors_text.push_str("Forward links:\n");
                for n in &ns.outgoing {
                    let short = truncate_content(&read_content(&n.path), 200);
                    let _ = write!(neighbors_text, "\n  → {} ({})\n", n.title, n.uuid);
                    let _ = writeln!(neighbors_text, "    Path: {}", n.path.display());
                    if !short.is_empty() {
                        let _ = write!(neighbors_text, "    {short}");
                    }
                }
                neighbors_text.push('\n');
            }

            if !ns.incoming.is_empty() {
                let _ = writeln!(backlinks_text, "=== Depth {d} ===");
                backlinks_text.push_str("Backlinks:\n");
                for n in &ns.incoming {
                    let _ = write!(backlinks_text, "\n  ← {} ({})\n", n.title, n.uuid);
                    let _ = writeln!(backlinks_text, "    Path: {}", n.path.display());
                }
                backlinks_text.push('\n');
            }
        }
    }

    let tags_str = node.filetags.join(", ");
    let cats_str = node.categories.join(", ");
    let aliases_str = node.aliases.join(", ");

    let rendered = render_template(
        DEFAULT_TEMPLATE,
        &ContextVars {
            title: &node.title,
            uuid: &node.uuid,
            path: &node.path.to_string_lossy(),
            tags: &tags_str,
            categories: &cats_str,
            aliases: &aliases_str,
            content: &content,
            neighbors: &neighbors_text,
            backlinks: &backlinks_text,
        },
    );

    let rendered = if let Some(max) = max_tokens {
        tokens::truncate_by_tokens(&rendered, max, encoding)
    } else {
        rendered
    };

    let final_tokens = tokens::count_tokens(&rendered, encoding);

    Ok(ContextOutput {
        target: node.title,
        context: rendered,
        estimated_tokens: final_tokens,
        encoding: encoding.to_string(),
        depth,
    })
}

pub fn run(config: &Config, ctx: &OutputContext, opts: &ContextOptions) -> Result<()> {
    let graph = Graph::load(config)?;

    match ctx.format {
        OutputFormat::Text => {
            for t in &opts.targets {
                let output =
                    build_context_output(&graph, t, opts.depth, opts.max_tokens, opts.encoding)?;
                println!("{}", output.context);
                eprintln!(
                    "[context: {} tokens, encoding: {}, depth: {}, max_tokens: {}]",
                    output.estimated_tokens,
                    output.encoding,
                    opts.depth,
                    opts.max_tokens
                        .map_or("unlimited".to_string(), |m| m.to_string())
                );
                if opts.targets.len() > 1 {
                    println!();
                }
            }
        }
        OutputFormat::Json => {
            let mut all_outputs = Vec::new();
            for t in &opts.targets {
                all_outputs.push(build_context_output(
                    &graph,
                    t,
                    opts.depth,
                    opts.max_tokens,
                    opts.encoding,
                )?);
            }
            if all_outputs.len() == 1 {
                ctx.print_json(&all_outputs[0])?;
            } else {
                ctx.print_json(&all_outputs)?;
            }
        }
        OutputFormat::Ndjson => {
            for t in &opts.targets {
                let output =
                    build_context_output(&graph, t, opts.depth, opts.max_tokens, opts.encoding)?;
                println!("{}", serde_json::to_string(&output)?);
            }
        }
    }

    Ok(())
}

#[derive(Serialize)]
struct ContextVars<'a> {
    title: &'a str,
    uuid: &'a str,
    path: &'a str,
    tags: &'a str,
    categories: &'a str,
    aliases: &'a str,
    content: &'a str,
    neighbors: &'a str,
    backlinks: &'a str,
}

static HANDLEBARS: LazyLock<Handlebars> = LazyLock::new(|| {
    let mut reg = Handlebars::new();
    reg.register_escape_fn(handlebars::no_escape);
    reg
});

fn render_template(template: &str, vars: &ContextVars) -> String {
    HANDLEBARS
        .render_template(template, vars)
        .unwrap_or_else(|e| {
            eprintln!("Template error: {e}");
            template.to_string()
        })
}

fn read_content(path: &std::path::Path) -> String {
    std::fs::read_to_string(path).unwrap_or_default()
}

fn truncate_content(s: &str, max_chars: usize) -> String {
    if s.len() <= max_chars {
        return s.to_string();
    }
    let mut truncated = s[..max_chars].to_string();
    truncated.push_str("...");
    truncated
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_template_basic() {
        let vars = ContextVars {
            title: "My Note",
            uuid: "abcd",
            path: "/a.org",
            tags: "tag1, tag2",
            categories: "",
            aliases: "",
            content: "some content",
            neighbors: "",
            backlinks: "",
        };
        let result = render_template("Title: {{title}}\nUUID: {{uuid}}", &vars);
        assert_eq!(result, "Title: My Note\nUUID: abcd");
    }

    #[test]
    fn test_render_template_conditional_present() {
        let vars = ContextVars {
            title: "Note",
            uuid: "x",
            path: "/x.org",
            tags: "mytag",
            categories: "",
            aliases: "",
            content: "body",
            neighbors: "",
            backlinks: "",
        };
        let result = render_template("{{#if tags}}Tags: {{tags}}{{/if}}", &vars);
        assert_eq!(result, "Tags: mytag");
    }

    #[test]
    fn test_render_template_conditional_empty() {
        let vars = ContextVars {
            title: "Note",
            uuid: "x",
            path: "/x.org",
            tags: "",
            categories: "",
            aliases: "",
            content: "body",
            neighbors: "",
            backlinks: "",
        };
        let result = render_template("before{{#if tags}}Tags: {{tags}}{{/if}}after", &vars);
        assert_eq!(result, "beforeafter");
    }

    #[test]
    fn test_default_template() {
        let vars = ContextVars {
            title: "My Note",
            uuid: "uu-id-1234",
            path: "/path/to/note.org",
            tags: "tag1",
            categories: "",
            aliases: "",
            content: "file content\nsecond line",
            neighbors: "\n  → Linked Note\n",
            backlinks: "",
        };
        let result = render_template(DEFAULT_TEMPLATE, &vars);
        assert!(result.contains("My Note"));
        assert!(result.contains("uu-id-1234"));
        assert!(result.contains("file content"));
        assert!(result.contains("Linked Note"));
        assert!(
            !result.contains("{{title}}"),
            "all placeholders should be replaced"
        );
        assert!(
            !result.contains("{{#tags}}"),
            "conditional tags tag should be removed"
        );
        assert!(
            !result.contains("{{/tags}}"),
            "conditional tags end should be removed"
        );
        assert!(result.contains("Tags: tag1"));
    }
}
