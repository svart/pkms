use crate::cli::{OutputFormat, QueryArgs};
use crate::config::ResolvedConfig;
use crate::graph::Graph;
use crate::graph::search::SearchFields;
use crate::output::OutputContext;
use anyhow::Result;
use serde::Serialize;
use std::fmt::Write;

#[derive(Serialize)]
pub struct QueryOutput {
    pub query: String,
    pub total_results: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub showed: Option<usize>,
    pub results: Vec<QueryResultEntry>,
}

#[derive(Serialize)]
pub struct QueryResultEntry {
    pub uuid: String,
    pub title: String,
    pub path: String,
    pub filetags: Vec<String>,
    pub score: f64,
    pub matches: Vec<String>,
    pub content_matches: Vec<ContextLine>,
}

#[derive(Serialize)]
pub struct ContextLine {
    pub line: usize,
    pub text: String,
}

pub struct QueryOptions {
    pub terms: String,
    pub limit: Option<usize>,
    pub tags: bool,
    pub title: bool,
    pub content: bool,
    pub todos: bool,
}

impl TryFrom<&QueryArgs> for QueryOptions {
    type Error = anyhow::Error;

    fn try_from(args: &QueryArgs) -> Result<Self> {
        Ok(QueryOptions {
            terms: args
                .terms
                .clone()
                .ok_or_else(|| anyhow::anyhow!("No search terms specified. Provide terms"))?,
            limit: args.limit,
            tags: args.tags,
            title: args.title,
            content: args.content,
            todos: args.todos,
        })
    }
}

pub fn run(config: &ResolvedConfig, ctx: &OutputContext, opts: &QueryOptions) -> Result<()> {
    let output = execute(config, opts)?;
    render(ctx, &output)
}

pub fn execute(config: &ResolvedConfig, opts: &QueryOptions) -> Result<QueryOutput> {
    let graph = Graph::load(config)?;

    let mut combined = search_by_text(&graph, &opts.terms, opts.tags, opts.title, opts.content)?;

    if opts.todos {
        combined.retain(|r| graph.nodes.get(&r.uuid).is_some_and(|n| n.has_todos));
    }

    let total_results = combined.len();
    let showed = opts.limit.map(|l| {
        let shown = combined.len().min(l);
        combined.truncate(l);
        shown
    });

    Ok(QueryOutput {
        query: opts.terms.clone(),
        total_results,
        showed,
        results: combined,
    })
}

fn search_by_text(
    graph: &Graph,
    terms: &str,
    only_tags: bool,
    only_title: bool,
    only_content: bool,
) -> Result<Vec<QueryResultEntry>> {
    let search_title = only_title || (!only_tags && !only_content);
    let search_tags = only_tags || (!only_title && !only_content);
    let search_content = only_content || (!only_title && !only_tags);

    let mut combined: Vec<QueryResultEntry> = Vec::new();

    if search_title || search_tags {
        let fields = SearchFields {
            title: search_title,
            alias: search_title,
            ref_: search_title,
            tag: search_tags,
            category: search_tags,
        };
        let title_results = graph.search(terms, &fields);
        for (node, score, matches) in title_results {
            combined.push(QueryResultEntry {
                uuid: node.uuid.clone(),
                title: node.title.clone(),
                path: node.path.display().to_string(),
                filetags: node.filetags.clone(),
                score,
                matches,
                content_matches: vec![],
            });
        }
    }

    if search_content {
        let content_results = graph.search_content(terms);
        for result in content_results {
            let ctx_lines: Vec<ContextLine> = result
                .lines
                .iter()
                .map(|l| {
                    let (line_str, text) = l.split_once(": ").unwrap_or(("0", l));
                    ContextLine {
                        line: line_str.parse().unwrap_or(0),
                        text: text.to_string(),
                    }
                })
                .collect();
            if let Some(existing) = combined.iter_mut().find(|r| r.uuid == result.node.uuid) {
                existing.content_matches = ctx_lines;
            } else {
                combined.push(QueryResultEntry {
                    uuid: result.node.uuid.clone(),
                    title: result.node.title.clone(),
                    path: result.node.path.display().to_string(),
                    filetags: result.node.filetags.clone(),
                    score: 1.0,
                    matches: vec!["content".to_string()],
                    content_matches: ctx_lines,
                });
            }
        }
    }

    combined.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    Ok(combined)
}

