use crate::cli::OutputFormat;
use crate::config::ResolvedConfig;
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
    #[serde(skip)]
    pub max_tokens: Option<usize>,
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
            title: node.title.clone(),
            uuid: node.uuid.clone(),
            path: node.path.to_string_lossy().to_string(),
            tags: tags_str,
            categories: cats_str,
            aliases: aliases_str,
            content: content.clone(),
            neighbors: neighbors_text.clone(),
            backlinks: backlinks_text.clone(),
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
        max_tokens,
    })
}

pub fn run(config: &ResolvedConfig, ctx: &OutputContext, opts: &ContextOptions) -> Result<()> {
    let outputs = execute(config, opts)?;
    render(ctx, &outputs)
}

pub fn execute(config: &ResolvedConfig, opts: &ContextOptions) -> Result<Vec<ContextOutput>> {
    let graph = Graph::load(config)?;

    opts.targets
        .iter()
        .map(|target| {
            build_context_output(&graph, target, opts.depth, opts.max_tokens, opts.encoding)
        })
        .collect()
}

pub fn render(ctx: &OutputContext, outputs: &[ContextOutput]) -> Result<()> {
    match ctx.format {
        OutputFormat::Text => {
            print!("{}", render_text(outputs));
            for output in outputs {
                eprintln!("{}", render_summary(output));
            }
        }
        OutputFormat::Json => {
            ctx.print_json_adaptive(outputs)?;
        }
        OutputFormat::Ndjson => {
            ctx.print_ndjson(outputs)?;
        }
    }

    Ok(())
}

pub fn render_text(outputs: &[ContextOutput]) -> String {
    outputs
        .iter()
        .map(|output| format!("{}\n", output.context))
        .collect::<Vec<_>>()
        .join("\n")
}

fn render_summary(output: &ContextOutput) -> String {
    format!(
        "[context: {} tokens, encoding: {}, depth: {}, max_tokens: {}]",
        output.estimated_tokens,
        output.encoding,
        output.depth,
        output
            .max_tokens
            .map_or("unlimited".to_string(), |m| m.to_string())
    )
}

#[derive(Serialize)]
struct ContextVars {
    title: String,
    uuid: String,
    path: String,
    tags: String,
    categories: String,
    aliases: String,
    content: String,
    neighbors: String,
    backlinks: String,
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

    fn output(context: &str) -> ContextOutput {
        ContextOutput {
            target: "Note".to_string(),
            context: context.to_string(),
            estimated_tokens: 12,
            encoding: "cl100k".to_string(),
            depth: 2,
            max_tokens: Some(100),
        }
    }

    #[test]
    fn renders_context_text_from_typed_output() {
        let text = render_text(&[output("first"), output("second")]);

        assert_eq!(text, "first\n\nsecond\n");
    }

    #[test]
    fn renders_context_summary_from_typed_output() {
        let summary = render_summary(&output("context"));

        assert_eq!(
            summary,
            "[context: 12 tokens, encoding: cl100k, depth: 2, max_tokens: 100]"
        );
    }

    #[test]
    fn test_render_template_basic() {
        let vars = ContextVars {
            title: "My Note".to_string(),
            uuid: "abcd".to_string(),
            path: "/a.org".to_string(),
            tags: "tag1, tag2".to_string(),
            categories: String::new(),
            aliases: String::new(),
            content: "some content".to_string(),
            neighbors: String::new(),
            backlinks: String::new(),
        };
        let result = render_template("Title: {{title}}\nUUID: {{uuid}}", &vars);
        assert_eq!(result, "Title: My Note\nUUID: abcd");
    }

    #[test]
    fn test_render_template_conditional_present() {
        let vars = ContextVars {
            title: "Note".to_string(),
            uuid: "x".to_string(),
            path: "/x.org".to_string(),
            tags: "mytag".to_string(),
            categories: String::new(),
            aliases: String::new(),
            content: "body".to_string(),
            neighbors: String::new(),
            backlinks: String::new(),
        };
        let result = render_template("{{#if tags}}Tags: {{tags}}{{/if}}", &vars);
        assert_eq!(result, "Tags: mytag");
    }

    #[test]
    fn test_render_template_conditional_empty() {
        let vars = ContextVars {
            title: "Note".to_string(),
            uuid: "x".to_string(),
            path: "/x.org".to_string(),
            tags: String::new(),
            categories: String::new(),
            aliases: String::new(),
            content: "body".to_string(),
            neighbors: String::new(),
            backlinks: String::new(),
        };
        let result = render_template("before{{#if tags}}Tags: {{tags}}{{/if}}after", &vars);
        assert_eq!(result, "beforeafter");
    }

    #[test]
    fn test_default_template() {
        let vars = ContextVars {
            title: "My Note".to_string(),
            uuid: "uu-id-1234".to_string(),
            path: "/path/to/note.org".to_string(),
            tags: "tag1".to_string(),
            categories: String::new(),
            aliases: String::new(),
            content: "file content\nsecond line".to_string(),
            neighbors: "\n  → Linked Note\n".to_string(),
            backlinks: String::new(),
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
