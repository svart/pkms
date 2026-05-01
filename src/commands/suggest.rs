use crate::config::Config;
use crate::graph::Graph;
use crate::parser::Link;
use crate::util;
use anyhow::Result;
use serde::Serialize;
use std::collections::{HashMap, HashSet};

#[derive(Serialize)]
pub struct SuggestOutput {
    pub target: String,
    pub target_uuid: String,
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

pub fn run(
    config: &Config,
    json: bool,
    verbose: bool,
    target: Option<&str>,
    limit: Option<usize>,
    input_json: Option<&std::path::PathBuf>,
    db_cli: Option<&std::path::Path>,
) -> Result<()> {
    let target = util::load_input_target(input_json, target, "target", "No target specified. Provide a target or use --input-json")?;

    let graph = Graph::load(config, db_cli, false)?;
    let limit = limit.unwrap_or(10);

    let node = graph.resolve_target(&target)?.clone();

    let target_lower = node.title.to_lowercase();

    // Extract meaningful keywords from target's title (words > 2 chars)
    let target_keywords: HashSet<String> = target_lower
        .split_whitespace()
        .filter(|w| w.len() > 2)
        .map(|w| w.to_string())
        .collect();

    // Extract keywords from target's full content
    let mut content_keywords: HashSet<String> = HashSet::new();
    if let Ok(content) = std::fs::read_to_string(&node.path) {
        let content_lower = content.to_lowercase();
        // Take first 100 unique meaningful words from content
        for word in content_lower.split_whitespace() {
            let clean: String = word.trim_matches(|c: char| !c.is_alphanumeric()).chars().filter(|c| c.is_alphanumeric()).collect();
            if clean.len() > 3 && !clean.starts_with("http") && !clean.starts_with("id") && !clean.starts_with("file") {
                content_keywords.insert(clean);
                if content_keywords.len() >= 50 {
                    break;
                }
            }
        }
    }

    let target_tags: HashSet<&str> = node.filetags.iter().map(|t| t.as_str()).collect();
    let target_backlinks: HashSet<&str> = graph
        .backlinks
        .get(&node.uuid)
        .map(|v| v.iter().map(|s| s.as_str()).collect())
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

    let mut scored: Vec<(&crate::graph::Node, f64, Vec<String>, HashMap<String, f64>)> = Vec::new();

    for other in graph.nodes.values() {
        if other.uuid == node.uuid {
            continue;
        }

        let mut score = 0.0;
        let mut reasons = Vec::new();
        let mut factor_scores: HashMap<String, f64> = HashMap::new();

        // 1. Title keyword match (strongest signal)
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

        // 2. Content keyword match (scan other note's content for target's keywords)
        if !content_keywords.is_empty() {
            if let Ok(other_content) = std::fs::read_to_string(&other.path) {
                let other_lc = other_content.to_lowercase();
                let content_match_count: usize = content_keywords.iter()
                    .filter(|kw| other_lc.contains(kw.as_str()))
                    .count();
                if content_match_count > 0 {
                    let s = content_match_count as f64 * 5.0;
                    score += s;
                    *factor_scores.entry("content".to_string()).or_insert(0.0) += s;
                    if title_overlap == 0 {
                        reasons.push(format!("{} content keyword matches", content_match_count));
                    }
                }
            }
        }

        // 3. Shared tags (strong signal)
        let tag_overlap: usize = other
            .filetags
            .iter()
            .filter(|t| target_tags.contains(t.as_str()))
            .count();
        if tag_overlap > 0 {
            let s = tag_overlap as f64 * 25.0;
            score += s;
            factor_scores.insert("tags".to_string(), s);
            reasons.push(format!("shared tags"));
        }

        // 4. Shared backlinks (co-citation)
        let other_backlinks: HashSet<&str> = graph
            .backlinks
            .get(&other.uuid)
            .map(|v| v.iter().map(|s| s.as_str()).collect())
            .unwrap_or_default();
        let shared_backlinks: usize = target_backlinks
            .intersection(&other_backlinks)
            .count();
        if shared_backlinks > 0 {
            let s = shared_backlinks as f64 * 15.0;
            score += s;
            *factor_scores.entry("backlinks".to_string()).or_insert(0.0) += s;
            reasons.push(format!("{} shared backlinks", shared_backlinks));
        }

        // 5. Shared outgoing links
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
        let shared_outgoing: usize = target_outgoing
            .intersection(&other_outgoing)
            .count();
        if shared_outgoing > 0 {
            let s = shared_outgoing as f64 * 12.0;
            score += s;
            *factor_scores.entry("outgoing".to_string()).or_insert(0.0) += s;
            reasons.push(format!("{} shared outgoing", shared_outgoing));
        }

        // 6. Directory proximity (same parent dir)
        if let (Some(tp), Some(op)) = (node.path.parent(), other.path.parent()) {
            if tp == op && score > 0.0 {
                let s = 5.0;
                score += s;
                *factor_scores.entry("directory".to_string()).or_insert(0.0) += s;
            }
        }

        if score > 0.0 {
            scored.push((other, score, reasons, factor_scores));
        }
    }

    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    scored.truncate(limit);

    if json {
        let suggestions: Vec<Suggestion> = scored
            .into_iter()
            .map(|(n, s, r, fs)| Suggestion {
                uuid: n.uuid.clone(),
                title: n.title.clone(),
                path: n.path.to_string_lossy().to_string(),
                score: s,
                reasons: r,
                filetags: n.filetags.clone(),
                scores: fs,
            })
            .collect();
        let output = SuggestOutput {
            target: node.title.clone(),
            target_uuid: node.uuid.clone(),
            suggestions,
        };
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        println!("Suggestions for \"{}\":", node.title);
        println!();
        for (i, (n, score, reasons, fs)) in scored.iter().enumerate() {
            println!(
                "{:3}. {:45} score: {:5.0}",
                i + 1,
                n.title,
                score
            );
            if !reasons.is_empty() {
                println!("       {}", reasons.join(", "));
            }
            if verbose && !fs.is_empty() {
                let mut factors: Vec<(&String, &f64)> = fs.iter().collect();
                factors.sort_by(|a, b| b.1.partial_cmp(a.1).unwrap_or(std::cmp::Ordering::Equal));
                let parts: Vec<String> = factors.iter().map(|(k, v)| format!("  {}: {:.0}", k, v)).collect();
                println!("       Factors:{}", parts.join(""));
            }
        }
    }

    Ok(())
}
