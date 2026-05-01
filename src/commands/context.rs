use crate::config::Config;
use crate::graph::Graph;
use anyhow::Result;
use serde::Serialize;

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
    target: &str,
    depth: u32,
    max_tokens: Option<usize>,
    db_cli: Option<&std::path::Path>,
) -> Result<()> {
    let graph = Graph::load(config, db_cli, false)?;

    let node = graph.resolve_target(target)?.clone();

    let content = std::fs::read_to_string(&node.path).unwrap_or_default();
    let neighbors = graph.get_neighbors(&node.uuid, depth);

    let mut ctx = String::new();

    ctx.push_str(&format!("# {}\n", node.title));
    ctx.push_str(&format!("UUID: {}\n", node.uuid));
    ctx.push_str(&format!("Path: {}\n", node.path.display()));
    if !node.filetags.is_empty() {
        ctx.push_str(&format!("Tags: {}\n", node.filetags.join(", ")));
    }
    if !node.aliases.is_empty() {
        ctx.push_str(&format!("Aliases: {}\n", node.aliases.join(", ")));
    }
    ctx.push('\n');

    ctx.push_str("--- Content ---\n");
    ctx.push_str(&content);
    if !content.ends_with('\n') {
        ctx.push('\n');
    }
    ctx.push_str("--- End Content ---\n\n");

    for d in 1..=depth {
        if let Some(ns) = neighbors.get(&d) {
            if !ns.outgoing.is_empty() || !ns.incoming.is_empty() {
                ctx.push_str(&format!("=== Depth {} ===\n", d));

                if !ns.outgoing.is_empty() {
                    ctx.push_str("Forward links:\n");
                    for n in &ns.outgoing {
                        let short = truncate_content(&read_content(&n.path), 200);
                        ctx.push_str(&format!("\n  → {} ({})\n", n.title, n.uuid));
                        ctx.push_str(&format!("    Path: {}\n", n.path.display()));
                        if !short.is_empty() {
                            ctx.push_str(&format!("    {}", short));
                        }
                    }
                }

                if !ns.incoming.is_empty() {
                    ctx.push_str("\nBacklinks:\n");
                    for n in &ns.incoming {
                        ctx.push_str(&format!("\n  ← {} ({})\n", n.title, n.uuid));
                        ctx.push_str(&format!("    Path: {}\n", n.path.display()));
                    }
                }
                ctx.push('\n');
            }
        }
    }

    if let Some(max) = max_tokens {
        ctx = truncate_by_tokens(&ctx, max);
    }

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
        println!("{}", ctx);
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
    let words: Vec<&str> = text.split_whitespace().collect();
    let word_count = words.len();
    let avg_word_len: f64 = words
        .iter()
        .map(|w| w.len() as f64)
        .sum::<f64>()
        / word_count.max(1) as f64;
    (word_count as f64 * (1.0 + avg_word_len / 10.0)).round() as usize
}

fn truncate_by_tokens(text: &str, max_tokens: usize) -> String {
    let words: Vec<&str> = text.split_whitespace().collect();
    let mut token_count = 0;
    let mut result = Vec::new();
    for word in words {
        let word_tokens = 1 + (word.len() as f64 / 10.0).ceil() as usize;
        if token_count + word_tokens > max_tokens {
            break;
        }
        token_count += word_tokens;
        result.push(word);
    }
    result.join(" ")
}
