use crate::cli::OrphansArgs;
use crate::cli::OutputFormat;
use crate::command_context::CommandContext;
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

pub fn execute(config: &ResolvedConfig, opts: &OrphansOptions) -> Result<OrphansOutput> {
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
            uuid: n.uuid.to_string(),
            title: n.title.clone(),
            path: util::path_string(&n.path),
            filetags: n.filetags.clone(),
            categories: n.categories.clone(),
        })
        .collect();

    Ok(OrphansOutput {
        count,
        showed,
        orphans: entries,
    })
}

pub fn render_text(output: &OrphansOutput) -> String {
    let mut lines = Vec::new();
    if output.count == output.orphans.len() {
        lines.push(format!("Orphan notes ({}):", output.count));
    } else {
        lines.push(format!(
            "Orphan notes ({}), showed: {}:",
            output.count,
            output.orphans.len()
        ));
    }
    lines.extend(
        output
            .orphans
            .iter()
            .map(|entry| format!("  {} ({})", entry.title, entry.uuid)),
    );
    lines.join("\n")
}

pub fn render(ctx: &OutputContext, output: &OrphansOutput) -> Result<()> {
    match ctx.format {
        OutputFormat::Text => {
            println!("{}", render_text(output));
        }
        OutputFormat::Json => {
            ctx.print_json(output)?;
        }
        OutputFormat::Ndjson => {
            ctx.print_ndjson(&output.orphans)?;
        }
    }

    Ok(())
}

pub fn run(ctx: &CommandContext<'_>, opts: &OrphansOptions) -> Result<()> {
    let output = execute(ctx.config(), opts)?;
    render(ctx.output(), &output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_limited_text_from_typed_output() {
        let output = OrphansOutput {
            count: 2,
            showed: Some(1),
            orphans: vec![OrphanEntry {
                uuid: "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa".to_string(),
                title: "Lonely".to_string(),
                path: "/db/lonely.org".to_string(),
                filetags: Vec::new(),
                categories: Vec::new(),
            }],
        };

        assert_eq!(
            render_text(&output),
            "Orphan notes (2), showed: 1:\n  Lonely (aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa)"
        );
    }
}
