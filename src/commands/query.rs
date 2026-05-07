use crate::cli::OutputFormat;
use crate::config::Config;
use crate::graph::Graph;
use crate::graph::search::SearchFields;
use crate::output::OutputContext;
use anyhow::Result;
use serde::Serialize;

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

#[allow(clippy::too_many_arguments)]
pub fn run(
    config: &Config,
    ctx: &OutputContext,
    terms: Option<&str>,
    limit: Option<usize>,
    only_tags: bool,
    only_title: bool,
    only_content: bool,
    db_cli: Option<&std::path::Path>,
) -> Result<()> {
    let terms = terms.ok_or_else(|| anyhow::anyhow!("No search terms specified. Provide terms"))?;
    let search_title = only_title || (!only_tags && !only_content);
    let search_tags = only_tags || (!only_title && !only_content);
    let search_content = only_content || (!only_title && !only_tags);

    let graph = Graph::load(config, db_cli)?;

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
                path: node.path.to_string_lossy().to_string(),
                filetags: node.filetags.clone(),
                score,
                matches,
                content_matches: vec![],
            });
        }
    }

    if search_content {
        let content_results = graph.search_content(terms);
        for (uuid, title, lines) in content_results {
            let ctx_lines: Vec<ContextLine> = lines
                .iter()
                .map(|l| {
                    let (line_str, text) = l.split_once(": ").unwrap_or(("0", l));
                    ContextLine {
                        line: line_str.parse().unwrap_or(0),
                        text: text.to_string(),
                    }
                })
                .collect();
            if let Some(existing) = combined.iter_mut().find(|r| r.uuid == uuid) {
                existing.content_matches = ctx_lines;
            } else {
                combined.push(QueryResultEntry {
                    uuid,
                    title,
                    path: String::new(),
                    filetags: vec![],
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

    let total_results = combined.len();
    let showed = limit.map(|l| {
        let shown = combined.len().min(l);
        combined.truncate(l);
        shown
    });

    print_query_output(ctx, terms, combined, total_results, showed)?;

    Ok(())
}

fn print_query_output(
    ctx: &OutputContext,
    terms: &str,
    results: Vec<QueryResultEntry>,
    total_results: usize,
    showed: Option<usize>,
) -> Result<()> {
    match ctx.format {
        OutputFormat::Text => {
            println!("Query: {terms}");
            if total_results == results.len() {
                println!("Results: {}", results.len());
            } else {
                println!("Results: {}, showed: {}", total_results, results.len());
            }
            println!();
            for (i, r) in results.iter().enumerate() {
                println!("{:3}. {}  (score: {:.1})", i + 1, r.title, r.score);
                println!("       UUID: {}", r.uuid);
                if !r.matches.is_empty() {
                    println!("       Matches: {}", r.matches.join(", "));
                }
                if !r.content_matches.is_empty() {
                    for cm in &r.content_matches[..std::cmp::min(3, r.content_matches.len())] {
                        println!("       > {}", cm.text);
                    }
                    if r.content_matches.len() > 3 {
                        println!("       ... and {} more", r.content_matches.len() - 3);
                    }
                }
            }
        }
        OutputFormat::Json => {
            ctx.print_json(&QueryOutput {
                query: terms.to_string(),
                total_results,
                showed,
                results,
            })?;
        }
        OutputFormat::Ndjson => {
            ctx.print_ndjson(&results)?;
        }
    }

    Ok(())
}
