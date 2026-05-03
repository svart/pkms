use crate::config::Config;
use crate::graph::Graph;
use anyhow::Result;
use serde::Serialize;
use std::fmt::Write;

const DEFAULT_TEMPLATE: &str = "# {{title}}\nUUID: {{uuid}}\nPath: {{path}}\n{{#tags}}Tags: {{tags}}\n{{/tags}}{{#aliases}}Aliases: {{aliases}}\n{{/aliases}}\n--- Content ---\n{{content}}--- End Content ---\n\n{{neighbors}}{{backlinks}}";

#[derive(Serialize)]
pub struct ContextOutput {
    pub target: String,
    pub context: String,
    pub estimated_tokens: usize,
    pub depth: u32,
}

pub fn run(
    config: &Config,
    json: bool,
    quiet: bool,
    target: Option<&str>,
    depth: u32,
    max_tokens: Option<usize>,
    include_outgoing: Option<bool>,
    include_incoming: Option<bool>,
    template: Option<&str>,
    db_cli: Option<&std::path::Path>,
) -> Result<()> {
    let target = target.ok_or_else(|| anyhow::anyhow!("No target specified. Provide a target"))?;

    let graph = Graph::load(config, db_cli, false)?;
    let node = graph.resolve_target(target)?.clone();
    let content = std::fs::read_to_string(&node.path).unwrap_or_default();
    let neighbors = graph.get_neighbors(&node.uuid, depth);

    let show_outgoing = include_outgoing.unwrap_or(true);
    let show_incoming = include_incoming.unwrap_or(true);

    let mut neighbors_text = String::new();
    let mut backlinks_text = String::new();

    for d in 1..=depth {
        if let Some(ns) = neighbors.get(&d) {
            if show_outgoing && !ns.outgoing.is_empty() {
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

            if show_incoming && !ns.incoming.is_empty() {
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
    let aliases_str = node.aliases.join(", ");

    let tmpl = template.unwrap_or(DEFAULT_TEMPLATE);
    let ctx = render_template(
        tmpl,
        &ContextVars {
            title: &node.title,
            uuid: &node.uuid,
            path: &node.path.to_string_lossy(),
            tags: &tags_str,
            aliases: &aliases_str,
            content: &content,
            neighbors: &neighbors_text,
            backlinks: &backlinks_text,
        },
    );

    let ctx = if let Some(max) = max_tokens {
        truncate_by_tokens(&ctx, max)
    } else {
        ctx
    };

    let final_tokens = estimate_tokens(&ctx);

    if json {
        let output = ContextOutput {
            target: node.title,
            context: ctx,
            estimated_tokens: final_tokens,
            depth,
        };
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        println!("{ctx}");
        if !quiet {
            eprintln!(
                "[context: ~{} tokens, depth: {}, max_tokens: {}]",
                final_tokens,
                depth,
                max_tokens.map_or("unlimited".to_string(), |m| m.to_string())
            );
        }
    }

    Ok(())
}

struct ContextVars<'a> {
    title: &'a str,
    uuid: &'a str,
    path: &'a str,
    tags: &'a str,
    aliases: &'a str,
    content: &'a str,
    neighbors: &'a str,
    backlinks: &'a str,
}

fn render_template(template: &str, vars: &ContextVars) -> String {
    let mut result = template.to_string();

    // Simple conditional blocks: {{#key}}...{{/key}}
    // Keep content only if the value is non-empty
    let conditionals = [
        ("tags", vars.tags),
        ("aliases", vars.aliases),
        ("neighbors", vars.neighbors),
        ("backlinks", vars.backlinks),
    ];
    for (key, val) in &conditionals {
        let start_tag = format!("{{{{#{key}}}}}");
        let end_tag = format!("{{{{/{key}}}}}");
        if val.is_empty() {
            // Remove the entire block
            while let Some(start) = result.find(&start_tag) {
                if let Some(end) = result[start..].find(&end_tag) {
                    let end = start + end + end_tag.len();
                    result.replace_range(start..end, "");
                } else {
                    break;
                }
            }
        } else {
            // Remove tags but keep content
            result = result.replace(&start_tag, "");
            result = result.replace(&end_tag, "");
        }
    }

    // Simple variable replacement
    let replacements = [
        ("title", vars.title),
        ("uuid", vars.uuid),
        ("path", vars.path),
        ("tags", vars.tags),
        ("aliases", vars.aliases),
        ("content", vars.content),
        ("neighbors", vars.neighbors),
        ("backlinks", vars.backlinks),
    ];
    for (key, val) in &replacements {
        result = result.replace(&format!("{{{{{key}}}}}"), val);
    }

    result
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

fn estimate_tokens(text: &str) -> usize {
    let mut tokens = 0usize;
    let mut in_word = false;
    for c in text.chars() {
        if c.is_whitespace() || c == '\n' {
            in_word = false;
        } else if c.is_ascii() {
            if !in_word {
                tokens += 1;
                in_word = true;
            }
        } else {
            tokens += 2;
            in_word = false;
        }
    }
    tokens
}

fn truncate_by_tokens(text: &str, max_tokens: usize) -> String {
    let mut tokens = 0usize;
    let mut in_word = false;
    let pos = text.char_indices().position(|(_, c)| {
        if c.is_whitespace() || c == '\n' {
            in_word = false;
            false
        } else if c.is_ascii() {
            if !in_word {
                tokens += 1;
                in_word = true;
            }
            tokens > max_tokens
        } else {
            tokens += 2;
            in_word = false;
            tokens > max_tokens
        }
    });
    match pos {
        Some(p) => text[..p].to_string(),
        None => text.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_estimate_tokens_empty() {
        assert_eq!(estimate_tokens(""), 0);
    }

    #[test]
    fn test_estimate_tokens_short() {
        assert_eq!(estimate_tokens("hello world"), 2);
    }

    #[test]
    fn test_estimate_tokens_single_word() {
        assert_eq!(estimate_tokens("hello"), 1);
    }

    #[test]
    fn test_estimate_tokens_cjk() {
        assert_eq!(estimate_tokens("你好世界"), 8);
    }

    #[test]
    fn test_truncate_by_tokens_short() {
        let text = "hello world this is a test";
        assert_eq!(truncate_by_tokens(text, 100), text);
    }

    #[test]
    fn test_truncate_by_tokens_long() {
        let text = "aaaa bbbb cccc dddd";
        // 4 words → 4 tokens, truncating at 2 tokens → first 2 start positions
        let truncated = truncate_by_tokens(text, 2);
        assert!(truncated.len() < text.len());
    }

    #[test]
    fn test_render_template_basic() {
        let vars = ContextVars {
            title: "My Note",
            uuid: "abcd",
            path: "/a.org",
            tags: "tag1, tag2",
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
            aliases: "",
            content: "body",
            neighbors: "",
            backlinks: "",
        };
        let result = render_template("{{#tags}}Tags: {{tags}}{{/tags}}", &vars);
        assert_eq!(result, "Tags: mytag");
    }

    #[test]
    fn test_render_template_conditional_empty() {
        let vars = ContextVars {
            title: "Note",
            uuid: "x",
            path: "/x.org",
            tags: "",
            aliases: "",
            content: "body",
            neighbors: "",
            backlinks: "",
        };
        let result = render_template("before{{#tags}}Tags: {{tags}}{{/tags}}after", &vars);
        assert_eq!(result, "beforeafter");
    }

    #[test]
    fn test_default_template() {
        let vars = ContextVars {
            title: "My Note",
            uuid: "uu-id-1234",
            path: "/path/to/note.org",
            tags: "tag1",
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
