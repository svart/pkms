use crate::config::Config;
use crate::graph::Graph;
use crate::output::OutputContext;
use anyhow::Result;

pub struct OpenOptions {
    pub targets: Vec<String>,
    pub editor: String,
    pub line: Option<usize>,
}

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

fn open_target(
    graph: &Graph,
    config: &Config,
    target: &str,
    editor: &str,
    line: Option<usize>,
) -> Result<()> {
    let (path, line_number) = if let Ok(id) = target.parse::<usize>() {
        graph.resolve_canonical_task_id(config, id)?
    } else {
        let node = graph.resolve_target(target)?;
        let path = node.path.display().to_string();
        let line = line.unwrap_or_else(|| find_line_for_node(graph, &node.path));
        (path, line)
    };

    let actual_line = line.unwrap_or(line_number);

    let path_ref = std::path::Path::new(&path);
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

    let editor_parts: Vec<&str> = editor.split_whitespace().collect();
    let mut cmd = std::process::Command::new(editor_parts[0]);
    if editor_parts.len() > 1 {
        cmd.args(&editor_parts[1..]);
    }
    cmd.arg(format!("+{actual_line}"));
    cmd.arg(&path);

    let status = cmd.status();
    match status {
        Ok(s) if s.success() => {}
        Ok(s) => eprintln!("{} exited with error: {s}", editor_parts[0]),
        Err(e) => eprintln!("Failed to run {}: {e}", editor_parts[0]),
    }

    Ok(())
}

pub fn run(config: &Config, _ctx: &OutputContext, opts: &OpenOptions) -> Result<()> {
    let graph = Graph::load(config)?;

    for target in &opts.targets {
        open_target(&graph, config, target, &opts.editor, opts.line)?;
    }

    Ok(())
}
