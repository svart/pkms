use crate::config::Config;
use crate::graph::Graph;
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
    json: bool,
    ndjson: bool,
    no_header: bool,
    count_only: bool,
    limit: usize,
    db_cli: Option<&std::path::Path>,
) -> Result<()> {
    let graph = Graph::load(config, db_cli, false)?;
    let hubs = graph.hubs(limit);

    if count_only {
        util::print_count(hubs.len(), json);
        return Ok(());
    }

    if json {
        let entries: Vec<HubEntry> = hubs
            .into_iter()
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
                    degree: deg,
                    outgoing,
                    incoming,
                }
            })
            .collect();
        if ndjson {
            return util::print_ndjson(&entries);
        }
        let output = HubsOutput {
            limit,
            hubs: entries,
        };
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        if !no_header {
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

    Ok(())
}
