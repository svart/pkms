use crate::config::Config;
use crate::graph::Graph;
use crate::output::OutputContext;
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

pub fn run(config: &Config, ctx: &OutputContext, db_cli: Option<&std::path::Path>) -> Result<()> {
    let graph = Graph::load(config, db_cli, false)?;
    let orphans = graph.orphan_nodes();

    let count = orphans.len();
    if ctx.count_only {
        ctx.print_count(count);
        return Ok(());
    }

    let entries: Vec<OrphanEntry> = orphans
        .iter()
        .map(|n| OrphanEntry {
            uuid: n.uuid.clone(),
            title: n.title.clone(),
            path: util::path_string(&n.path),
            filetags: n.filetags.clone(),
        })
        .collect();

    match ctx.format {
        crate::cli::OutputFormat::Text => {
            if !ctx.no_header {
                println!("Orphan notes ({count}):");
            }
            for n in &orphans {
                println!("  {} ({})", n.title, util::short_uuid(&n.uuid));
            }
        }
        crate::cli::OutputFormat::Json => {
            ctx.print_json(&OrphansOutput {
                count: orphans.len(),
                orphans: entries,
            })?;
        }
        crate::cli::OutputFormat::Ndjson => {
            ctx.print_ndjson(&entries)?;
        }
    }

    Ok(())
}
