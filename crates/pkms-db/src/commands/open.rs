use anyhow::Result;
use pkms_org::graph::tasks::{TaskLocation, TaskStateConfig};
use pkms_org::{Graph, OrgConfig};

pub struct OpenOptions {
    pub targets: Vec<String>,
    pub editor: String,
    pub line: Option<usize>,
}

pub const DEFAULT_EDITOR: &str = "emacsclient -n";

fn find_line_for_node(graph: &Graph, path: &std::path::Path) -> usize {
    graph
        .results
        .iter()
        .find(|r| r.path == *path)
        .and_then(|r| {
            r.parsed
                .headings
                .iter()
                .find(|h| h.todo_state.is_some())
                .map(|h| h.line_number)
        })
        .unwrap_or(1)
}

pub fn open_target(
    graph: &Graph,
    task_states: &TaskStateConfig,
    target: &str,
    editor: &str,
    line: Option<usize>,
) -> Result<()> {
    let location = if let Ok(id) = target.parse::<usize>() {
        graph.resolve_canonical_task_id(task_states, id)?
    } else {
        let node = graph.resolve_target(target)?;
        let path = node.path.display().to_string();
        let line_number = line.unwrap_or_else(|| find_line_for_node(graph, &node.path));
        TaskLocation { path, line_number }
    };

    let actual_line = line.unwrap_or(location.line_number);

    let path_ref = std::path::Path::new(&location.path);
    let title = graph
        .results
        .iter()
        .find(|r| r.path == *path_ref)
        .and_then(|r| r.parsed.title.as_deref())
        .map(|s| s.to_string())
        .unwrap_or_else(|| {
            path_ref
                .file_stem()
                .map(|s| s.display().to_string())
                .unwrap_or_default()
        });
    println!("Opening: {} (line {})", title, actual_line);

    let editor_parts = shlex::split(editor).unwrap_or_else(|| vec![editor.to_string()]);
    let Some((first, rest)) = editor_parts.split_first() else {
        eprintln!("Empty editor command");
        return Ok(());
    };
    let mut cmd = std::process::Command::new(first);
    if !rest.is_empty() {
        cmd.args(rest);
    }
    cmd.arg(format!("+{actual_line}"));
    cmd.arg(&location.path);

    let status = cmd.status();
    match status {
        Ok(s) if s.success() => {}
        Ok(s) => eprintln!("{first} exited with error: {s}"),
        Err(e) => eprintln!("Failed to run {first}: {e}"),
    }

    Ok(())
}

pub fn execute(
    org_config: &OrgConfig,
    task_states: &TaskStateConfig,
    opts: &OpenOptions,
) -> Result<()> {
    let graph = Graph::load(org_config)?;

    for target in &opts.targets {
        open_target(&graph, task_states, target, &opts.editor, opts.line)?;
    }

    Ok(())
}
