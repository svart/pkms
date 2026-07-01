use crate::cli::{OutputFormat, StatsArgs};
use crate::command_context::CommandContext;
use crate::config::ResolvedConfig;
use crate::graph::Graph;
use crate::output::OutputContext;
use crate::util::format_size;
use anyhow::Result;
use serde::Serialize;
use std::collections::{BTreeMap, HashMap, HashSet};

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
    #[serde(skip_serializing)]
    pub recent_days: Option<u32>,
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

impl From<&crate::graph::Node> for RecentNote {
    fn from(n: &crate::graph::Node) -> Self {
        RecentNote {
            uuid: n.uuid.to_string(),
            title: n.title.clone(),
            path: n.path.display().to_string(),
        }
    }
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

#[derive(Serialize)]
#[serde(untagged)]
pub enum StatsCommandOutput {
    Stats(StatsOutput),
    Hubs(HubsOutput),
    Tags(TagsOutput),
    Todos(TodoStats),
}

pub struct StatsOptions {
    pub days: Option<u32>,
    pub hubs: Option<usize>,
    pub tags: bool,
    pub todos: bool,
}

impl From<&StatsArgs> for StatsOptions {
    fn from(args: &StatsArgs) -> Self {
        StatsOptions {
            days: args.days,
            hubs: args.hubs,
            tags: args.tags,
            todos: args.todos,
        }
    }
}

pub fn run(ctx: &CommandContext<'_>, opts: &StatsOptions) -> Result<()> {
    let output = execute(ctx.config(), opts)?;
    render(ctx.output(), &output)
}

pub fn execute(config: &ResolvedConfig, opts: &StatsOptions) -> Result<StatsCommandOutput> {
    let graph = Graph::load(config)?;
    let db_root = config.resolved_db_root();

    if let Some(limit) = opts.hubs {
        return Ok(StatsCommandOutput::Hubs(build_hubs_output(&graph, limit)));
    }

    if opts.tags {
        return Ok(StatsCommandOutput::Tags(build_tags_output(&graph)));
    }

    if opts.todos {
        return Ok(StatsCommandOutput::Todos(build_todo_stats(&graph)));
    }

    Ok(StatsCommandOutput::Stats(build_stats_output(
        opts.days, &graph, db_root,
    )))
}

pub fn render(ctx: &OutputContext, output: &StatsCommandOutput) -> Result<()> {
    match (&ctx.format, output) {
        (OutputFormat::Text, StatsCommandOutput::Stats(output)) => {
            println!("{}", render_stats_text(output));
        }
        (OutputFormat::Text, StatsCommandOutput::Hubs(output)) => {
            println!("{}", render_hubs_text(output));
        }
        (OutputFormat::Text, StatsCommandOutput::Tags(output)) => {
            println!("{}", render_tags_text(output));
        }
        (OutputFormat::Text, StatsCommandOutput::Todos(output)) => {
            println!("{}", render_todo_stats_text(output));
        }
        (OutputFormat::Json, output) => ctx.print_json(output)?,
        (OutputFormat::Ndjson, StatsCommandOutput::Stats(output)) => ctx.print_ndjson(&[output])?,
        (OutputFormat::Ndjson, StatsCommandOutput::Hubs(output)) => {
            ctx.print_ndjson(&output.hubs)?;
        }
        (OutputFormat::Ndjson, StatsCommandOutput::Tags(output)) => {
            ctx.print_ndjson(&output.tags)?;
        }
        (OutputFormat::Ndjson, StatsCommandOutput::Todos(output)) => ctx.print_ndjson(&[output])?,
    }

    Ok(())
}

fn build_hubs_output(graph: &Graph, limit: usize) -> HubsOutput {
    let hubs = graph.hubs(limit);
    let degrees = graph.authored_internal_degrees();

    let hubs: Vec<HubEntryDetailed> = hubs
        .iter()
        .enumerate()
        .map(|(i, (n, deg))| {
            let (outgoing, incoming) = degrees.get(&n.uuid).copied().unwrap_or_default();
            HubEntryDetailed {
                rank: i + 1,
                uuid: n.uuid.to_string(),
                title: n.title.clone(),
                degree: *deg,
                outgoing,
                incoming,
            }
        })
        .collect();

    HubsOutput { limit, hubs }
}

fn build_tags_output(graph: &Graph) -> TagsOutput {
    let mut tag_counts: HashMap<String, usize> = HashMap::new();
    for result in &graph.results {
        let mut seen_tags = HashSet::new();
        for tag in &result.parsed.filetags {
            if seen_tags.insert(tag.as_str()) {
                *tag_counts.entry(tag.clone()).or_default() += 1;
            }
        }
    }

    let mut tags: Vec<(String, usize)> = tag_counts.into_iter().collect();
    tags.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));

    let tags = tags
        .into_iter()
        .map(|(tag, count)| TagEntry {
            tag,
            count,
            notes: vec![],
        })
        .collect();

    TagsOutput { tags }
}

