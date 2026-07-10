use crate::cli::OutputFormat;
use crate::command_context::CommandContext;
use crate::output::{OutputContext, terminal_markup};
use anyhow::Result;
use pkms_task::ShowOutput;

pub use pkms_task::{HeadingTarget, ShowOptions, TaskIdEntry};

pub(super) fn run(ctx: &CommandContext<'_>, opts: &ShowOptions) -> Result<()> {
    let org_config = ctx.config().org_config();
    let outputs = pkms_task::execute_show(&org_config, opts)?;
    render(ctx.output(), &outputs)
}

fn render(ctx: &OutputContext, outputs: &[ShowOutput]) -> Result<()> {
    match ctx.format {
        OutputFormat::Text => {
            let text = pkms_task::render_show_text(outputs);
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
