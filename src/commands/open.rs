use crate::config::Config;
use crate::graph::Graph;
use crate::output::OutputContext;
use crate::parser::find_daily_file_date;
use anyhow::Result;

pub struct OpenOptions {
    pub targets: Vec<String>,
    pub editor: String,
    pub line: Option<usize>,
}

fn priority_value(p: Option<char>) -> u8 {
    match p {
        Some('A') => 0,
        Some('B') => 1,
        Some('C') => 2,
        _ => 3,
    }
}

fn resolve_agenda_id(graph: &Graph, config: &Config, id: usize) -> Result<(String, usize)> {
    let valid_states = config.todo_states();
    let closed_states = config.closed_todo_states();

    let mut items: Vec<(String, usize, Option<char>)> = Vec::new();

    for result in &graph.results {
        if result.parse_error.is_some() {
            continue;
        }
        let is_daily = find_daily_file_date(&result.path).is_some();

        for heading in &result.parsed.headings {
            let has_sched = heading.scheduled.is_some();
            let has_deadline = heading.deadline.is_some();
            let is_todo = heading
                .todo_state
                .as_ref()
                .is_some_and(|s| valid_states.iter().any(|vs| vs.eq_ignore_ascii_case(s)));

            let eligible = has_sched || has_deadline || (is_daily && is_todo);

            if !eligible {
                continue;
            }

            if !closed_states.is_empty()
                && let Some(ref todo_state) = heading.todo_state
                && closed_states
                    .iter()
                    .any(|cs| cs.eq_ignore_ascii_case(todo_state))
            {
                continue;
            }

            items.push((
                result.path.to_string_lossy().to_string(),
                heading.line_number,
                heading.priority,
            ));
        }
    }

    items.sort_by(|a, b| {
        let a_p = priority_value(a.2);
        let b_p = priority_value(b.2);
        a_p.cmp(&b_p).then(a.0.cmp(&b.0)).then(a.1.cmp(&b.1))
    });

    if id == 0 || id > items.len() {
        anyhow::bail!("No task with ID {}. Valid range is 1–{}", id, items.len());
    }

    let (path, line, _) = &items[id - 1];
    Ok((path.clone(), *line))
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
        resolve_agenda_id(graph, config, id)?
    } else {
        let node = graph.resolve_target(target)?;
        let path = node.path.to_string_lossy().to_string();
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
                .map(|s| s.to_string_lossy().to_string())
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
