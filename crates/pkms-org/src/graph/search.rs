use std::collections::{HashMap, HashSet};

use super::Graph;

const TITLE_MATCH_WEIGHT: f64 = 10.0;
const REF_MATCH_WEIGHT: f64 = 6.0;
const TAG_MATCH_WEIGHT: f64 = 5.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SearchField {
    Title,
    Alias,
    Ref,
    Tag,
    Category,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SearchFields {
    selected: [bool; SearchField::COUNT],
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
        Self::new([
            SearchField::Title,
            SearchField::Alias,
            SearchField::Ref,
            SearchField::Tag,
        ])
    }
}

impl SearchField {
    const ALL: [Self; Self::COUNT] = [
        Self::Title,
        Self::Alias,
        Self::Ref,
        Self::Tag,
        Self::Category,
    ];
    const COUNT: usize = 5;

    fn index(self) -> usize {
        match self {
            Self::Title => 0,
            Self::Alias => 1,
            Self::Ref => 2,
            Self::Tag => 3,
            Self::Category => 4,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Title => "title",
            Self::Alias => "alias",
            Self::Ref => "ref",
            Self::Tag => "tag",
            Self::Category => "category",
        }
    }
}

impl SearchFields {
    pub fn new(fields: impl IntoIterator<Item = SearchField>) -> Self {
        let mut selected = [false; SearchField::COUNT];
        for field in fields {
            selected[field.index()] = true;
        }
        Self { selected }
    }

    fn includes(self, field: SearchField) -> bool {
        self.selected[field.index()]
    }

    fn iter(&self) -> impl Iterator<Item = SearchField> + '_ {
        SearchField::ALL
            .into_iter()
            .filter(|field| self.includes(*field))
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct SearchMatch {
    field: SearchField,
    score: f64,
}

impl Graph {
    pub fn search(&self, terms: &str, fields: &SearchFields) -> Vec<SearchResult<'_>> {
        let query = terms.to_lowercase();
        let words: Vec<&str> = query.split_whitespace().collect();
        let mut results = Vec::new();

        for node in self.nodes.values() {
            let matches: Vec<SearchMatch> = fields
                .iter()
                .flat_map(|field| search_matches_for_field(node, field, &words))
                .collect();
            let score = matches.iter().map(|match_| match_.score).sum();
            if score == 0.0 {
                continue;
            }

            let mut sources = HashSet::new();
            for match_ in matches {
                sources.insert(match_.field.label());
            }

            let mut matches: Vec<String> = sources.into_iter().map(String::from).collect();
            matches.sort();
            results.push(SearchResult {
                node,
                score,
                matches,
            });
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

fn search_matches_for_field(
    node: &super::Node,
    field: SearchField,
    words: &[&str],
) -> Vec<SearchMatch> {
    match field {
        SearchField::Title => score_case_insensitive_word_matches(
            SearchField::Title,
            &node.title,
            words,
            TITLE_MATCH_WEIGHT,
        )
        .into_iter()
        .collect(),
        SearchField::Alias => node
            .aliases
            .iter()
            .filter_map(|alias| {
                score_case_insensitive_word_matches(
                    SearchField::Alias,
                    alias,
                    words,
                    TITLE_MATCH_WEIGHT,
                )
            })
            .collect(),
        SearchField::Ref => node
            .refs
            .iter()
            .filter_map(|ref_| {
                score_case_insensitive_any_match(SearchField::Ref, ref_, words, REF_MATCH_WEIGHT)
            })
            .collect(),
        SearchField::Tag => node
            .filetags
            .iter()
            .filter_map(|tag| score_case_sensitive_any_match(SearchField::Tag, tag, words))
            .collect(),
        SearchField::Category => node
            .categories
            .iter()
            .filter_map(|category| {
                score_case_sensitive_any_match(SearchField::Category, category, words)
            })
            .collect(),
    }
}

#[allow(clippy::cast_precision_loss)]
fn score_case_insensitive_word_matches(
    field: SearchField,
    value: &str,
    words: &[&str],
    weight: f64,
) -> Option<SearchMatch> {
    let value = value.to_lowercase();
    let matched_words = words.iter().filter(|word| value.contains(*word)).count();
    (matched_words > 0).then_some(SearchMatch {
        field,
        score: matched_words as f64 * weight,
    })
}

fn score_case_insensitive_any_match(
    field: SearchField,
    value: &str,
    words: &[&str],
    weight: f64,
) -> Option<SearchMatch> {
    let value = value.to_lowercase();
    words
        .iter()
        .any(|word| value.contains(*word))
        .then_some(SearchMatch {
            field,
            score: weight,
        })
}

fn score_case_sensitive_any_match(
    field: SearchField,
    value: &str,
    words: &[&str],
) -> Option<SearchMatch> {
    words
        .iter()
        .any(|word| value.contains(*word))
        .then_some(SearchMatch {
            field,
            score: TAG_MATCH_WEIGHT,
        })
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
