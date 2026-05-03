use crate::cli::OutputFormat;
use crate::config::Config;
use crate::graph::Graph;
use crate::output::OutputContext;
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
    ctx: &OutputContext,
    tag_filter: Option<&str>,
    db_cli: Option<&std::path::Path>,
) -> Result<()> {
    let graph = Graph::load(config, db_cli)?;

    if let Some(tag) = tag_filter {
        let notes = graph.notes_by_tag(tag);
        let notes_json: Vec<TagNote> = notes
            .iter()
            .map(|n| TagNote {
                uuid: n.uuid.clone(),
                title: n.title.clone(),
                path: util::path_string(&n.path),
            })
            .collect();

        match ctx.format {
            OutputFormat::Text => {
                println!("Tag: {tag}");
                println!("Notes: {}", notes.len());
                for n in &notes {
                    println!("  {} ({})", n.title, n.uuid);
                }
            }
            OutputFormat::Json => {
                ctx.print_json(&TagsOutput {
                    tags: vec![TagEntry {
                        tag: tag.to_string(),
                        count: notes.len(),
                        notes: notes_json,
                    }],
                })?;
            }
            OutputFormat::Ndjson => {
                ctx.print_ndjson(&notes_json)?;
            }
        }
    } else {
        let tags = graph.all_tags();
        let entries: Vec<TagEntry> = tags
            .iter()
            .map(|(tag, count)| TagEntry {
                tag: tag.clone(),
                count: *count,
                notes: vec![],
            })
            .collect();

        match ctx.format {
            OutputFormat::Text => {
                println!("Filetags (count):");
                for (tag, count) in &tags {
                    println!("  {tag:30} {count}");
                }
                println!();
                println!("Total unique tags: {}", tags.len());
            }
            OutputFormat::Json => {
                ctx.print_json(&TagsOutput { tags: entries })?;
            }
            OutputFormat::Ndjson => {
                ctx.print_ndjson(&entries)?;
            }
        }
    }

    Ok(())
}
