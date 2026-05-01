use crate::config::Config;
use crate::graph::Graph;
use crate::util;
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
        return util::print_count(count, json);
    }

    if json {
        let entries: Vec<OrphanEntry> = orphans
            .iter()
            .map(|n| OrphanEntry {
                uuid: n.uuid.clone(),
                title: n.title.clone(),
                path: util::path_string(&n.path),
                filetags: n.filetags.clone(),
            })
            .collect();
        if ndjson {
            return util::print_ndjson(&entries);
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
            println!("  {} ({})", n.title, util::short_uuid(&n.uuid));
        }
    }

    Ok(())
}
