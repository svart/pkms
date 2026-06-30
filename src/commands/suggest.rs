use crate::cli::OutputFormat;
use crate::command_context::CommandContext;
use crate::config::ResolvedConfig;
use crate::graph::{Graph, Node};
use crate::output::OutputContext;
use crate::parser::{HEADING_RE, Link};
use anyhow::Result;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::fmt::Write;

const TITLE_OVERLAP_WEIGHT: f64 = 20.0;
const CONTENT_MATCH_WEIGHT: f64 = 5.0;
const TAG_OVERLAP_WEIGHT: f64 = 25.0;
const CATEGORY_OVERLAP_WEIGHT: f64 = 25.0;
const BACKLINK_OVERLAP_WEIGHT: f64 = 15.0;
const OUTGOING_OVERLAP_WEIGHT: f64 = 12.0;
const DIRECTORY_PROXIMITY_WEIGHT: f64 = 5.0;
const NEIGHBOR_BOOST_DENOM: f64 = 100.0;
const MAX_CONTENT_KEYWORDS: usize = 50;

type SuggestResult = (Node, Vec<Suggestion>, usize, Option<usize>, Option<String>);

fn find_heading_title_for_uuid(content: &str, heading_uuid: &str) -> Option<String> {
    let lines: Vec<&str> = content.lines().collect();
    for (i, line) in lines.iter().enumerate() {
        if line.contains(heading_uuid) && i > 0 {
            // The heading line should be the previous line
            let prev = lines[i - 1].trim();
            if let Some(cap) = HEADING_RE.captures(prev) {
                let title = cap.get(4).map_or("", |m| m.as_str()).trim();
                if !title.is_empty() {
                    return Some(title.to_string());
                }
            }
            // PROPERTIES: might be on the line after the heading,
            // so check two lines back
            if i > 1 {
                let prev2 = lines[i - 2].trim();
                if let Some(cap) = HEADING_RE.captures(prev2) {
                    let title = cap.get(4).map_or("", |m| m.as_str()).trim();
                    if !title.is_empty() {
                        return Some(title.to_string());
                    }
                }
            }
        }
    }
    None
}

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
    #[serde(skip)]
    pub target_heading_context: Option<String>,
    #[serde(skip)]
    pub ndjson_target_uuid: Option<String>,
    #[serde(skip)]
    pub score_precision: usize,
}

#[derive(Clone, Serialize)]
pub struct Suggestion {
    pub uuid: String,
    pub title: String,
    pub path: String,
    pub score: f64,
    pub reasons: Vec<String>,
    pub filetags: Vec<String>,
    pub scores: HashMap<String, f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_uuid: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub heading_context: Option<String>,
}

struct SuggestionContext<'a> {
    target_keywords: &'a HashSet<String>,
    content_keywords: &'a HashSet<String>,
    target_tags: &'a HashSet<&'a str>,
    target_backlinks: &'a HashSet<&'a str>,
    target_outgoing: &'a HashSet<&'a str>,
}

