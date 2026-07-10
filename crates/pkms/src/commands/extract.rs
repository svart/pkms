use crate::cli::ExtractArgs;
use crate::command_context::CommandContext;
use crate::output::OutputContext;
use anyhow::Result;
use pkms_db::commands::extract::{self, ExtractOptions, ExtractOutput};

pub fn options_from_args(args: &ExtractArgs) -> Result<ExtractOptions> {
    let heading_uuid = uuid::Uuid::parse_str(&args.heading_uuid)
        .map_err(|_| anyhow::anyhow!("Invalid UUID format: {}", args.heading_uuid))?
        .to_string();
    Ok(ExtractOptions {
        heading_uuid,
        new_name: args.new_name.clone(),
        apply: args.apply,
    })
}

pub fn run(ctx: &CommandContext<'_>, opts: &ExtractOptions) -> Result<()> {
    let org_config = ctx.config().org_config();
    let output = extract::execute(&org_config, opts)?;
    render(ctx.output(), &output)
}

fn render(ctx: &OutputContext, output: &ExtractOutput) -> Result<()> {
    if ctx.is_structured() {
        ctx.print_structured(output)?;
    } else {
        print!("{}", extract::render_text(output));
    }

    Ok(())
}
