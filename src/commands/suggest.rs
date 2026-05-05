use crate::cli::OutputFormat;
use crate::config::Config;
use crate::graph::{Graph, Node};
use crate::output::OutputContext;
use crate::parser::Link;
use anyhow::Result;
use serde::Serialize;
use std::collections::{HashMap, HashSet};

type ScoredItem<'a> = (
    &'a crate::graph::Node,
    f64,
    Vec<String>,
    HashMap<String, f64>,
);

#[derive(Serialize)]
pub struct SuggestOutput {
    pub target: String,
    pub target_uuid: String,
    pub total: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub showed: Option<usize>,
    pub suggestions: Vec<Suggestion>,
}

#[derive(Serialize)]
pub struct Suggestion {
    pub uuid: String,
    pub title: String,
    pub path: String,
    pub score: f64,
    pub reasons: Vec<String>,
    pub filetags: Vec<String>,
    pub scores: HashMap<String, f64>,
}

#[allow(clippy::too_many_arguments)]
fn neighbor_relevance(
    neighbor_uuid: &str,
    graph: &Graph,
    target_node: &Node,
    target_keywords: &HashSet<String>,
    content_keywords: &HashSet<String>,
    target_tags: &HashSet<&str>,
    target_backlinks: &HashSet<&str>,
    target_outgoing: &HashSet<&str>,
) -> f64 {
    if neighbor_uuid == target_node.uuid {
        return 0.0;
    }
    let neighbor = match graph.nodes.get(neighbor_uuid) {
        Some(n) => n,
        None => return 0.0,
    };

    let mut score = 0.0;

    let neighbor_lower = neighbor.title.to_lowercase();
    let neighbor_words: Vec<&str> = neighbor_lower
        .split_whitespace()
        .filter(|w| w.len() > 2)
        .collect();
    let title_overlap = neighbor_words
        .iter()
        .filter(|w| target_keywords.iter().any(|kw| kw.as_str() == **w))
        .count();
    if title_overlap > 0 {
        score += title_overlap as f64 * 20.0;
    }

    if !content_keywords.is_empty()
        && let Ok(nc) = std::fs::read_to_string(&neighbor.path)
    {
        let ncl = nc.to_lowercase();
        let cm = content_keywords
            .iter()
            .filter(|kw| ncl.contains(kw.as_str()))
            .count();
        if cm > 0 {
            score += cm as f64 * 5.0;
        }
    }

    let tag_overlap = neighbor
        .filetags
        .iter()
        .filter(|t| target_tags.contains(t.as_str()))
        .count();
    if tag_overlap > 0 {
        score += tag_overlap as f64 * 25.0;
    }

    let cat_overlap = neighbor
        .categories
        .iter()
        .filter(|c| target_tags.contains(c.as_str()))
        .count();
    if cat_overlap > 0 {
        score += cat_overlap as f64 * 25.0;
    }

    let shared_backlinks = graph
        .backlinks
        .get(neighbor_uuid)
        .map(|v| {
            v.iter()
                .filter(|bl| target_backlinks.contains(bl.as_str()))
                .count()
        })
        .unwrap_or(0);
    if shared_backlinks > 0 {
        score += shared_backlinks as f64 * 15.0;
    }

    let shared_outgoing = neighbor
        .outgoing
        .iter()
        .filter_map(|l| {
            if let Link::Internal(u) = l {
                Some(u.as_str())
            } else {
                None
            }
        })
        .filter(|u| target_outgoing.contains(u))
        .count();
    if shared_outgoing > 0 {
        score += shared_outgoing as f64 * 12.0;
    }

    score
}

