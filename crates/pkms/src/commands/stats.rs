use crate::cli::{OutputFormat, StatsArgs};
use crate::command_context::CommandContext;
use crate::output::OutputContext;
use anyhow::Result;
use pkms_db::commands::stats::{self, StatsCommandOutput, StatsOptions};

pub fn options_from_args(args: &StatsArgs) -> StatsOptions {
    StatsOptions {
        days: args.days,
        hubs: args.hubs,
        tags: args.tags,
        todos: args.todos,
    }
}

pub fn run(ctx: &CommandContext<'_>, opts: &StatsOptions) -> Result<()> {
    let config = ctx.config().db_command_config();
    let output = stats::execute(&config.org, opts)?;
    render(ctx.output(), &output)
}

pub fn render(ctx: &OutputContext, output: &StatsCommandOutput) -> Result<()> {
    match (&ctx.format, output) {
        (OutputFormat::Text, StatsCommandOutput::Stats(output)) => {
            println!("{}", stats::render_stats_text(output));
        }
        (OutputFormat::Text, StatsCommandOutput::Hubs(output)) => {
            println!("{}", stats::render_hubs_text(output));
        }
        (OutputFormat::Text, StatsCommandOutput::Tags(output)) => {
            println!("{}", stats::render_tags_text(output));
        }
        (OutputFormat::Text, StatsCommandOutput::Todos(output)) => {
            println!("{}", stats::render_todo_stats_text(output));
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
