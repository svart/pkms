use crate::cli::OutputFormat;
use crate::command_context::CommandContext;
use crate::output::OutputContext;
use anyhow::Result;
use pkms_db::commands::get::{self, GetOutput};

pub use pkms_db::commands::get::GetOptions;

pub fn render(ctx: &OutputContext, outputs: &[GetOutput]) -> Result<()> {
    match ctx.format {
        OutputFormat::Text => {
            print!("{}", get::render_text(outputs));
        }
        OutputFormat::Json => {
            if outputs.len() == 1 {
                ctx.print_json(&outputs[0])?;
            } else {
                ctx.print_json(outputs)?;
            }
        }
        OutputFormat::Ndjson => ctx.print_ndjson(outputs)?,
    }

    Ok(())
}

pub fn run(ctx: &CommandContext<'_>, opts: &GetOptions) -> Result<()> {
    let org_config = ctx.config().org_config();
    let outputs = get::execute(&org_config, opts)?;
    render(ctx.output(), &outputs)
}
