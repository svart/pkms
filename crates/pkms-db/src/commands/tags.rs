use std::collections::{HashMap, HashSet};

use anyhow::Result;
use pkms_org::{Graph, OrgConfig};
use serde::Serialize;

#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct TagEntry {
    pub tag: String,
    pub count: usize,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct TagsOutput {
    pub tags: Vec<TagEntry>,
}

pub fn execute(config: &OrgConfig) -> Result<TagsOutput> {
    let graph = crate::load_graph(config)?;
    Ok(build_output(&graph))
}

fn build_output(graph: &Graph) -> TagsOutput {
    let mut counts = HashMap::<String, usize>::new();

    for file in graph.files() {
        count_assignment(&file.parsed.filetags, &mut counts);
        for heading in &file.parsed.headings {
            count_assignment(&heading.tags, &mut counts);
        }
    }

    let mut tags = counts
        .into_iter()
        .map(|(tag, count)| TagEntry { tag, count })
        .collect::<Vec<_>>();
    tags.sort_by(|left, right| {
        right
            .count
            .cmp(&left.count)
            .then_with(|| left.tag.cmp(&right.tag))
    });
    TagsOutput { tags }
}

fn count_assignment(tags: &[String], counts: &mut HashMap<String, usize>) {
    let mut seen = HashSet::new();
    for tag in tags {
        if seen.insert(tag.as_str()) {
            *counts.entry(tag.clone()).or_default() += 1;
        }
    }
}

pub fn render_text(output: &TagsOutput) -> String {
    let width = output
        .tags
        .iter()
        .map(|entry| entry.tag.chars().count())
        .max()
        .unwrap_or(3)
        .max(3);
    let mut lines = vec![format!("{:<width$}  COUNT", "TAG")];
    lines.extend(
        output
            .tags
            .iter()
            .map(|entry| format!("{:<width$}  {}", entry.tag, entry.count)),
    );
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn counts_direct_note_and_heading_assignments_and_sorts_by_usage() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("one.org"),
            ":PROPERTIES:\n:ID: 11111111-1111-1111-1111-111111111111\n:END:\n#+title: One\n#+filetags: :alpha:common:alpha:\n* Heading :common:common:\n",
        )
        .unwrap();
        fs::write(
            dir.path().join("two.org"),
            ":PROPERTIES:\n:ID: 22222222-2222-2222-2222-222222222222\n:END:\n#+title: Two\n#+filetags: :alpha:\n* TODO Task :beta:common:delta:\n",
        )
        .unwrap();
        let config = OrgConfig {
            db_root: dir.path().to_path_buf(),
            ignore_patterns: Vec::new(),
            home_dir: None,
        };

        let output = execute(&config).unwrap();

        assert_eq!(
            output.tags,
            vec![
                TagEntry {
                    tag: "common".to_string(),
                    count: 3,
                },
                TagEntry {
                    tag: "alpha".to_string(),
                    count: 2,
                },
                TagEntry {
                    tag: "beta".to_string(),
                    count: 1,
                },
                TagEntry {
                    tag: "delta".to_string(),
                    count: 1,
                },
            ]
        );
    }

    #[test]
    fn text_always_displays_the_usage_count() {
        let output = TagsOutput {
            tags: vec![TagEntry {
                tag: "rust".to_string(),
                count: 12,
            }],
        };

        assert_eq!(render_text(&output), "TAG   COUNT\nrust  12");
    }
}
