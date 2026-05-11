use crate::cli::OutputFormat;
use crate::config::Config;
#[cfg(feature = "embed")]
use crate::embed;
use crate::graph::{Graph, Node};
use crate::output::OutputContext;
use crate::parser::{HEADING_RE, Link};
use crate::util;
use anyhow::Result;
use serde::Serialize;
use std::collections::{HashMap, HashSet};

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
        score += title_overlap as f64 * 20.0;
    }

    if !ctx.content_keywords.is_empty()
        && let Ok(nc) = std::fs::read_to_string(&neighbor.path)
    {
        let ncl = nc.to_lowercase();
        let cm = ctx
            .content_keywords
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
        .filter(|t| ctx.target_tags.contains(t.as_str()))
        .count();
    if tag_overlap > 0 {
        score += tag_overlap as f64 * 25.0;
    }

    let cat_overlap = neighbor
        .categories
        .iter()
        .filter(|c| ctx.target_tags.contains(c.as_str()))
        .count();
    if cat_overlap > 0 {
        score += cat_overlap as f64 * 25.0;
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
        .filter(|u| ctx.target_outgoing.contains(u))
        .count();
    if shared_outgoing > 0 {
        score += shared_outgoing as f64 * 12.0;
    }

    score
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
            .filter(|w| ctx.target_keywords.iter().any(|kw| kw.as_str() == **w))
            .count();
        if title_overlap > 0 {
            let s = title_overlap as f64 * 20.0;
            score += s;
            factor_scores.insert("title".to_string(), s);
            reasons.push(format!("shared title: \"{}\"", other.title));
        }

        if !ctx.content_keywords.is_empty()
            && let Ok(other_content) = std::fs::read_to_string(&other.path)
        {
            let other_lc = other_content.to_lowercase();
            let content_match_count: usize = ctx
                .content_keywords
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
            .filter(|t| ctx.target_tags.contains(t.as_str()))
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
            .filter(|c| ctx.target_tags.contains(c.as_str()))
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
        let shared_backlinks: usize = ctx.target_backlinks.intersection(&other_backlinks).count();
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
        let shared_outgoing: usize = ctx.target_outgoing.intersection(&other_outgoing).count();
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
        if let Ok(content) = std::fs::read_to_string(&node.path) {
            find_heading_title_for_uuid(&content, target)
        } else {
            None
        }
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
            path: n.path.to_string_lossy().to_string(),
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

#[allow(clippy::too_many_arguments)]
pub fn run(
    config: &Config,
    ctx: &OutputContext,
    target: Option<&str>,
    limit: Option<usize>,
    exclude_orphans: bool,
    from_stdin: bool,
    use_embed: bool,
) -> Result<()> {
    let targets: Vec<String> = if from_stdin || (target.is_none() && util::is_stdin_piped()) {
        util::read_stdin_ndjson()?
    } else if let Some(t) = target {
        vec![t.to_string()]
    } else {
        anyhow::bail!(
            "No target specified and no stdin pipe detected. Provide a target or use --from-stdin."
        );
    };

    let graph = Graph::load(config)?;

    if use_embed {
        #[cfg(feature = "embed")]
        {
            return suggest_by_embedding(&graph, ctx, &targets, limit);
        }
        #[cfg(not(feature = "embed"))]
        {
            anyhow::bail!("--embed requires building with the 'embed' feature enabled");
        }
    }

    match ctx.format {
        OutputFormat::Text => {
            for t in &targets {
                let (node, suggestions, _total, _showed, heading_ctx) =
                    compute_suggestions_for_node(&graph, t, exclude_orphans, limit, None)?;
                if let Some(ref h) = heading_ctx {
                    println!("Suggestions for \"{}\" ({})", node.title, h);
                } else {
                    println!("Suggestions for \"{}\":", node.title);
                }
                println!();
                for (i, s) in suggestions.iter().enumerate() {
                    println!("{:3}. {}  (score: {:.1})", i + 1, s.title, s.score);
                    println!("       UUID: {}", s.uuid);
                    if !s.scores.is_empty() {
                        let mut factors: Vec<&str> = s.scores.keys().map(String::as_str).collect();
                        factors.sort();
                        println!("       Matches: {}", factors.join(", "));
                    }
                }
                if targets.len() > 1 {
                    println!();
                }
            }
        }
        OutputFormat::Json => {
            let mut all_outputs = Vec::new();
            for t in &targets {
                let (node, suggestions, total, showed, _heading_ctx) =
                    compute_suggestions_for_node(&graph, t, exclude_orphans, limit, None)?;
                all_outputs.push(SuggestOutput {
                    target: node.title.clone(),
                    target_uuid: node.uuid.clone(),
                    total,
                    showed,
                    suggestions,
                });
            }
            if all_outputs.len() == 1 {
                ctx.print_json(&all_outputs[0])?;
            } else {
                ctx.print_json(&all_outputs)?;
            }
        }
        OutputFormat::Ndjson => {
            for t in &targets {
                let (_node, suggestions, _total, _showed, _heading_ctx) =
                    compute_suggestions_for_node(
                        &graph,
                        t,
                        exclude_orphans,
                        limit,
                        Some(t.clone()),
                    )?;
                for s in &suggestions {
                    println!("{}", serde_json::to_string(s)?);
                }
            }
        }
    }

    Ok(())
}

#[cfg(feature = "embed")]
fn suggest_by_embedding(
    graph: &Graph,
    ctx: &OutputContext,
    targets: &[String],
    limit: Option<usize>,
) -> Result<()> {
    let mut texts = Vec::new();
    let mut node_list: Vec<&Node> = graph.nodes.values().collect();
    node_list.sort_by(|a, b| a.uuid.cmp(&b.uuid));

    for node in &node_list {
        texts.push(if node.title.is_empty() {
            node.path.display().to_string()
        } else {
            node.title.clone()
        });
    }

    if texts.is_empty() {
        return Ok(());
    }

    let embeddings = embed::compute_embeddings(&texts)?;

    let all_outputs: Vec<SuggestOutput> = targets
        .iter()
        .filter_map(|t| {
            let target_node = graph.resolve_target(t).ok()?;
            let target_idx = node_list.iter().position(|n| n.uuid == target_node.uuid)?;
            let target_emb = &embeddings[target_idx];

            let mut scored: Vec<(usize, f64)> = node_list
                .iter()
                .enumerate()
                .map(|(i, _)| (i, embed::cosine_similarity(target_emb, &embeddings[i])))
                .collect();
            scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

            let suggestions: Vec<Suggestion> = scored
                .iter()
                .filter(|(i, _)| *i != target_idx)
                .take(limit.unwrap_or(usize::MAX))
                .map(|(i, score)| {
                    let n = node_list[*i];
                    let mut m = HashMap::new();
                    m.insert("semantic".to_string(), *score);
                    Suggestion {
                        uuid: n.uuid.clone(),
                        title: n.title.clone(),
                        path: n.path.to_string_lossy().to_string(),
                        score: *score,
                        reasons: vec!["semantic similarity".to_string()],
                        filetags: n.filetags.clone(),
                        scores: m,
                        target_uuid: Some(target_node.uuid.clone()),
                        heading_context: None,
                    }
                })
                .collect();

            Some(SuggestOutput {
                target: target_node.title.clone(),
                target_uuid: target_node.uuid.clone(),
                total: suggestions.len(),
                showed: limit.map(|l| suggestions.len().min(l)),
                suggestions,
            })
        })
        .collect();

    match ctx.format {
        OutputFormat::Text => {
            for output in &all_outputs {
                println!("Suggestions for \"{}\":", output.target);
                println!();
                for (i, s) in output.suggestions.iter().enumerate() {
                    println!("{:3}. {}  (score: {:.3})", i + 1, s.title, s.score);
                    println!("       UUID: {}", s.uuid);
                }
                if all_outputs.len() > 1 {
                    println!();
                }
            }
        }
        OutputFormat::Json => {
            if all_outputs.len() == 1 {
                ctx.print_json(&all_outputs[0])?;
            } else {
                ctx.print_json(&all_outputs)?;
            }
        }
        OutputFormat::Ndjson => {
            for output in &all_outputs {
                for s in &output.suggestions {
                    println!("{}", serde_json::to_string(s)?);
                }
            }
        }
    }

    Ok(())
}
