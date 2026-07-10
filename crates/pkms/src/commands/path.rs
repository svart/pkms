use crate::cli::PathArgs;
use crate::command_context::CommandContext;
use crate::output::OutputContext;
use anyhow::Result;
use pkms_db::commands::path::{self, PathOptions, PathOutput};

pub fn options_from_args(args: &PathArgs) -> PathOptions {
    PathOptions {
        from: args.from.clone(),
        to: args.to.clone(),
    }
}

pub fn run(ctx: &CommandContext<'_>, opts: &PathOptions) -> Result<()> {
    let org_config = ctx.config().org_config();
    let output = path::execute(&org_config, opts)?;
    render(ctx.output(), &output)
}

fn render(ctx: &OutputContext, output: &PathOutput) -> Result<()> {
    if ctx.is_structured() {
        ctx.print_structured(output)
    } else {
        print!("{}", path::render_text(output));
        Ok(())
    }
}
