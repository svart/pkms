use crate::cli::OutputFormat;
use crate::config::Config;
use crate::graph::Graph;
use crate::output::OutputContext;
use crate::parser::Link;
use anyhow::Result;
use serde::Serialize;
use std::collections::BTreeMap;

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
    pub recent_notes: Option<Vec<RecentNote>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub todo_stats: Option<TodoStats>,
}

#[derive(Serialize)]
pub struct TodoStats {
    pub total_todo_headings: usize,
    pub files_with_todos: usize,
    pub by_state: Vec<TodoStateEntry>,
}

#[derive(Serialize)]
pub struct TodoStateEntry {
    pub state: String,
    pub count: usize,
}

#[derive(Serialize)]
pub struct DirEntry {
    pub directory: String,
    pub count: usize,
}

#[derive(Serialize)]
pub struct HubEntryDetailed {
    pub rank: usize,
    pub uuid: String,
    pub title: String,
    pub degree: usize,
    pub outgoing: usize,
    pub incoming: usize,
}

#[derive(Serialize)]
pub struct HubsOutput {
    pub limit: usize,
    pub hubs: Vec<HubEntryDetailed>,
}

#[derive(Serialize)]
pub struct RecentNote {
    pub uuid: String,
    pub title: String,
    pub path: String,
}

#[derive(Serialize)]
pub struct TagsOutput {
    pub tags: Vec<TagEntry>,
}

#[derive(Serialize)]
pub struct TagEntry {
    pub tag: String,
    pub count: usize,
    pub notes: Vec<TagNote>,
}

#[derive(Serialize)]
pub struct TagNote {
    pub uuid: String,
    pub title: String,
    pub path: String,
}

pub struct StatsOptions {
    pub days: Option<u32>,
    pub hubs: Option<usize>,
    pub tags: bool,
    pub todos: bool,
}

pub fn run(config: &Config, ctx: &OutputContext, opts: &StatsOptions) -> Result<()> {
    let graph = Graph::load(config)?;
    let db_root = config.resolved_db_root()?;

    if let Some(limit) = opts.hubs {
        return print_hubs(ctx, &graph, limit);
    }

    if opts.tags {
        return print_tags(ctx, &graph);
    }

    if opts.todos {
        return print_todo_stats(ctx, &graph);
    }

    print_stats(config, ctx, opts.days, &graph, db_root)
}

fn print_hubs(ctx: &OutputContext, graph: &Graph, limit: usize) -> Result<()> {
    let hubs = graph.hubs(limit);

    let entries: Vec<HubEntryDetailed> = hubs
        .iter()
        .enumerate()
        .map(|(i, (n, deg))| {
            let outgoing = n
                .outgoing
                .iter()
                .filter(|l| matches!(l, Link::Internal(_)))
                .count();
            let incoming = graph.backlinks.get(&n.uuid).map_or(0, std::vec::Vec::len);
            HubEntryDetailed {
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
            println!("Top {limit} hubs:");
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
                    node.uuid
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

fn print_tags(ctx: &OutputContext, graph: &Graph) -> Result<()> {
    let tags = graph.all_tags();
    let entries: Vec<TagEntry> = tags
        .iter()
        .map(|(tag, count)| TagEntry {
            tag: tag.clone(),
            count: *count,
            notes: vec![],
        })
        .collect();

    match ctx.format {
        OutputFormat::Text => {
            println!("Filetags (count):");
            for (tag, count) in &tags {
                println!("  {tag:30} {count}");
            }
            println!();
            println!("Total unique tags: {}", tags.len());
        }
        OutputFormat::Json => {
            ctx.print_json(&TagsOutput { tags: entries })?;
        }
        OutputFormat::Ndjson => {
            ctx.print_ndjson(&entries)?;
        }
    }

    Ok(())
}

fn print_todo_stats(ctx: &OutputContext, graph: &Graph) -> Result<()> {
    let mut by_state: BTreeMap<String, usize> = BTreeMap::new();
    let mut files_with_todos = 0;

    for node in graph.nodes.values() {
        if node.has_todos {
            files_with_todos += 1;
        }
    }

    for node in graph.nodes.values() {
        if let Ok(content) = std::fs::read_to_string(&node.path) {
            let parsed = crate::parser::parse_note(&content);
            for heading in &parsed.headings {
                if let Some(ref state) = heading.todo_state {
                    *by_state.entry(state.to_uppercase()).or_default() += 1;
                }
            }
        }
    }

    let total: usize = by_state.values().sum();
    let state_entries: Vec<TodoStateEntry> = by_state
        .into_iter()
        .map(|(state, count)| TodoStateEntry { state, count })
        .collect();

    let stats = TodoStats {
        total_todo_headings: total,
        files_with_todos,
        by_state: state_entries,
    };

    match ctx.format {
        OutputFormat::Text => {
            println!("TODO Statistics:");
            println!("  Total TODO headings: {}", stats.total_todo_headings);
            println!("  Files with TODOs:    {}", stats.files_with_todos);
            println!();
            println!("  By state:");
            for entry in &stats.by_state {
                println!("    {:20} {}", entry.state, entry.count);
            }
        }
        OutputFormat::Json => {
            ctx.print_json(&stats)?;
        }
        OutputFormat::Ndjson => {
            println!("{}", serde_json::to_string(&stats)?);
        }
    }

    Ok(())
}

#[allow(clippy::cast_precision_loss)]
fn print_stats(
    _config: &Config,
    ctx: &OutputContext,
    days: Option<u32>,
    graph: &Graph,
    db_root: &std::path::Path,
) -> Result<()> {
    let stats = graph.stats();

    let avg = if stats.total_notes > 0 {
        stats.total_links as f64 / stats.total_notes as f64
    } else {
        0.0
    };

    let dirs = graph.directory_breakdown(db_root);
    let disk_size = graph.disk_size();

    if ctx.is_json() {
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
            todo_stats: None,
        };
        ctx.print_json(&output)?;
    } else {
        println!("Database: {}", db_root.display());
        println!("  Notes:             {}", stats.total_notes);
        println!(
            "  Links:             {} (avg: {:.2}/note)",
            stats.total_links, avg
        );
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
    }

    Ok(())
}

fn format_size(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB"];
    let mut unit = 0;
    let mut divisor = 1u64;
    while bytes / divisor >= 1024 && unit < UNITS.len() - 1 {
        divisor *= 1024;
        unit += 1;
    }
    let whole = bytes / divisor;
    let frac = (bytes % divisor) * 10 / divisor;
    format!("{}.{} {}", whole, frac, UNITS[unit])
}
