use crate::config::Config;
use crate::graph::Graph;
use anyhow::Result;
use serde::Serialize;

#[derive(Serialize)]
pub struct StatsOutput {
    pub db_root: String,
    pub total_notes: usize,
    pub total_links: usize,
    pub internal_links: usize,
    pub file_links: usize,
    pub url_links: usize,
    pub avg_links_per_note: f64,
    pub orphans: usize,
    pub broken_links: usize,
    pub disk_size_bytes: u64,
    pub directories: Vec<DirEntry>,
    pub hubs: Vec<HubEntry>,
    pub recent_notes: Option<Vec<RecentNote>>,
}

#[derive(Serialize)]
pub struct DirEntry {
    pub directory: String,
    pub count: usize,
}

#[derive(Serialize)]
pub struct HubEntry {
    pub uuid: String,
    pub title: String,
    pub degree: usize,
}

#[derive(Serialize)]
pub struct RecentNote {
    pub uuid: String,
    pub title: String,
    pub path: String,
}

pub fn run(
    config: &Config,
    json: bool,
    verbose: bool,
    days: Option<u32>,
    db_cli: Option<&std::path::Path>,
) -> Result<()> {
    let graph = Graph::load(config, db_cli, false)?;
    let db_root = config.resolve_db_root(db_cli)?;
    let stats = graph.stats();

    let avg = if stats.total_notes > 0 {
        stats.total_links as f64 / stats.total_notes as f64
    } else {
        0.0
    };

    let dirs = graph.directory_breakdown(&db_root);
    let hubs = graph.hubs(10);
    let disk_size = graph.disk_size();

    if json {
        let output = StatsOutput {
            db_root: db_root.to_string_lossy().to_string(),
            total_notes: stats.total_notes,
            total_links: stats.total_links,
            internal_links: stats.total_internal_links,
            file_links: stats.total_file_links,
            url_links: stats.total_url_links,
            avg_links_per_note: avg,
            orphans: stats.orphan_notes,
            broken_links: stats.broken_link_count,
            disk_size_bytes: disk_size,
            directories: dirs
                .into_iter()
                .map(|(d, c)| DirEntry {
                    directory: d,
                    count: c,
                })
                .collect(),
            hubs: hubs
                .into_iter()
                .map(|(n, d)| HubEntry {
                    uuid: n.uuid.clone(),
                    title: n.title.clone(),
                    degree: d,
                })
                .collect(),
            recent_notes: days.map(|d| {
                graph
                    .notes_since(d)
                    .into_iter()
                    .map(|n| RecentNote {
                        uuid: n.uuid.clone(),
                        title: n.title.clone(),
                        path: n.path.to_string_lossy().to_string(),
                    })
                    .collect()
            }),
        };
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        println!("Database: {}", db_root.display());
        println!("  Notes:             {}", stats.total_notes);
        println!("  Links:             {} (avg: {:.2}/note)", stats.total_links, avg);
        println!("    Internal:        {}", stats.total_internal_links);
        println!("    File:            {}", stats.total_file_links);
        println!("    URL:             {}", stats.total_url_links);
        println!("  Orphans:           {}", stats.orphan_notes);
        println!("  Broken links:      {}", stats.broken_link_count);
        println!("  Disk size:         {}", format_size(disk_size));
        println!();
        if let Some(d) = days {
            let recent = graph.notes_since(d);
            println!("  Recent ({} days):   {}", d, recent.len());
        }
        if verbose {
            println!();
            println!("Directories:");
            for (dir, count) in &dirs {
                let display = if dir.is_empty() { "(root)" } else { dir.as_str() };
                println!("  {:40} {}", display, count);
            }
        }
        println!();
        println!("Top hubs:");
        for (i, (node, deg)) in hubs.iter().enumerate() {
            println!("  {:3}. {:40} {} links", i + 1, node.title, deg);
        }
    }

    Ok(())
}

fn format_size(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB"];
    let mut size = bytes as f64;
    let mut unit = 0;
    while size > 1024.0 && unit < UNITS.len() - 1 {
        size /= 1024.0;
        unit += 1;
    }
    format!("{:.1} {}", size, UNITS[unit])
}
