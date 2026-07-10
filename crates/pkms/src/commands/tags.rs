use anyhow::Result;
use pkms_db::commands::tags;

use crate::{
    cli::{OutputFormat, TagsArgs},
    command_context::CommandContext,
};

#[cfg(feature = "rag")]
mod suggest;

pub fn run(ctx: &CommandContext<'_>, args: &TagsArgs) -> Result<()> {
    #[cfg(feature = "rag")]
    if let Some(command) = &args.command {
        return match command {
            crate::cli::TagsCommand::Suggest(args) => suggest::run(ctx, args),
        };
    }
    #[cfg(not(feature = "rag"))]
    let _ = args;

    let output = tags::execute(&ctx.config().org_config())?;
    match ctx.output().format {
        OutputFormat::Text => println!("{}", tags::render_text(&output)),
        OutputFormat::Json => ctx.output().print_json(&output)?,
        OutputFormat::Ndjson => ctx.output().print_ndjson(&output.tags)?,
    }
    Ok(())
}
