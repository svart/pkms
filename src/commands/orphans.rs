use crate::config::Config;
use crate::graph::Graph;
use crate::output::OutputContext;
use crate::util;
use anyhow::Result;
use regex::Regex;
use serde::Serialize;

#[derive(Serialize)]
pub struct OrphansOutput {
    pub count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub showed: Option<usize>,
    pub orphans: Vec<OrphanEntry>,
}

#[derive(Serialize)]
pub struct OrphanEntry {
    pub uuid: String,
    pub title: String,
    pub path: String,
    pub filetags: Vec<String>,
    pub categories: Vec<String>,
}

pub struct OrphansOptions {
    pub limit: Option<usize>,
    pub with_dailies: bool,
}

pub fn run(config: &Config, ctx: &OutputContext, opts: &OrphansOptions) -> Result<()> {
    let graph = Graph::load(config)?;
    let mut orphans = graph.orphan_nodes();

    if !opts.with_dailies {
        let daily_re = Regex::new(r"^\d{4}-\d{2}-\d{2}$").unwrap();
        orphans.retain(|n| {
            n.path
                .file_stem()
                .and_then(|s| s.to_str())
                .is_none_or(|s| !daily_re.is_match(s))
        });
    }

    let count = orphans.len();
    let showed = opts.limit.map(|l| {
        let shown = orphans.len().min(l);
        orphans.truncate(l);
        shown
    });

    let entries: Vec<OrphanEntry> = orphans
        .iter()
        .map(|n| OrphanEntry {
            uuid: n.uuid.clone(),
            title: n.title.clone(),
            path: util::path_string(&n.path),
            filetags: n.filetags.clone(),
            categories: n.categories.clone(),
        })
        .collect();

    match ctx.format {
        crate::cli::OutputFormat::Text => {
            if count == entries.len() {
                println!("Orphan notes ({}):", count);
            } else {
                println!("Orphan notes ({}), showed: {}:", count, entries.len());
            }
            for n in &orphans {
                println!("  {} ({})", n.title, n.uuid);
            }
        }
        crate::cli::OutputFormat::Json => {
            ctx.print_json(&OrphansOutput {
                count,
                showed,
                orphans: entries,
            })?;
        }
        crate::cli::OutputFormat::Ndjson => {
            ctx.print_ndjson(&entries)?;
        }
    }

    Ok(())
}
