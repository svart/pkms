use crate::command_context::CommandContext;
use crate::editor::{self, EditorTarget};
use anyhow::Result;
use pkms_org::Graph;
use std::path::PathBuf;

pub use crate::editor::{DEFAULT_EDITOR, open_target};

pub enum OpenTarget {
    Note(String),
    Location { path: PathBuf, line_number: usize },
}

pub struct OpenOptions {
    pub targets: Vec<OpenTarget>,
    pub editor: String,
    pub line: Option<usize>,
}

pub fn run(ctx: &CommandContext<'_>, opts: &OpenOptions) -> Result<()> {
    let org_config = ctx.config().org_config();
    let graph = Graph::load(&org_config)?;

    for target in &opts.targets {
        let target = match target {
            OpenTarget::Note(target) => editor::target_for_note(&graph, target, opts.line)?,
            OpenTarget::Location { path, line_number } => {
                EditorTarget::from_location(&graph, path, *line_number, opts.line)
            }
        };
        editor::open(&opts.editor, &target)?;
    }

    Ok(())
}
