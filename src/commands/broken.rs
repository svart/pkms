use crate::config::Config;
use crate::graph::Graph;
use crate::output::OutputContext;
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

pub fn run(config: &Config, ctx: &OutputContext, db_cli: Option<&std::path::Path>) -> Result<()> {
    let graph = Graph::load(config, db_cli)?;
    let links = graph.broken_links_list();

    let entries: Vec<BrokenEntry> = links
        .iter()
        .map(|(src, title, tgt)| BrokenEntry {
            source_uuid: src.clone(),
            source_title: title.clone(),
            target_uuid: tgt.clone(),
        })
        .collect();

    match ctx.format {
        crate::cli::OutputFormat::Text => {
            println!("Broken links ({}):", links.len());
            for (_src, title, tgt) in &links {
                println!("  {title} -> {tgt}");
            }
        }
        crate::cli::OutputFormat::Json => {
            ctx.print_json(&BrokenOutput {
                count: entries.len(),
                links: entries,
            })?;
        }
        crate::cli::OutputFormat::Ndjson => {
            ctx.print_ndjson(&entries)?;
        }
    }

    Ok(())
}
