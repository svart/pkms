use crate::cli::OrphansArgs;
use crate::cli::OutputFormat;
use crate::command_context::CommandContext;
use crate::output::OutputContext;
use anyhow::Result;
use pkms_db::commands::orphans::{self, OrphansOptions, OrphansOutput};

pub fn options_from_args(args: &OrphansArgs) -> OrphansOptions {
    OrphansOptions {
        limit: args.limit,
        with_dailies: args.with_dailies,
    }
}

pub fn render(ctx: &OutputContext, output: &OrphansOutput) -> Result<()> {
    match ctx.format {
        OutputFormat::Text => {
            println!("{}", orphans::render_text(output));
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
    let org_config = ctx.config().org_config();
    let output = orphans::execute(&org_config, opts)?;
    render(ctx.output(), &output)
}
