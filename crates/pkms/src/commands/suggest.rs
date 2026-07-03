use crate::cli::OutputFormat;
use crate::command_context::CommandContext;
use crate::output::OutputContext;
use anyhow::Result;
use pkms_db::commands::suggest::{self, SuggestOutput};

pub use pkms_db::commands::suggest::SuggestOptions;

pub fn run(ctx: &CommandContext<'_>, opts: &SuggestOptions) -> Result<()> {
    let config = ctx.config().db_command_config();
    let outputs = suggest::execute(&config.org, opts)?;
    render(ctx.output(), &outputs)
}

pub fn render(ctx: &OutputContext, outputs: &[SuggestOutput]) -> Result<()> {
    match ctx.format {
        OutputFormat::Text => {
            print!("{}", suggest::render_text(outputs));
        }
        OutputFormat::Json => {
            if outputs.len() == 1 {
                ctx.print_json(&outputs[0])?;
            } else {
                ctx.print_json(outputs)?;
            }
        }
        OutputFormat::Ndjson => {
            for output in outputs {
                for suggestion in suggest::ndjson_suggestions(output) {
                    println!("{}", serde_json::to_string(&suggestion)?);
                }
            }
        }
    }

    Ok(())
}
