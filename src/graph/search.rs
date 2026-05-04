use std::collections::{HashMap, HashSet};

use super::Graph;

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
                    score += words_in_title as f64 * 10.0;
                    sources.insert("title");
                }
            }

            if fields.alias {
                for alias in &node.aliases {
                    let alias_lower = alias.to_lowercase();
                    let words_in_alias: usize =
                        words.iter().filter(|w| alias_lower.contains(*w)).count();
                    if words_in_alias > 0 {
                        score += words_in_alias as f64 * 10.0;
                        sources.insert("alias");
                    }
                }
            }

            if fields.ref_ {
                for ref_ in &node.refs {
                    let ref_lower = ref_.to_lowercase();
                    if words.iter().any(|w| ref_lower.contains(*w)) {
                        score += 6.0;
                        sources.insert("ref");
                    }
                }
            }

            if fields.tag {
                for tag in &node.filetags {
                    if words.iter().any(|w| tag.contains(*w)) {
                        score += 5.0;
                        sources.insert("tag");
                    }
                }
            }

            if fields.category {
                for cat in &node.categories {
                    if words.iter().any(|w| cat.contains(*w)) {
                        score += 5.0;
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

        for node in self.nodes.values() {
            if let Ok(content) = std::fs::read_to_string(&node.path) {
                let content_lower = content.to_lowercase();
                if content_lower.contains(&query) {
                    let mut context_lines = Vec::new();
                    for (i, line) in content.lines().enumerate() {
                        if line.to_lowercase().contains(&query) {
                            context_lines.push(format!("{}: {}", i + 1, line.trim()));
                        }
                    }
                    results.push((node.uuid.clone(), node.title.clone(), context_lines));
                }
            }
        }

        results
    }

    #[allow(dead_code)]
    pub fn all_categories(&self) -> Vec<(String, usize)> {
        let mut cat_counts: HashMap<String, usize> = HashMap::new();
        for node in self.nodes.values() {
            for cat in &node.categories {
                *cat_counts.entry(cat.clone()).or_default() += 1;
            }
        }
        let mut cats: Vec<(String, usize)> = cat_counts.into_iter().collect();
        cats.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        cats
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
