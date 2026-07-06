use crate::cli::OutputFormat;
use crate::command_context::CommandContext;
use crate::output::{OutputContext, terminal_markup};
use anyhow::Result;
use pkms_db::commands::show::{self, ShowOutput};

pub use pkms_db::commands::show::{HeadingTarget, ShowOptions, TaskIdEntry};

pub fn run(ctx: &CommandContext<'_>, opts: &ShowOptions) -> Result<()> {
    let config = ctx.config().db_command_config();
    let outputs = show::execute(&config.org, opts)?;
    render(ctx.output(), &outputs)
}

pub fn render(ctx: &OutputContext, outputs: &[ShowOutput]) -> Result<()> {
    match ctx.format {
        OutputFormat::Text => {
            let text = show::render_text(outputs);
            print!("{}", terminal_markup::format_if_terminal_supported(&text));
        }
        OutputFormat::Json => {
            ctx.print_json_adaptive(outputs)?;
        }
        OutputFormat::Ndjson => {
            ctx.print_ndjson(outputs)?;
        }
    }

    Ok(())
}
