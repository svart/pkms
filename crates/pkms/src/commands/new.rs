use crate::cli::NewArgs;
use crate::command_context::CommandContext;
use crate::output::OutputContext;
use anyhow::Result;
use pkms_db::commands::new::{self, NewOptions, NewOutput};

pub fn options_from_args(args: &NewArgs) -> NewOptions {
    NewOptions {
        title: args.title.clone(),
        create: args.create,
        tags: args.tags.clone(),
        aliases: args.aliases.clone(),
        heading: args.heading.clone(),
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