fn neighbor_relevance(
    neighbor_uuid: &str,
    graph: &Graph,
    target_node: &Node,
    ctx: &SuggestionContext,
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
        .filter(|w| ctx.target_keywords.iter().any(|kw| kw.as_str() == **w))
        .count();
    if title_overlap > 0 {
        score += title_overlap as f64 * TITLE_OVERLAP_WEIGHT;
    }

    if !ctx.content_keywords.is_empty()
        && let Some(content) = node_content(graph, neighbor)
    {
        let ncl = content.to_lowercase();
        let cm = ctx
            .content_keywords
            .iter()
            .filter(|kw| ncl.contains(kw.as_str()))
            .count();
        if cm > 0 {
            score += cm as f64 * CONTENT_MATCH_WEIGHT;
        }
    }

    let tag_overlap = neighbor
        .filetags
        .iter()
        .filter(|t| ctx.target_tags.contains(t.as_str()))
        .count();
    if tag_overlap > 0 {
        score += tag_overlap as f64 * TAG_OVERLAP_WEIGHT;
    }

    let cat_overlap = neighbor
        .categories
        .iter()
        .filter(|c| ctx.target_tags.contains(c.as_str()))
        .count();
    if cat_overlap > 0 {
        score += cat_overlap as f64 * CATEGORY_OVERLAP_WEIGHT;
    }

    let shared_backlinks = graph
        .backlinks
        .get(neighbor_uuid)
        .map(|v| {
            v.iter()
                .filter(|bl| ctx.target_backlinks.contains(bl.as_str()))
                .count()
        })
        .unwrap_or(0);
    if shared_backlinks > 0 {
        score += shared_backlinks as f64 * BACKLINK_OVERLAP_WEIGHT;
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
        .filter(|u| ctx.target_outgoing.contains(u))
        .count();
    if shared_outgoing > 0 {
        score += shared_outgoing as f64 * OUTGOING_OVERLAP_WEIGHT;
    }

    score
}

fn is_orphan(node: &crate::graph::Node, graph: &Graph) -> bool {
    let has_outgoing = node.outgoing.iter().any(|l| matches!(l, Link::Internal(_)));
    let has_incoming = graph
        .backlinks
        .get(&node.uuid)
        .is_some_and(|b| !b.is_empty());
    !has_outgoing && !has_incoming
}

fn score_title_overlap(
    other: &crate::graph::Node,
    ctx: &SuggestionContext,
    reasons: &mut Vec<String>,
) -> f64 {
    let other_lower = other.title.to_lowercase();
    let other_words: Vec<&str> = other_lower.split_whitespace().collect();
    let overlap: usize = other_words
        .iter()
        .filter(|w| ctx.target_keywords.iter().any(|kw| kw.as_str() == **w))
        .count();
    if overlap > 0 {
        reasons.push(format!("shared title: \"{}\"", other.title));
        overlap as f64 * TITLE_OVERLAP_WEIGHT
    } else {
        0.0
    }
}

fn score_content_match(
    other: &crate::graph::Node,
    graph: &Graph,
    ctx: &SuggestionContext,
    title_overlap: bool,
    reasons: &mut Vec<String>,
) -> f64 {
    if ctx.content_keywords.is_empty() {
        return 0.0;
    }
    let Some(other_content) = node_content(graph, other) else {
        return 0.0;
    };
    let other_lc = other_content.to_lowercase();
    let count: usize = ctx
        .content_keywords
        .iter()
        .filter(|kw| other_lc.contains(kw.as_str()))
        .count();
    if count > 0 {
        if !title_overlap {
            reasons.push(format!("{count} content keyword matches"));
        }
        count as f64 * CONTENT_MATCH_WEIGHT
    } else {
        0.0
    }
}

fn score_tag_overlap(
    other: &crate::graph::Node,
    ctx: &SuggestionContext,
    reasons: &mut Vec<String>,
) -> f64 {
    let tag_overlap: usize = other
        .filetags
        .iter()
        .filter(|t| ctx.target_tags.contains(t.as_str()))
        .count();
    let cat_overlap: usize = other
        .categories
        .iter()
        .filter(|c| ctx.target_tags.contains(c.as_str()))
        .count();
    let total = tag_overlap + cat_overlap;
    if total > 0 {
        reasons.push("shared tags".to_string());
        total as f64 * TAG_OVERLAP_WEIGHT
    } else {
        0.0
    }
}

fn score_backlink_overlap(
    other: &crate::graph::Node,
    graph: &Graph,
    ctx: &SuggestionContext,
    reasons: &mut Vec<String>,
) -> f64 {
    let other_backlinks: HashSet<&str> = graph
        .backlinks
        .get(&other.uuid)
        .map(|v| v.iter().map(std::string::String::as_str).collect())
        .unwrap_or_default();
    let shared: usize = ctx.target_backlinks.intersection(&other_backlinks).count();
    if shared > 0 {
        reasons.push(format!("{shared} shared backlinks"));
        shared as f64 * BACKLINK_OVERLAP_WEIGHT
    } else {
        0.0
    }
}

fn score_outgoing_overlap(
    other: &crate::graph::Node,
    ctx: &SuggestionContext,
    reasons: &mut Vec<String>,
) -> f64 {
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
    let shared: usize = ctx.target_outgoing.intersection(&other_outgoing).count();
    if shared > 0 {
        reasons.push(format!("{shared} shared outgoing"));
        shared as f64 * OUTGOING_OVERLAP_WEIGHT
    } else {
        0.0
    }
}

fn score_directory_proximity(node: &crate::graph::Node, other: &crate::graph::Node) -> f64 {
    if let (Some(tp), Some(op)) = (node.path.parent(), other.path.parent())
        && tp == op
    {
        DIRECTORY_PROXIMITY_WEIGHT
    } else {
        0.0
    }
}

#[allow(clippy::cast_precision_loss)]
fn score_neighborhood(
    other: &crate::graph::Node,
    graph: &Graph,
    node: &crate::graph::Node,
    ctx: &SuggestionContext,
    reasons: &mut Vec<String>,
) -> f64 {
    let mut total_neighbor_score = 0.0;
    let mut neighbor_count = 0;

    for link in &other.outgoing {
        if let Link::Internal(uuid) = link {
            total_neighbor_score += neighbor_relevance(uuid, graph, node, ctx);
            neighbor_count += 1;
        }
    }

    if let Some(incoming) = graph.backlinks.get(&other.uuid) {
        for uuid in incoming {
            total_neighbor_score += neighbor_relevance(uuid, graph, node, ctx);
            neighbor_count += 1;
        }
    }

    if neighbor_count == 0 {
        return 0.0;
    }

    let avg = total_neighbor_score / neighbor_count as f64;
    let boost = ((avg / NEIGHBOR_BOOST_DENOM) - 0.3).clamp(-0.5, 1.0);
    if boost.abs() <= 0.01 {
        return 0.0;
    }
    if boost > 0.0 {
        reasons.push("relevant neighborhood".to_string());
    } else {
        reasons.push("unrelated neighborhood".to_string());
    }
    boost
}

#[allow(clippy::cast_precision_loss)]
fn compute_scores<'a>(
    node: &'a crate::graph::Node,
    graph: &'a crate::graph::Graph,
    ctx: &SuggestionContext,
    exclude_orphans: bool,
) -> Vec<ScoredItem<'a>> {
    let mut scored: Vec<ScoredItem<'a>> = Vec::new();

    for other in graph.nodes.values() {
        if other.uuid == node.uuid {
            continue;
        }
        if exclude_orphans && is_orphan(other, graph) {
            continue;
        }

        let mut score = 0.0;
        let mut reasons = Vec::new();
        let mut factor_scores: HashMap<String, f64> = HashMap::new();

        let title_s = score_title_overlap(other, ctx, &mut reasons);
        if title_s > 0.0 {
            score += title_s;
            factor_scores.insert("title".to_string(), title_s);
        }

        let content_s = score_content_match(other, graph, ctx, title_s > 0.0, &mut reasons);
        if content_s > 0.0 {
            score += content_s;
            *factor_scores.entry("content".to_string()).or_insert(0.0) += content_s;
        }

        let tag_s = score_tag_overlap(other, ctx, &mut reasons);
        if tag_s > 0.0 {
            score += tag_s;
            factor_scores.insert("tags".to_string(), tag_s);
        }

        let backlink_s = score_backlink_overlap(other, graph, ctx, &mut reasons);
        if backlink_s > 0.0 {
            score += backlink_s;
            *factor_scores.entry("backlinks".to_string()).or_insert(0.0) += backlink_s;
        }

        let outgoing_s = score_outgoing_overlap(other, ctx, &mut reasons);
        if outgoing_s > 0.0 {
            score += outgoing_s;
            *factor_scores.entry("outgoing".to_string()).or_insert(0.0) += outgoing_s;
        }

        let dir_s = score_directory_proximity(node, other);
        if dir_s > 0.0 {
            score += dir_s;
            *factor_scores.entry("directory".to_string()).or_insert(0.0) += dir_s;
        }

        let neighborhood_boost = score_neighborhood(other, graph, node, ctx, &mut reasons);
        if neighborhood_boost != 0.0 {
            factor_scores.insert("neighborhood".to_string(), neighborhood_boost);
            score *= 1.0 + neighborhood_boost;
        }

        if score > 0.0 {
            scored.push((other, score, reasons, factor_scores));
        }
    }

    scored
}

