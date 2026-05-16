use std::collections::{HashMap, HashSet};

use super::Graph;

const TITLE_MATCH_WEIGHT: f64 = 10.0;
const REF_MATCH_WEIGHT: f64 = 6.0;
const TAG_MATCH_WEIGHT: f64 = 5.0;

pub struct SearchFields {
    pub title: bool,
    pub alias: bool,
    pub ref_: bool,
    pub tag: bool,
    pub category: bool,
}

impl Default for SearchFields {
    fn default() -> Self {
        Self {
            title: true,
            alias: true,
            ref_: true,
            tag: true,
            category: false,
        }
    }
}

impl Graph {
    #[allow(clippy::cast_precision_loss)]
    pub fn search(
        &self,
        terms: &str,
        fields: &SearchFields,
    ) -> Vec<(&super::Node, f64, Vec<String>)> {
        let query = terms.to_lowercase();
        let words: Vec<&str> = query.split_whitespace().collect();
        let mut results: Vec<(&super::Node, f64, Vec<String>)> = Vec::new();

        for node in self.nodes.values() {
            let mut score = 0.0;
            let mut sources = HashSet::new();

            if fields.title {
                let title_lower = node.title.to_lowercase();
                let words_in_title: usize =
                    words.iter().filter(|w| title_lower.contains(*w)).count();
                if words_in_title > 0 {
                    score += words_in_title as f64 * TITLE_MATCH_WEIGHT;
                    sources.insert("title");
                }
            }

            if fields.alias {
                for alias in &node.aliases {
                    let alias_lower = alias.to_lowercase();
                    let words_in_alias: usize =
                        words.iter().filter(|w| alias_lower.contains(*w)).count();
                    if words_in_alias > 0 {
                        score += words_in_alias as f64 * TITLE_MATCH_WEIGHT;
                        sources.insert("alias");
                    }
                }
            }

            if fields.ref_ {
                for ref_ in &node.refs {
                    let ref_lower = ref_.to_lowercase();
                    if words.iter().any(|w| ref_lower.contains(*w)) {
                        score += REF_MATCH_WEIGHT;
                        sources.insert("ref");
                    }
                }
            }

            if fields.tag {
                for tag in &node.filetags {
                    if words.iter().any(|w| tag.contains(*w)) {
                        score += TAG_MATCH_WEIGHT;
                        sources.insert("tag");
                    }
                }
            }

            if fields.category {
                for cat in &node.categories {
                    if words.iter().any(|w| cat.contains(*w)) {
                        score += TAG_MATCH_WEIGHT;
                        sources.insert("category");
                    }
                }
            }

            if score > 0.0 {
                let mut matches: Vec<String> = sources.into_iter().map(String::from).collect();
                matches.sort();
                results.push((node, score, matches));
            }
        }

        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        results
    }

    pub fn search_content(&self, terms: &str) -> Vec<(String, String, Vec<String>)> {
        let query = terms.to_lowercase();
        let mut results = Vec::new();
        let mut seen_paths = HashSet::new();

        for node in self.nodes.values() {
            if !seen_paths.insert(node.path.clone()) {
                continue;
            }
            let content = self
                .results
                .iter()
                .find(|r| r.path == node.path)
                .and_then(|r| r.raw_content.clone())
                .or_else(|| std::fs::read_to_string(&node.path).ok());
            let Some(content) = content else {
                continue;
            };
            if !content.to_lowercase().contains(&query) {
                continue;
            }
            let mut context_lines = Vec::new();
            for (i, line) in content.lines().enumerate() {
                if line.to_lowercase().contains(&query) {
                    context_lines.push(format!("{}: {}", i + 1, line.trim()));
                }
            }
            results.push((
                node.uuid.clone(),
                node.title.clone(),
                context_lines,
            ));
        }

        results
    }

    pub fn all_tags(&self) -> Vec<(String, usize)> {
        let mut tag_counts: HashMap<String, usize> = HashMap::new();
        for node in self.nodes.values() {
            for tag in &node.filetags {
                *tag_counts.entry(tag.clone()).or_default() += 1;
            }
        }
        let mut tags: Vec<(String, usize)> = tag_counts.into_iter().collect();
        tags.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        tags
    }
}
