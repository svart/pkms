use crate::cli::{MentionsArgs, OutputFormat};
use crate::command_context::CommandContext;
use crate::output::OutputContext;
use anyhow::{Context, Result};
use pkms_db::commands::mentions::{self, MentionsOptions, MentionsOutput, MentionsSource};

pub fn options_from_args(args: &MentionsArgs) -> Result<MentionsOptions> {
    let source = if args.target == "-" {
        MentionsSource::Text(
            std::io::read_to_string(std::io::stdin()).context("Failed to read text from stdin")?,
        )
    } else {
        MentionsSource::Target(args.target.clone())
    };
    Ok(MentionsOptions {
        source,
        incoming: args.incoming,
        with_dailies: args.with_dailies,
    })
}

pub fn render(ctx: &OutputContext, output: &MentionsOutput) -> Result<()> {
    match ctx.format {
        OutputFormat::Text => print!("{}", mentions::render_text(output)),
        OutputFormat::Json => ctx.print_json(output)?,
        OutputFormat::Ndjson => ctx.print_ndjson(&output.mentions)?,
    }
    Ok(())
}

pub fn run(ctx: &CommandContext<'_>, opts: &MentionsOptions) -> Result<()> {
    let org_config = ctx.config().org_config();
    let output = mentions::execute(&org_config, opts)?;
    render(ctx.output(), &output)
}