fn compute_suggestions_for_node(
    graph: &Graph,
    target: &str,
    exclude_orphans: bool,
    limit: Option<usize>,
    target_uuid: Option<String>,
) -> Result<SuggestResult> {
    let node = graph
        .nodes
        .get(target)
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("Note not found: {target}"))?;

    let heading_context: Option<String> = if graph.heading_uuid_to_primary.contains_key(target) {
        node_content(graph, &node).and_then(|content| find_heading_title_for_uuid(content, target))
    } else {
        None
    };

    let target_lower = if let Some(ref h) = heading_context {
        format!("{} {}", node.title, h)
    } else {
        node.title.to_lowercase()
    };

    let mut target_keywords: HashSet<String> = target_lower
        .split_whitespace()
        .filter(|w| w.len() > 2)
        .map(std::string::ToString::to_string)
        .collect();

    let mut content_keywords: HashSet<String> = HashSet::new();
    if let Some(content) = node_content(graph, &node) {
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
                if content_keywords.len() >= MAX_CONTENT_KEYWORDS {
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

    if let Some(ref h) = heading_context {
        target_keywords.extend(
            h.split_whitespace()
                .filter(|w| w.len() > 2)
                .map(std::string::ToString::to_string),
        );
    }

    let suggest_ctx = SuggestionContext {
        target_keywords: &target_keywords,
        content_keywords: &content_keywords,
        target_tags: &target_tags,
        target_backlinks: &target_backlinks,
        target_outgoing: &target_outgoing,
    };
    let mut scored = compute_scores(&node, graph, &suggest_ctx, exclude_orphans);
    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    let total = scored.len();
    let showed = limit.map(|l| {
        let shown = scored.len().min(l);
        scored.truncate(l);
        shown
    });

    let suggestions: Vec<Suggestion> = scored
        .iter()
        .map(|(n, s, r, fs)| Suggestion {
            uuid: n.uuid.clone(),
            title: n.title.clone(),
            path: n.path.display().to_string(),
            score: *s,
            reasons: r.clone(),
            filetags: n.filetags.clone(),
            scores: fs.clone(),
            target_uuid: target_uuid.clone(),
            heading_context: heading_context.clone(),
        })
        .collect();

    Ok((node, suggestions, total, showed, heading_context))
}

fn node_content<'a>(graph: &'a Graph, node: &Node) -> Option<&'a str> {
    graph.raw_content_for_path(&node.path)
}

