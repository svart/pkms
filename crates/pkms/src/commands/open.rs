use crate::command_context::CommandContext;
use anyhow::Result;

pub use pkms_db::commands::open::{DEFAULT_EDITOR, OpenOptions, OpenTarget, open_target};

pub fn run(ctx: &CommandContext<'_>, opts: &OpenOptions) -> Result<()> {
    let org_config = ctx.config().org_config();
    pkms_db::commands::open::execute(&org_config, opts)
}
