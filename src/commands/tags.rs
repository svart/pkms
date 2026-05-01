use crate::config::Config;
use crate::graph::Graph;
use crate::util;
use anyhow::Result;
use serde::Serialize;

#[derive(Serialize)]
pub struct TagsOutput {
    pub tags: Vec<TagEntry>,
}

#[derive(Serialize)]
pub struct TagEntry {
    pub tag: String,
    pub count: usize,
    pub notes: Vec<TagNote>,
}

#[derive(Serialize)]
pub struct TagNote {
    pub uuid: String,
    pub title: String,
    pub path: String,
}

pub fn run(
    config: &Config,
    json: bool,
    ndjson: bool,
    no_header: bool,
    count_only: bool,
    verbose: bool,
    tag_filter: Option<&str>,
    db_cli: Option<&std::path::Path>,
) -> Result<()> {
    let graph = Graph::load(config, db_cli, verbose)?;

    if let Some(tag) = tag_filter {
        let notes = graph.notes_by_tag(tag);
        if count_only {
            return util::print_count(notes.len(), json);
        }
        if json {
            let notes_json: Vec<TagNote> = notes
                .iter()
                .map(|n| TagNote {
                    uuid: n.uuid.clone(),
                    title: n.title.clone(),
                    path: util::path_string(&n.path),
                })
                .collect();
            if ndjson {
                return util::print_ndjson(&notes_json);
            } else {
                let output = TagsOutput {
                    tags: vec![TagEntry {
                        tag: tag.to_string(),
                        count: notes.len(),
                        notes: notes_json,
                    }],
                };
                println!("{}", serde_json::to_string_pretty(&output)?);
            }
        } else {
            if !no_header {
                println!("Tag: {}", tag);
                println!("Notes: {}", notes.len());
            }
            for n in &notes {
                println!("  {} ({})", n.title, n.uuid);
            }
        }
    } else {
        let tags = graph.all_tags();
        if count_only {
            return util::print_count(tags.len(), json);
        }
        if json {
            let entries: Vec<TagEntry> = tags
                .into_iter()
                .map(|(tag, count)| TagEntry {
                    tag,
                    count,
                    notes: vec![],
                })
                .collect();
            if ndjson {
                return util::print_ndjson(&entries);
            } else {
                let output = TagsOutput { tags: entries };
                println!("{}", serde_json::to_string_pretty(&output)?);
            }
        } else {
            if !no_header {
                println!("Filetags (count):");
            }
            for (tag, count) in &tags {
                println!("  {:30} {}", tag, count);
            }
            if !no_header {
                println!();
                println!("Total unique tags: {}", tags.len());
            }
        }
    }

    Ok(())
}
