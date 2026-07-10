use crate::command_context::CommandContext;
use crate::editor::{self, EditorTarget};
use anyhow::Result;
use std::path::PathBuf;

pub(super) fn run(
    ctx: &CommandContext<'_>,
    path: impl Into<PathBuf>,
    line_number: usize,
    editor_command: &str,
    line_override: Option<usize>,
) -> Result<()> {
    let graph = pkms_org::Graph::load(&ctx.config().org_config())?;
    let target = EditorTarget::from_location(&graph, path, line_number, line_override);
    editor::open(editor_command, &target)
}