#[allow(clippy::cast_precision_loss, clippy::too_many_arguments)]
fn compute_scores<'a>(
    node: &'a crate::graph::Node,
    graph: &'a crate::graph::Graph,
    target_keywords: &HashSet<String>,
    content_keywords: &HashSet<String>,
    target_tags: &HashSet<&str>,
    target_backlinks: &HashSet<&str>,
    target_outgoing: &HashSet<&str>,
    exclude_orphans: bool,
) -> Vec<ScoredItem<'a>> {
    let mut scored: Vec<ScoredItem<'a>> = Vec::new();

    for other in graph.nodes.values() {
        if other.uuid == node.uuid {
            continue;
        }
        if exclude_orphans {
            let has_outgoing = other
                .outgoing
                .iter()
                .any(|l| matches!(l, Link::Internal(_)));
            let has_incoming = graph
                .backlinks
                .get(&other.uuid)
                .is_some_and(|b| !b.is_empty());
            if !has_outgoing && !has_incoming {
                continue;
            }
        }

        let mut score = 0.0;
        let mut reasons = Vec::new();
        let mut factor_scores: HashMap<String, f64> = HashMap::new();

        let other_lower = other.title.to_lowercase();
        let other_words: Vec<&str> = other_lower.split_whitespace().collect();
        let title_overlap: usize = other_words
            .iter()
            .filter(|w| target_keywords.iter().any(|kw| kw.as_str() == **w))
            .count();
        if title_overlap > 0 {
            let s = title_overlap as f64 * 20.0;
            score += s;
            factor_scores.insert("title".to_string(), s);
            reasons.push(format!("shared title: \"{}\"", other.title));
        }

        if !content_keywords.is_empty()
            && let Ok(other_content) = std::fs::read_to_string(&other.path)
        {
            let other_lc = other_content.to_lowercase();
            let content_match_count: usize = content_keywords
                .iter()
                .filter(|kw| other_lc.contains(kw.as_str()))
                .count();
            if content_match_count > 0 {
                let s = content_match_count as f64 * 5.0;
                score += s;
                *factor_scores.entry("content".to_string()).or_insert(0.0) += s;
                if title_overlap == 0 {
                    reasons.push(format!("{content_match_count} content keyword matches"));
                }
            }
        }

        let tag_overlap: usize = other
            .filetags
            .iter()
            .filter(|t| target_tags.contains(t.as_str()))
            .count();
        if tag_overlap > 0 {
            let s = tag_overlap as f64 * 25.0;
            score += s;
            factor_scores.insert("tags".to_string(), s);
            reasons.push("shared tags".to_string());
        }

        let cat_overlap: usize = other
            .categories
            .iter()
            .filter(|c| target_tags.contains(c.as_str()))
            .count();
        if cat_overlap > 0 {
            let s = cat_overlap as f64 * 25.0;
            score += s;
            factor_scores.insert("categories".to_string(), s);
            reasons.push("shared categories".to_string());
        }

        let other_backlinks: HashSet<&str> = graph
            .backlinks
            .get(&other.uuid)
            .map(|v| v.iter().map(std::string::String::as_str).collect())
            .unwrap_or_default();
        let shared_backlinks: usize = target_backlinks.intersection(&other_backlinks).count();
        if shared_backlinks > 0 {
            let s = shared_backlinks as f64 * 15.0;
            score += s;
            *factor_scores.entry("backlinks".to_string()).or_insert(0.0) += s;
            reasons.push(format!("{shared_backlinks} shared backlinks"));
        }

        let other_outgoing: HashSet<&str> = other
            .outgoing
            .iter()
            .filter_map(|l| {
                if let Link::Internal(u) = l {
                    Some(u.as_str())
                } else {
                    None
                }
            })
            .collect();
        let shared_outgoing: usize = target_outgoing.intersection(&other_outgoing).count();
        if shared_outgoing > 0 {
            let s = shared_outgoing as f64 * 12.0;
            score += s;
            *factor_scores.entry("outgoing".to_string()).or_insert(0.0) += s;
            reasons.push(format!("{shared_outgoing} shared outgoing"));
        }

        if let (Some(tp), Some(op)) = (node.path.parent(), other.path.parent())
            && tp == op
            && score > 0.0
        {
            let s = 5.0;
            score += s;
            *factor_scores.entry("directory".to_string()).or_insert(0.0) += s;
        }

        // Neighborhood relevance boost
        let mut total_neighbor_score = 0.0;
        let mut neighbor_count = 0;

        for link in &other.outgoing {
            if let Link::Internal(uuid) = link {
                total_neighbor_score += neighbor_relevance(
                    uuid,
                    graph,
                    node,
                    target_keywords,
                    content_keywords,
                    target_tags,
                    target_backlinks,
                    target_outgoing,
                );
                neighbor_count += 1;
            }
        }

        if let Some(incoming) = graph.backlinks.get(&other.uuid) {
            for uuid in incoming {
                total_neighbor_score += neighbor_relevance(
                    uuid,
                    graph,
                    node,
                    target_keywords,
                    content_keywords,
                    target_tags,
                    target_backlinks,
                    target_outgoing,
                );
                neighbor_count += 1;
            }
        }

        if neighbor_count > 0 {
            let avg = total_neighbor_score / neighbor_count as f64;
            let boost = ((avg / 100.0) - 0.3).clamp(-0.5, 1.0);
            if boost.abs() > 0.01 {
                factor_scores.insert("neighborhood".to_string(), boost);
                if boost > 0.0 {
                    reasons.push("relevant neighborhood".to_string());
                } else {
                    reasons.push("unrelated neighborhood".to_string());
                }
            }
            score *= 1.0 + boost;
        }

        if score > 0.0 {
            scored.push((other, score, reasons, factor_scores));
        }
    }

    scored
}