pub struct SuggestOptions {
    pub targets: Vec<String>,
    pub limit: Option<usize>,
    pub exclude_orphans: bool,
}

pub fn run(ctx: &CommandContext<'_>, opts: &SuggestOptions) -> Result<()> {
    let outputs = execute(ctx.config(), opts)?;
    render(ctx.output(), &outputs)
}

pub fn execute(config: &ResolvedConfig, opts: &SuggestOptions) -> Result<Vec<SuggestOutput>> {
    let graph = Graph::load(config)?;

    opts.targets
        .iter()
        .map(|target| {
            let (node, suggestions, total, showed, heading_ctx) = compute_suggestions_for_node(
                &graph,
                target,
                opts.exclude_orphans,
                opts.limit,
                None,
            )?;
            Ok(SuggestOutput {
                target: node.title.clone(),
                target_uuid: node.uuid.clone(),
                total,
                showed,
                suggestions,
                target_heading_context: heading_ctx,
                ndjson_target_uuid: Some(target.clone()),
                score_precision: 1,
            })
        })
        .collect()
}

pub fn render(ctx: &OutputContext, outputs: &[SuggestOutput]) -> Result<()> {
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
        OutputFormat::Ndjson => {
            for output in outputs {
                for suggestion in ndjson_suggestions(output) {
                    println!("{}", serde_json::to_string(&suggestion)?);
                }
            }
        }
    }

    Ok(())
}

