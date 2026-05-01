use std::collections::HashMap;

use super::Graph;

impl Graph {
    pub fn search(&self, terms: &str) -> Vec<(&super::Node, f64, Vec<String>)> {
        let query = terms.to_lowercase();
        let words: Vec<&str> = query.split_whitespace().collect();
        let mut results: Vec<(&super::Node, f64, Vec<String>)> = Vec::new();

        for node in self.nodes.values() {
            let mut score = 0.0;
            let mut matches = Vec::new();

            let title_lower = node.title.to_lowercase();
            let words_in_title: usize = words.iter().filter(|w| title_lower.contains(*w)).count();
            if words_in_title > 0 {
                score += words_in_title as f64 * 10.0;
                matches.push(format!("title: {}", node.title));
            }

            for alias in &node.aliases {
                let alias_lower = alias.to_lowercase();
                let words_in_alias: usize =
                    words.iter().filter(|w| alias_lower.contains(*w)).count();
                if words_in_alias > 0 {
                    score += words_in_alias as f64 * 8.0;
                    matches.push(format!("alias: {alias}"));
                }
            }

            for ref_ in &node.refs {
                let ref_lower = ref_.to_lowercase();
                if words.iter().any(|w| ref_lower.contains(*w)) {
                    score += 6.0;
                    matches.push(format!("ref: {ref_}"));
                }
            }

            for tag in &node.filetags {
                if words.iter().any(|w| tag.contains(*w)) {
                    score += 5.0;
                    matches.push(format!("tag: {tag}"));
                }
            }

            if score > 0.0 {
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

    pub fn notes_by_tag(&self, tag: &str) -> Vec<&super::Node> {
        self.nodes
            .values()
            .filter(|n| n.filetags.iter().any(|t| t == tag))
            .collect()
    }
}