fn build_todo_stats(graph: &Graph) -> TodoStats {
    let mut by_state: BTreeMap<String, usize> = BTreeMap::new();

    let files_with_todos = graph
        .results
        .iter()
        .filter(|result| result.parsed.has_todo_headings())
        .count();

    for result in &graph.results {
        for heading in &result.parsed.headings {
            if let Some(ref state) = heading.todo_state {
                *by_state.entry(state.to_uppercase()).or_default() += 1;
            }
        }
    }

    let total: usize = by_state.values().sum();
    let state_entries: Vec<TodoStateEntry> = by_state
        .into_iter()
        .map(|(state, count)| TodoStateEntry { state, count })
        .collect();

    TodoStats {
        total_todo_headings: total,
        files_with_todos,
        by_state: state_entries,
    }
}

#[allow(clippy::cast_precision_loss)]
fn build_stats_output(days: Option<u32>, graph: &Graph, db_root: &std::path::Path) -> StatsOutput {
    let stats = graph.stats();

    let avg = if stats.total_notes > 0 {
        stats.total_links as f64 / stats.total_notes as f64
    } else {
        0.0
    };

    let dirs = graph.directory_breakdown(db_root);
    let disk_size = graph.disk_size();

    StatsOutput {
        db_root: db_root.display().to_string(),
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
                .map(RecentNote::from)
                .collect()
        }),
        recent_days: days,
        todo_stats: None,
    }
}

pub fn render_stats_text(output: &StatsOutput) -> String {
    let mut lines = vec![
        format!("Database: {}", output.db_root),
        format!("  Notes:             {}", output.total_notes),
        format!(
            "  Links:             {} (avg: {:.2}/note)",
            output.total_links, output.avg_links_per_note
        ),
        format!("    Internal:        {}", output.internal_links),
        format!("    File:            {}", output.file_links),
        format!("    URL:             {}", output.url_links),
        format!("  Orphans:           {}", output.orphans),
        format!("  Broken links:      {}", output.broken_links),
        format!(
            "  Disk size:         {}",
            format_size(output.disk_size_bytes)
        ),
        String::new(),
    ];
    if let (Some(days), Some(recent)) = (output.recent_days, &output.recent_notes) {
        lines.push(format!("  Recent ({days} days):   {}", recent.len()));
    }
    lines.join("\n")
}

pub fn render_hubs_text(output: &HubsOutput) -> String {
    let mut lines = vec![format!("Top {} hubs:", output.limit)];
    lines.extend(output.hubs.iter().map(|entry| {
        format!(
            "  {:3}. {:40} {} links ({} out / {} in)  {}",
            entry.rank, entry.title, entry.degree, entry.outgoing, entry.incoming, entry.uuid
        )
    }));
    lines.join("\n")
}

pub fn render_tags_text(output: &TagsOutput) -> String {
    let mut lines = vec!["Filetags (count):".to_string()];
    lines.extend(
        output
            .tags
            .iter()
            .map(|entry| format!("  {:30} {}", entry.tag, entry.count)),
    );
    lines.push(String::new());
    lines.push(format!("Total unique tags: {}", output.tags.len()));
    lines.join("\n")
}

pub fn render_todo_stats_text(output: &TodoStats) -> String {
    let mut lines = vec![
        "TODO Statistics:".to_string(),
        format!("  Total TODO headings: {}", output.total_todo_headings),
        format!("  Files with TODOs:    {}", output.files_with_todos),
        String::new(),
        "  By state:".to_string(),
    ];
    lines.extend(
        output
            .by_state
            .iter()
            .map(|entry| format!("    {:20} {}", entry.state, entry.count)),
    );
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_stats_text_from_typed_output() {
        let output = StatsOutput {
            db_root: "/db".to_string(),
            total_notes: 2,
            total_links: 3,
            internal_links: 1,
            file_links: 1,
            url_links: 1,
            avg_links_per_note: 1.5,
            orphans: 1,
            broken_links: 0,
            disk_size_bytes: 1024,
            directories: vec![DirEntry {
                directory: "roam".to_string(),
                count: 2,
            }],
            recent_days: None,
            recent_notes: None,
            todo_stats: None,
        };

        let text = render_stats_text(&output);

        assert!(text.contains("Database: /db"));
        assert!(text.contains("  Links:             3 (avg: 1.50/note)"));
        assert!(text.contains("  Disk size:         1.0 KB"));
    }

    #[test]
    fn renders_todo_stats_text_from_typed_output() {
        let output = TodoStats {
            total_todo_headings: 2,
            files_with_todos: 1,
            by_state: vec![TodoStateEntry {
                state: "TODO".to_string(),
                count: 2,
            }],
        };

        assert_eq!(
            render_todo_stats_text(&output),
            "TODO Statistics:\n  Total TODO headings: 2\n  Files with TODOs:    1\n\n  By state:\n    TODO                 2"
        );
    }
}