pub fn render_text(outputs: &[SuggestOutput]) -> String {
    outputs
        .iter()
        .map(render_one_text)
        .collect::<Vec<_>>()
        .join("\n")
}

fn render_one_text(output: &SuggestOutput) -> String {
    let mut text = String::new();

    if let Some(ref heading_context) = output.target_heading_context {
        let _ = writeln!(
            text,
            "Suggestions for \"{}\" ({})",
            output.target, heading_context
        );
    } else {
        let _ = writeln!(text, "Suggestions for \"{}\":", output.target);
    }
    text.push('\n');
    for (i, suggestion) in output.suggestions.iter().enumerate() {
        let score = format!(
            "{:.precision$}",
            suggestion.score,
            precision = output.score_precision
        );
        let _ = writeln!(text, "{:3}. {}  (score: {score})", i + 1, suggestion.title);
        let _ = writeln!(text, "       UUID: {}", suggestion.uuid);
        if !suggestion.scores.is_empty() && output.score_precision == 1 {
            let mut factors: Vec<&str> = suggestion.scores.keys().map(String::as_str).collect();
            factors.sort();
            let _ = writeln!(text, "       Matches: {}", factors.join(", "));
        }
    }

    text
}

fn ndjson_suggestions(output: &SuggestOutput) -> Vec<Suggestion> {
    output
        .suggestions
        .iter()
        .map(|suggestion| {
            let mut suggestion = suggestion.clone();
            if suggestion.target_uuid.is_none() {
                suggestion.target_uuid = output.ndjson_target_uuid.clone();
            }
            suggestion
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn suggestion(title: &str, score: f64) -> Suggestion {
        Suggestion {
            uuid: "11111111-1111-4111-8111-111111111111".to_string(),
            title: title.to_string(),
            path: "/notes/a.org".to_string(),
            score,
            reasons: vec!["shared tags".to_string()],
            filetags: vec!["tag".to_string()],
            scores: HashMap::from([("tags".to_string(), 25.0)]),
            target_uuid: None,
            heading_context: None,
        }
    }

    fn output() -> SuggestOutput {
        SuggestOutput {
            target: "Target".to_string(),
            target_uuid: "22222222-2222-4222-8222-222222222222".to_string(),
            total: 1,
            showed: None,
            suggestions: vec![suggestion("Suggestion", 25.0)],
            target_heading_context: None,
            ndjson_target_uuid: Some("input-target".to_string()),
            score_precision: 1,
        }
    }

    #[test]
    fn renders_suggest_text_from_typed_output() {
        let text = render_text(&[output()]);

        assert!(text.contains("Suggestions for \"Target\":"));
        assert!(text.contains("  1. Suggestion  (score: 25.0)"));
        assert!(text.contains("       UUID: 11111111-1111-4111-8111-111111111111"));
        assert!(text.contains("       Matches: tags"));
    }

    #[test]
    fn renders_heading_context_in_suggest_header() {
        let mut output = output();
        output.target_heading_context = Some("Heading".to_string());

        let text = render_text(&[output]);

        assert!(text.contains("Suggestions for \"Target\" (Heading)"));
    }

    #[test]
    fn fills_ndjson_target_uuid_without_changing_typed_suggestion() {
        let output = output();

        let suggestions = ndjson_suggestions(&output);

        assert_eq!(suggestions[0].target_uuid.as_deref(), Some("input-target"));
        assert!(output.suggestions[0].target_uuid.is_none());
    }
}