pub fn run(
    config: &Config,
    ctx: &OutputContext,
    target: Option<&str>,
    limit: Option<usize>,
    exclude_orphans: bool,
    db_cli: Option<&std::path::Path>,
) -> Result<()> {
    let target = target.ok_or_else(|| anyhow::anyhow!("No target specified. Provide a target"))?;

    let graph = Graph::load(config, db_cli)?;
    let node = graph
        .nodes
        .get(target)
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("Note not found: {target}"))?;

    let target_lower = node.title.to_lowercase();

    let target_keywords: HashSet<String> = target_lower
        .split_whitespace()
        .filter(|w| w.len() > 2)
        .map(std::string::ToString::to_string)
        .collect();

    let mut content_keywords: HashSet<String> = HashSet::new();
    if let Ok(content) = std::fs::read_to_string(&node.path) {
        let content_lower = content.to_lowercase();
        for word in content_lower.split_whitespace() {
            let clean: String = word
                .trim_matches(|c: char| !c.is_alphanumeric())
                .chars()
                .filter(|c| c.is_alphanumeric())
                .collect();
            if clean.len() > 3
                && !clean.starts_with("http")
                && !clean.starts_with("id")
                && !clean.starts_with("file")
            {
                content_keywords.insert(clean);
                if content_keywords.len() >= 50 {
                    break;
                }
            }
        }
    }

    let target_tags: HashSet<&str> = node
        .filetags
        .iter()
        .chain(node.categories.iter())
        .map(std::string::String::as_str)
        .collect();
    let target_backlinks: HashSet<&str> = graph
        .backlinks
        .get(&node.uuid)
        .map(|v| v.iter().map(std::string::String::as_str).collect())
        .unwrap_or_default();
    let target_outgoing: HashSet<&str> = node
        .outgoing
        .iter()
        .filter_map(|l| {
            if let Link::Internal(u) = l {
                Some(u.as_str())
            } else {
                None
            }
        })
        .collect();

    let mut scored = compute_scores(
        &node,
        &graph,
        &target_keywords,
        &content_keywords,
        &target_tags,
        &target_backlinks,
        &target_outgoing,
        exclude_orphans,
    );
    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    let total = scored.len();
    let showed = limit.map(|l| {
        let shown = scored.len().min(l);
        scored.truncate(l);
        shown
    });

    print_suggest_output(ctx, &node, &scored, total, showed)
}

fn print_suggest_output(
    ctx: &OutputContext,
    node: &crate::graph::Node,
    scored: &[ScoredItem<'_>],
    total: usize,
    showed: Option<usize>,
) -> Result<()> {
    let suggestions: Vec<Suggestion> = scored
        .iter()
        .map(|(n, s, r, fs)| Suggestion {
            uuid: n.uuid.clone(),
            title: n.title.clone(),
            path: n.path.to_string_lossy().to_string(),
            score: *s,
            reasons: r.clone(),
            filetags: n.filetags.clone(),
            scores: fs.clone(),
        })
        .collect();

    match ctx.format {
        OutputFormat::Text => {
            println!("Suggestions for \"{}\":", node.title);
            println!();
            for (i, (n, score, _reasons, fs)) in scored.iter().enumerate() {
                println!("{:3}. {}  (score: {:.1})", i + 1, n.title, score);
                println!("       UUID: {}", n.uuid);
                if !fs.is_empty() {
                    let mut factors: Vec<&str> = fs.keys().map(String::as_str).collect();
                    factors.sort();
                    println!("       Matches: {}", factors.join(", "));
                }
            }
        }
        OutputFormat::Json => {
            let output = SuggestOutput {
                target: node.title.clone(),
                target_uuid: node.uuid.clone(),
                total,
                showed,
                suggestions,
            };
            ctx.print_json(&output)?;
        }
        OutputFormat::Ndjson => {
            ctx.print_ndjson(&suggestions)?;
        }
    }

    Ok(())
}
