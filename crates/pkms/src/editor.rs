use anyhow::Result;
use pkms_org::Graph;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus};

pub const DEFAULT_EDITOR: &str = "emacsclient -n";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorTarget {
    pub path: PathBuf,
    pub line_number: usize,
    pub title: String,
}

impl EditorTarget {
    pub fn from_location(
        graph: &Graph,
        path: impl Into<PathBuf>,
        line_number: usize,
        line_override: Option<usize>,
    ) -> Self {
        let path = path.into();
        Self {
            title: title_for_path(graph, &path),
            path,
            line_number: line_override.unwrap_or(line_number),
        }
    }
}

pub fn target_for_note(
    graph: &Graph,
    target: &str,
    line_override: Option<usize>,
) -> Result<EditorTarget> {
    let node = graph.resolve_target(target)?;
    let line_number = line_override.unwrap_or_else(|| first_task_line(graph, &node.path));
    Ok(EditorTarget {
        path: node.path.clone(),
        line_number,
        title: title_for_path(graph, &node.path),
    })
}

pub fn open_target(graph: &Graph, target: &str, editor: &str, line: Option<usize>) -> Result<()> {
    open(editor, &target_for_note(graph, target, line)?)
}

pub fn open(editor: &str, target: &EditorTarget) -> Result<()> {
    println!("Opening: {} (line {})", target.title, target.line_number);

    match run_editor(editor, target) {
        None => eprintln!("Empty editor command"),
        Some((_, Ok(status))) if status.success() => {}
        Some((command, Ok(status))) => {
            eprintln!("{command} exited with error: {status}");
        }
        Some((command, Err(error))) => {
            eprintln!("Failed to run {command}: {error}");
        }
    }

    Ok(())
}

fn title_for_path(graph: &Graph, path: &Path) -> String {
    graph
        .results
        .iter()
        .find(|result| result.path == path)
        .and_then(|result| result.parsed.title.clone())
        .unwrap_or_else(|| {
            path.file_stem()
                .map(|stem| stem.display().to_string())
                .unwrap_or_default()
        })
}

fn first_task_line(graph: &Graph, path: &Path) -> usize {
    graph
        .results
        .iter()
        .find(|result| result.path == path)
        .and_then(|result| {
            result
                .parsed
                .headings
                .iter()
                .find(|heading| heading.todo_state.is_some())
                .map(|heading| heading.line_number)
        })
        .unwrap_or(1)
}

fn command_parts(editor: &str) -> Vec<String> {
    shlex::split(editor).unwrap_or_else(|| vec![editor.to_string()])
}

fn run_editor(
    editor: &str,
    target: &EditorTarget,
) -> Option<(String, std::io::Result<ExitStatus>)> {
    let parts = command_parts(editor);
    let (program, arguments) = parts.split_first()?;
    let mut command = Command::new(program);
    command.args(arguments);
    command.arg(format!("+{}", target.line_number));
    command.arg(&target.path);
    Some((program.clone(), command.status()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_quoted_editor_arguments() {
        assert_eq!(
            command_parts("editor --eval 'value with spaces'"),
            ["editor", "--eval", "value with spaces"]
        );
    }

    #[test]
    fn exposes_non_zero_child_status_for_diagnostics() {
        let target = EditorTarget {
            path: PathBuf::from("note.org"),
            line_number: 7,
            title: "Note".to_string(),
        };

        let (_, status) =
            run_editor("sh -c 'exit 7'", &target).expect("editor command should not be empty");
        let status = status.expect("shell should start");

        assert_eq!(status.code(), Some(7));
    }
}
