use crate::cli::OrphansArgs;
use crate::config::ResolvedConfig;
use crate::graph::Graph;
use crate::output::OutputContext;
use crate::util;
use anyhow::Result;
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

impl From<&OrphansArgs> for OrphansOptions {
    fn from(args: &OrphansArgs) -> Self {
        OrphansOptions {
            limit: args.limit,
            with_dailies: args.with_dailies,
        }
    }
}

pub fn run(config: &ResolvedConfig, ctx: &OutputContext, opts: &OrphansOptions) -> Result<()> {
    let graph = Graph::load(config)?;
    let mut orphans = if opts.with_dailies {
        graph.orphan_nodes_including_dailies()
    } else {
        graph.orphan_nodes()
    };

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
