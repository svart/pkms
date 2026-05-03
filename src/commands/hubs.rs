use crate::cli::OutputFormat;
use crate::config::Config;
use crate::graph::Graph;
use crate::output::OutputContext;
use crate::parser::Link;
use crate::util;
use anyhow::Result;
use serde::Serialize;

#[derive(Serialize)]
pub struct HubsOutput {
    pub limit: usize,
    pub hubs: Vec<HubEntry>,
}

#[derive(Serialize)]
pub struct HubEntry {
    pub rank: usize,
    pub uuid: String,
    pub title: String,
    pub degree: usize,
    pub outgoing: usize,
    pub incoming: usize,
}

pub fn run(
    config: &Config,
    ctx: &OutputContext,
    limit: usize,
    db_cli: Option<&std::path::Path>,
) -> Result<()> {
    let graph = Graph::load(config, db_cli, false)?;
    let hubs = graph.hubs(limit);

    if ctx.count_only {
        ctx.print_count(hubs.len());
        return Ok(());
    }

    let entries: Vec<HubEntry> = hubs
        .iter()
        .enumerate()
        .map(|(i, (n, deg))| {
            let outgoing = n
                .outgoing
                .iter()
                .filter(|l| matches!(l, Link::Internal(_)))
                .count();
            let incoming = graph.backlinks.get(&n.uuid).map_or(0, std::vec::Vec::len);
            HubEntry {
                rank: i + 1,
                uuid: n.uuid.clone(),
                title: n.title.clone(),
                degree: *deg,
                outgoing,
                incoming,
            }
        })
        .collect();

    match ctx.format {
        OutputFormat::Text => {
            if !ctx.no_header {
                println!("Top {limit} hubs:");
            }
            for (i, (node, deg)) in hubs.iter().enumerate() {
                let outgoing = node
                    .outgoing
                    .iter()
                    .filter(|l| matches!(l, Link::Internal(_)))
                    .count();
                let incoming = graph
                    .backlinks
                    .get(&node.uuid)
                    .map_or(0, std::vec::Vec::len);
                println!(
                    "  {:3}. {:40} {} links ({} out / {} in)  {}",
                    i + 1,
                    node.title,
                    deg,
                    outgoing,
                    incoming,
                    util::short_uuid(&node.uuid)
                );
            }
        }
        OutputFormat::Json => {
            ctx.print_json(&HubsOutput {
                limit,
                hubs: entries,
            })?;
        }
        OutputFormat::Ndjson => {
            ctx.print_ndjson(&entries)?;
        }
    }

    Ok(())
}
