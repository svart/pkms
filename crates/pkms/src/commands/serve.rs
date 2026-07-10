use crate::command_context::CommandContext;
use crate::editor;
use anyhow::Result;
use std::io::Write;

pub use pkms_web::ServeOptions;

pub fn run(ctx: &CommandContext<'_>, opts: &ServeOptions) -> Result<()> {
    let config = ctx.config().web_command_config();
    let output = ctx.output();

    pkms_web::serve(
        &config,
        opts,
        editor::open_target,
        editor::DEFAULT_EDITOR,
        |started| {
            if output.is_structured() {
                output.print_structured(started)?;
            } else {
                println!("Serving {}", started.url);
            }
            std::io::stdout().flush()?;
            Ok(())
        },
    )
}
