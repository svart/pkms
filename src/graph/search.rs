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

pub struct ContentSearchResult<'a> {
    pub node: &'a super::Node,
    pub lines: Vec<String>,
}

pub struct SearchResult<'a> {
    pub node: &'a super::Node,
    pub score: f64,
    pub matches: Vec<String>,
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
    pub fn search(&self, terms: &str, fields: &SearchFields) -> Vec<SearchResult<'_>> {
        let query = terms.to_lowercase();
        let words: Vec<&str> = query.split_whitespace().collect();
        let mut results = Vec::new();

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
                results.push(SearchResult {
                    node,
                    score,
                    matches,
                });
            }
        }

        results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        results
    }

    pub fn search_content(&self, terms: &str) -> Vec<ContentSearchResult<'_>> {
        let query = terms.to_lowercase();
        let mut results = Vec::new();

        if self.results.is_empty() {
            for uuid in self.path_to_uuid.values() {
                let Some(node) = self.nodes.get(uuid) else {
                    continue;
                };
                let Some(content) = std::fs::read_to_string(&node.path).ok() else {
                    continue;
                };
                let Some(lines) = content_match_lines(&content, &query) else {
                    continue;
                };
                results.push(ContentSearchResult { node, lines });
            }
            return results;
        }

        for scan_result in &self.results {
            let Some(uuid) = self.path_to_uuid.get(&scan_result.path) else {
                continue;
            };
            let Some(node) = self.nodes.get(uuid) else {
                continue;
            };
            let lines = if let Some(content) = scan_result.raw_content.as_deref() {
                content_match_lines(content, &query)
            } else {
                std::fs::read_to_string(&scan_result.path)
                    .ok()
                    .and_then(|content| content_match_lines(&content, &query))
            };
            let Some(lines) = lines else { continue };
            results.push(ContentSearchResult { node, lines });
        }

        results
    }

    pub fn all_tags(&self) -> Vec<(String, usize)> {
        let mut tag_counts: HashMap<String, usize> = HashMap::new();
        for uuid in self.path_to_uuid.values() {
            if let Some(node) = self.nodes.get(uuid) {
                let mut seen_tags = HashSet::new();
                for tag in &node.filetags {
                    if seen_tags.insert(tag.as_str()) {
                        *tag_counts.entry(tag.clone()).or_default() += 1;
                    }
                }
            }
        }
        let mut tags: Vec<(String, usize)> = tag_counts.into_iter().collect();
        tags.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        tags
    }
}

fn content_match_lines(content: &str, query: &str) -> Option<Vec<String>> {
    let mut lines = Vec::new();
    for (i, line) in content.lines().enumerate() {
        if line.to_lowercase().contains(query) {
            lines.push(format!("{}: {}", i + 1, line.trim()));
        }
    }
    (!lines.is_empty()).then_some(lines)
}
