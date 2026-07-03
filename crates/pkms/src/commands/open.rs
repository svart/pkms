use crate::command_context::CommandContext;
use anyhow::Result;

pub use pkms_db::commands::open::{DEFAULT_EDITOR, OpenOptions, open_target};

pub fn run(ctx: &CommandContext<'_>, opts: &OpenOptions) -> Result<()> {
    let config = ctx.config().db_command_config();
    pkms_db::commands::open::execute(&config.org, &config.task_states, opts)
}
