use crate::config::Config;
use crate::graph::Graph;
use anyhow::Result;
use serde::Serialize;

#[derive(Serialize)]
pub struct BrokenOutput {
    pub count: usize,
    pub links: Vec<BrokenEntry>,
}

#[derive(Serialize)]
pub struct BrokenEntry {
    pub source_uuid: String,
    pub source_title: String,
    pub target_uuid: String,
}

pub fn run(
    config: &Config,
    json: bool,
    ndjson: bool,
    no_header: bool,
    count_only: bool,
    db_cli: Option<&std::path::Path>,
) -> Result<()> {
    let graph = Graph::load(config, db_cli, false)?;
    let links = graph.broken_links_list();

    let count = links.len();
    if count_only {
        if json {
            println!("{}", serde_json::json!({"count": count}));
        } else {
            println!("{}", count);
        }
        return Ok(());
    }

    if json {
        let entries: Vec<BrokenEntry> = links
            .into_iter()
            .map(|(src, title, tgt)| BrokenEntry {
                source_uuid: src,
                source_title: title,
                target_uuid: tgt,
            })
            .collect();
        if ndjson {
            for e in &entries {
                println!("{}", serde_json::to_string(e)?);
            }
        } else {
            let output = BrokenOutput {
                count: entries.len(),
                links: entries,
            };
            println!("{}", serde_json::to_string_pretty(&output)?);
        }
    } else {
        if !no_header {
            println!("Broken links ({}):", count);
        }
        for (_src, title, tgt) in &links {
            println!("  {} -> {}", title, tgt);
        }
    }

    Ok(())
}
