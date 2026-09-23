use crate::cli::NewArgs;
use crate::command_context::CommandContext;
use crate::output::OutputContext;
use anyhow::{Context, Result};
use pkms_db::commands::new::{self, NewOptions, NewOutput};
use std::io::Read;
use std::path::Path;

pub fn options_from_args(args: &NewArgs) -> Result<NewOptions> {
    Ok(NewOptions {
        title: args.title.clone(),
        create: args.create,
        tags: args.tags.clone(),
        aliases: args.aliases.clone(),
        heading: args.heading.clone(),
        body: args.body.as_deref().map(read_body).transpose()?,
    })
}

fn read_body(source: &Path) -> Result<String> {
    if source == Path::new("-") {
        let mut body = String::new();
        std::io::stdin()
            .read_to_string(&mut body)
            .context("Failed to read note body from stdin")?;
        Ok(body)
    } else {
        std::fs::read_to_string(source)
            .with_context(|| format!("Failed to read note body from {}", source.display()))
    }
}

pub fn run(ctx: &CommandContext<'_>, opts: &NewOptions) -> Result<()> {
    let config = ctx.config().note_creation_config();
    let output = new::execute(&config, opts)?;
    render(ctx.output(), &output)
}

fn render(ctx: &OutputContext, output: &NewOutput) -> Result<()> {
    if ctx.is_structured() {
        ctx.print_structured(output)?;
    } else {
        print!("{}", new::render_text(output));
    }

    Ok(())
}