pub fn render(ctx: &OutputContext, output: &QueryOutput) -> Result<()> {
    match ctx.format {
        OutputFormat::Text => {
            print!("{}", render_text(output));
        }
        OutputFormat::Json => {
            ctx.print_json(output)?;
        }
        OutputFormat::Ndjson => {
            ctx.print_ndjson(&output.results)?;
        }
    }

    Ok(())
}

pub fn render_text(output: &QueryOutput) -> String {
    let mut text = String::new();

    let _ = writeln!(text, "Query: {}", output.query);
    if output.total_results == output.results.len() {
        let _ = writeln!(text, "Results: {}", output.results.len());
    } else {
        let _ = writeln!(
            text,
            "Results: {}, showed: {}",
            output.total_results,
            output.results.len()
        );
    }
    text.push('\n');
    for (i, r) in output.results.iter().enumerate() {
        let _ = writeln!(text, "{:3}. {}  (score: {:.1})", i + 1, r.title, r.score);
        let _ = writeln!(text, "       UUID: {}", r.uuid);
        if !r.matches.is_empty() {
            let _ = writeln!(text, "       Matches: {}", r.matches.join(", "));
        }
        if !r.content_matches.is_empty() {
            for cm in &r.content_matches[..std::cmp::min(3, r.content_matches.len())] {
                let _ = writeln!(text, "       > {}", cm.text);
            }
            if r.content_matches.len() > 3 {
                let _ = writeln!(text, "       ... and {} more", r.content_matches.len() - 3);
            }
        }
    }

    text
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(title: &str, score: f64) -> QueryResultEntry {
        QueryResultEntry {
            uuid: "11111111-1111-4111-8111-111111111111".to_string(),
            title: title.to_string(),
            path: "/notes/a.org".to_string(),
            filetags: vec!["tag".to_string()],
            score,
            matches: vec!["title".to_string()],
            content_matches: vec![],
        }
    }

    #[test]
    fn renders_query_text_from_typed_output() {
        let output = QueryOutput {
            query: "alpha".to_string(),
            total_results: 1,
            showed: None,
            results: vec![entry("Alpha Note", 10.0)],
        };

        let text = render_text(&output);

        assert!(text.contains("Query: alpha"));
        assert!(text.contains("Results: 1"));
        assert!(text.contains("  1. Alpha Note  (score: 10.0)"));
        assert!(text.contains("       Matches: title"));
    }

    #[test]
    fn renders_limited_result_count_from_typed_output() {
        let output = QueryOutput {
            query: "alpha".to_string(),
            total_results: 4,
            showed: Some(1),
            results: vec![entry("Alpha Note", 10.0)],
        };

        let text = render_text(&output);

        assert!(text.contains("Results: 4, showed: 1"));
    }

    #[test]
    fn renders_first_three_content_matches() {
        let mut result = entry("Alpha Note", 1.0);
        result.content_matches = vec![
            ContextLine {
                line: 1,
                text: "one".to_string(),
            },
            ContextLine {
                line: 2,
                text: "two".to_string(),
            },
            ContextLine {
                line: 3,
                text: "three".to_string(),
            },
            ContextLine {
                line: 4,
                text: "four".to_string(),
            },
        ];
        let output = QueryOutput {
            query: "alpha".to_string(),
            total_results: 1,
            showed: None,
            results: vec![result],
        };

        let text = render_text(&output);

        assert!(text.contains("       > one"));
        assert!(text.contains("       > three"));
        assert!(!text.contains("       > four"));
        assert!(text.contains("       ... and 1 more"));
    }
}
