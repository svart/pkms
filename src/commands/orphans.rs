use crate::config::Config;
use crate::graph::Graph;
use anyhow::Result;
use serde::Serialize;

#[derive(Serialize)]
pub struct OrphansOutput {
    pub count: usize,
    pub orphans: Vec<OrphanEntry>,
}

#[derive(Serialize)]
pub struct OrphanEntry {
    pub uuid: String,
    pub title: String,
    pub path: String,
    pub filetags: Vec<String>,
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
    let orphans = graph.orphan_nodes();

    let count = orphans.len();
    if count_only {
        if json {
            println!("{}", serde_json::json!({"count": count}));
        } else {
            println!("{}", count);
        }
        return Ok(());
    }

    if json {
        let entries: Vec<OrphanEntry> = orphans
            .iter()
            .map(|n| OrphanEntry {
                uuid: n.uuid.clone(),
                title: n.title.clone(),
                path: n.path.to_string_lossy().to_string(),
                filetags: n.filetags.clone(),
            })
            .collect();
        if ndjson {
            for e in &entries {
                println!("{}", serde_json::to_string(e)?);
            }
        } else {
            let output = OrphansOutput {
                count: orphans.len(),
                orphans: entries,
            };
            println!("{}", serde_json::to_string_pretty(&output)?);
        }
    } else {
        if !no_header {
            println!("Orphan notes ({}):", count);
        }
        for n in &orphans {
            let short = if n.uuid.len() > 8 { &n.uuid[..8] } else { &n.uuid };
            println!("  {} ({})", n.title, short);
        }
    }

    Ok(())
}
