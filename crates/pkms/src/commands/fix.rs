use crate::cli::{FixArgs, FixAttachArgs, FixCommand, FixUuidArgs};
use crate::command_context::CommandContext;
use crate::output::OutputContext;
use anyhow::Result;
use pkms_db::commands::fix::{
    self, AttachFixOutput, FixAttachOptions, FixUuidOptions, UuidFixOutput,
};

pub fn run(ctx: &CommandContext<'_>, args: &FixArgs) -> Result<()> {
    let org_config = ctx.config().org_config();
    match &args.command {
        FixCommand::Uuid(args) => {
            let output = fix::execute_uuid(&org_config, &uuid_options_from_args(args)?)?;
            print_uuid_fix_output(ctx.output(), &output)
        }
        FixCommand::Attach(args) => {
            let output = fix::execute_attach(&org_config, &attach_options_from_args(args))?;
            print_attach_fix_output(ctx.output(), &output)
        }
    }
}

fn uuid_options_from_args(args: &FixUuidArgs) -> Result<FixUuidOptions> {
    let broken_uuid = uuid::Uuid::parse_str(&args.broken_uuid)
        .map_err(|_| anyhow::anyhow!("Invalid UUID format: {}", args.broken_uuid))?
        .to_string();
    let target_uuid = uuid::Uuid::parse_str(&args.target)
        .map_err(|_| anyhow::anyhow!("Invalid UUID format: {}", args.target))?
        .to_string();
    Ok(FixUuidOptions {
        broken_uuid,
        target_uuid,
        apply: args.apply,
    })
}

fn attach_options_from_args(args: &FixAttachArgs) -> FixAttachOptions {
    FixAttachOptions {
        apply: args.apply,
        copy: args.copy,
    }
}

fn print_uuid_fix_output(ctx: &OutputContext, output: &UuidFixOutput) -> Result<()> {
    if ctx.is_structured() {
        ctx.print_structured(output)?;
    } else {
        print!("{}", fix::render_uuid_text(output));
    }

    Ok(())
}

fn print_attach_fix_output(ctx: &OutputContext, output: &AttachFixOutput) -> Result<()> {
    if ctx.is_structured() {
        ctx.print_structured(output)?;
    } else {
        print!("{}", fix::render_attach_text(output));
    }

    Ok(())
}
