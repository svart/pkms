use crate::config::Config;
use crate::graph::Graph;
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
    limit: usize,
    db_cli: Option<&std::path::Path>,
) -> Result<()> {
    let graph = Graph::load(config, db_cli, false)?;
    let hubs = graph.hubs(limit);

    if json {
        let entries: Vec<HubEntry> = hubs
            .into_iter()
            .enumerate()
            .map(|(i, (n, deg))| {
                let outgoing = n
                    .outgoing
                    .iter()
                    .filter(|l| matches!(l, crate::parser::Link::Internal(_)))
                    .count();
                let incoming = graph.backlinks.get(&n.uuid).map_or(0, |v| v.len());
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
            for e in &entries {
                println!("{}", serde_json::to_string(e)?);
            }
        } else {
            let output = HubsOutput { limit, hubs: entries };
            println!("{}", serde_json::to_string_pretty(&output)?);
        }
    } else {
        println!("Top {} hubs:", limit);
        for (i, (node, deg)) in hubs.iter().enumerate() {
            let outgoing = node
                .outgoing
                .iter()
                .filter(|l| matches!(l, crate::parser::Link::Internal(_)))
                .count();
            let incoming = graph.backlinks.get(&node.uuid).map_or(0, |v| v.len());
            let short = if node.uuid.len() > 8 { &node.uuid[..8] } else { &node.uuid };
            println!(
                "  {:3}. {:40} {} links ({} out / {} in)  {}",
                i + 1,
                node.title,
                deg,
                outgoing,
                incoming,
                short
            );
        }
    }

    Ok(())
}
