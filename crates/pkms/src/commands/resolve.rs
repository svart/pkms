use crate::cli::{OutputFormat, ResolveArgs};
use crate::command_context::CommandContext;
use crate::output::OutputContext;
use anyhow::Result;
use pkms_db::commands::resolve::{self, ResolveCommandOutput, ResolveOptions};

pub fn options_from_args(args: &ResolveArgs) -> ResolveOptions {
    ResolveOptions {
        uuid: args.uuid.clone(),
        title: args.title.clone(),
        tags: args.tags.clone(),
        limit: args.limit,
        fields: args.fields.clone(),
        todos: args.todos,
    }
}

pub fn run(ctx: &CommandContext<'_>, opts: &ResolveOptions) -> Result<()> {
    let org_config = ctx.config().org_config();
    let output = resolve::execute(&org_config, opts)?;
    render(ctx.output(), &output)
}

pub fn render(ctx: &OutputContext, output: &ResolveCommandOutput) -> Result<()> {
    match ctx.format {
        OutputFormat::Text => {
            print!(
                "{}",
                resolve::render_text(&output.output, output.fields.as_ref())
            );
        }
        OutputFormat::Json => {
            ctx.print_json(&output.output)?;
        }
        OutputFormat::Ndjson => {
            for note in &output.output.results {
                let v =
                    resolve::filter_fields(&serde_json::to_value(note)?, output.fields.as_ref());
                println!("{}", serde_json::to_string(&v)?);
            }
        }
    }

    Ok(())
}
