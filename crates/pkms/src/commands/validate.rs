use crate::cli::OutputFormat;
use crate::command_context::CommandContext;
use crate::output::OutputContext;
use anyhow::Result;
use pkms_db::commands::validate::{self, ValidateOutput};

pub use pkms_db::commands::validate::ValidateOptions;

pub fn run(ctx: &CommandContext<'_>, opts: &ValidateOptions) -> Result<()> {
    let org_config = ctx.config().org_config();
    let outputs = validate::execute(&org_config, opts)?;
    render(ctx.output(), &outputs)
}

pub fn render(ctx: &OutputContext, outputs: &[ValidateOutput]) -> Result<()> {
    match ctx.format {
        OutputFormat::Text => {
            print!("{}", validate::render_text(outputs));
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
