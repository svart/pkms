use crate::cli::OutputFormat;
use crate::config::Config;
use crate::graph::Graph;
use crate::output::OutputContext;
use crate::parser::{Link, strip_org_links};
use anyhow::Result;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct TaskParent {
    pub title: String,
    pub todo_state: Option<String>,
    pub priority: Option<char>,
    pub line_number: usize,
    pub level: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct TaskChild {
    pub title: String,
    pub todo_state: Option<String>,
    pub priority: Option<char>,
    pub line_number: usize,
    pub level: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct OutgoingLink {
    pub link_type: String,
    pub target: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ShowOutput {
    pub heading_title: String,
    pub line_number: usize,
    pub end_line: usize,
    pub todo_state: Option<String>,
    pub priority: Option<char>,
    pub tags: Vec<String>,
    pub filetags: Vec<String>,
    pub scheduled: Option<String>,
    pub deadline: Option<String>,
    pub path: String,
    pub note_title: String,
    pub note_uuid: String,
    pub heading_uuid: Option<String>,
    pub parents: Vec<TaskParent>,
    pub children: Vec<TaskChild>,
    pub outgoing: Vec<OutgoingLink>,
    pub content: String,
}

pub struct ShowOptions {
    pub targets: Vec<HeadingTarget>,
}

pub struct HeadingTarget {
    pub note_target: String,
    pub canonical_id: Option<usize>,
}

impl HeadingTarget {
    pub fn from_arg(arg: String) -> Result<Self> {
        if let Ok(id) = arg.parse::<usize>() {
            Ok(HeadingTarget {
                note_target: arg,
                canonical_id: Some(id),
            })
        } else {
            Ok(HeadingTarget {
                note_target: arg,
                canonical_id: None,
            })
        }
    }
}

fn find_heading_end(content: &str, heading_line: usize) -> usize {
    let lines: Vec<&str> = content.lines().collect();
    if heading_line == 0 || heading_line > lines.len() {
        return lines.len();
    }
    let heading_text = lines[heading_line - 1];
    let level = heading_text.chars().take_while(|c| *c == '*').count();
    if level == 0 {
        return lines.len();
    }
    for (i, line) in lines.iter().enumerate().skip(heading_line) {
        if line.starts_with('*') {
            let l = line.chars().take_while(|c| *c == '*').count();
            if l <= level {
                return i + 1;
            }
        }
    }
    lines.len()
}

fn find_parents(headings: &[crate::parser::Heading], target_idx: usize) -> Vec<TaskParent> {
    let target_level = headings[target_idx].level;
    let mut parents = Vec::new();
    let mut seen_levels: Vec<usize> = Vec::new();
    for h in headings[..target_idx].iter().rev() {
        if h.level >= target_level {
            continue;
        }
        if seen_levels.contains(&h.level) {
            continue;
        }
        seen_levels.push(h.level);
        if h.todo_state.is_some() {
            parents.push(TaskParent {
                title: h.title.clone(),
                todo_state: h.todo_state.clone(),
                priority: h.priority,
                line_number: h.line_number,
                level: h.level,
            });
        }
    }
    parents.reverse();
    parents
}

fn find_children(
    headings: &[crate::parser::Heading],
    target_idx: usize,
    end_line: usize,
) -> Vec<TaskChild> {
    let target_level = headings[target_idx].level;
    let mut children = Vec::new();
    for h in headings.iter().skip(target_idx + 1) {
        if h.line_number >= end_line {
            break;
        }
        if h.level > target_level {
            children.push(TaskChild {
                title: h.title.clone(),
                todo_state: h.todo_state.clone(),
                priority: h.priority,
                line_number: h.line_number,
                level: h.level,
            });
        }
    }
    children
}

fn extract_outgoing(links: &[Link]) -> Vec<OutgoingLink> {
    links
        .iter()
        .map(|link| {
            let (link_type, target) = match link {
                Link::Internal(u) => ("id".to_string(), u.clone()),
                Link::File(f) => ("file".to_string(), f.clone()),
                Link::Url(u) => ("url".to_string(), u.clone()),
                Link::Attachment(a) => ("attachment".to_string(), a.clone()),
            };
            OutgoingLink {
                link_type,
                target,
                description: None,
            }
        })
        .collect()
}

fn show_heading_by_line(
    content: &str,
    headings: &[crate::parser::Heading],
    filetags: &[String],
    note_title: &str,
    note_uuid: &str,
    path: &std::path::Path,
    line_number: usize,
) -> Result<ShowOutput> {
    let heading_idx = headings
        .iter()
        .position(|h| h.line_number == line_number)
        .ok_or_else(|| {
            anyhow::anyhow!(
                "No heading found at line {} in '{}'",
                line_number,
                note_title
            )
        })?;
    let heading = &headings[heading_idx];

    let end_line = find_heading_end(content, heading.line_number);
    let parents = find_parents(headings, heading_idx);
    let children = find_children(headings, heading_idx, end_line);

    let content_lines: Vec<&str> = content.lines().collect();
    let block_content = if heading.line_number <= content_lines.len() {
        let start = heading.line_number - 1;
        let end = end_line - 1;
        if end > start {
            content_lines[start..end].join("\n")
        } else {
            String::new()
        }
    } else {
        String::new()
    };

    let all_tags: Vec<String> = {
        let mut seen = std::collections::HashSet::new();
        let mut result = Vec::new();
        for tag in filetags.iter().chain(heading.tags.iter()) {
            if seen.insert(tag.clone()) {
                result.push(tag.clone());
            }
        }
        result
    };

    let outgoing = extract_outgoing(&heading.outgoing);

    Ok(ShowOutput {
        heading_title: heading.title.clone(),
        line_number: heading.line_number,
        end_line,
        todo_state: heading.todo_state.clone(),
        priority: heading.priority,
        tags: all_tags,
        filetags: filetags.to_vec(),
        scheduled: heading.scheduled.clone(),
        deadline: heading.deadline.clone(),
        path: path.display().to_string(),
        note_title: note_title.to_string(),
        note_uuid: note_uuid.to_string(),
        heading_uuid: heading.uuid.clone(),
        parents,
        children,
        outgoing,
        content: block_content,
    })
}

fn process_one_show(graph: &Graph, config: &Config, target: &HeadingTarget) -> Result<ShowOutput> {
    let (path, line_number) = if let Some(cid) = target.canonical_id {
        graph.resolve_canonical_task_id(config, cid)?
    } else {
        // Try graph node lookup first
        if let Ok(node) = graph.resolve_target(&target.note_target) {
            let parsed = graph
                .results
                .iter()
                .find(|r| r.path == node.path)
                .map(|r| &r.parsed);

            let headings: &[crate::parser::Heading] = match parsed {
                Some(p) => &p.headings,
                None => &[],
            };

            let content = std::fs::read_to_string(&node.path)?;
            let note_title = strip_org_links(&node.title);

            let first_todo = headings.iter().find(|h| h.todo_state.is_some());
            match first_todo {
                Some(h) => {
                    return show_heading_by_line(
                        &content,
                        headings,
                        &node.filetags,
                        &note_title,
                        &node.uuid,
                        &node.path,
                        h.line_number,
                    );
                }
                None => {
                    anyhow::bail!(
                        "No TODO heading found in note '{}'. Use a numeric canonical ID (from todo/agenda) or --uuid to specify an exact note.",
                        note_title
                    );
                }
            }
        }

        // Fallback: search in results for files without UUIDs
        let mut found = None;
        for result in &graph.results {
            let path_str = result.path.to_string_lossy().to_lowercase();
            let target_lower = target.note_target.to_lowercase();
            if path_str.contains(&target_lower)
                || result
                    .path
                    .file_stem()
                    .is_some_and(|s| s.to_string_lossy().to_lowercase() == target_lower)
                || result
                    .parsed
                    .title
                    .as_ref()
                    .is_some_and(|t| t.to_lowercase() == target_lower)
            {
                let content = std::fs::read_to_string(&result.path)?;
                let headings = &result.parsed.headings;
                let note_title =
                    strip_org_links(&result.parsed.title.clone().unwrap_or_else(|| {
                        result
                            .path
                            .file_stem()
                            .map(|s| s.display().to_string())
                            .unwrap_or_default()
                    }));
                let first_todo = headings.iter().find(|h| h.todo_state.is_some());
                match first_todo {
                    Some(h) => {
                        return show_heading_by_line(
                            &content,
                            headings,
                            &result.parsed.filetags,
                            &note_title,
                            "",
                            &result.path,
                            h.line_number,
                        );
                    }
                    None => {
                        found = Some(anyhow::anyhow!(
                            "No TODO heading found in note '{}'. Use a numeric canonical ID (from todo/agenda) or --uuid to specify an exact note.",
                            note_title
                        ));
                    }
                }
            }
        }
        match found {
            Some(e) => return Err(e),
            None => anyhow::bail!("Note not found: {}", target.note_target),
        }
    };

    // Show heading by resolved path + line_number
    let content = std::fs::read_to_string(&path)?;
    let path_ref = std::path::Path::new(&path);

    let result = graph.results.iter().find(|r| r.path == *path_ref);
    let parsed = match result {
        Some(r) => &r.parsed,
        None => anyhow::bail!("No parsed data for path: {}", path),
    };

    let note_title = strip_org_links(&parsed.title.clone().unwrap_or_else(|| {
        path_ref
            .file_stem()
            .map(|s| s.display().to_string())
            .unwrap_or_default()
    }));

    let note_uuid = parsed.uuids.first().cloned().unwrap_or_default();

    show_heading_by_line(
        &content,
        &parsed.headings,
        &parsed.filetags,
        &note_title,
        &note_uuid,
        path_ref,
        line_number,
    )
}

pub fn read_stdin_targets() -> Result<Vec<HeadingTarget>> {
    let values = crate::util::read_stdin_ndjson_raw()?;
    let mut targets = Vec::new();
    for value in &values {
        let uuid = value
            .get("uuid")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let path = value
            .get("path")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let target_str = uuid.or(path).unwrap_or_default();
        if !target_str.is_empty() {
            targets.push(HeadingTarget::from_arg(target_str)?);
        }
    }
    if targets.is_empty() {
        anyhow::bail!("No valid NDJSON lines with 'uuid' or 'path' field found on stdin");
    }
    Ok(targets)
}

fn print_one_show_text(output: &ShowOutput, graph: &Graph) {
    println!("Task:       {}", output.heading_title);
    println!("  File:     {}", output.path);
    println!("  Lines:    {} – {}", output.line_number, output.end_line);
    if let Some(ref state) = output.todo_state {
        println!("  State:    {}", state);
    }
    if let Some(p) = output.priority {
        println!("  Priority: [#{}]", p);
    }
    if !output.tags.is_empty() {
        println!("  Tags:     {}", output.tags.join(", "));
    }
    if let Some(ref s) = output.scheduled {
        println!("  Scheduled: {}", s);
    }
    if let Some(ref d) = output.deadline {
        println!("  Deadline:  {}", d);
    }
    println!("  Note:     {}", output.note_title);
    println!("  Note UUID: {}", output.note_uuid);
    if let Some(ref huuid) = output.heading_uuid {
        println!("  Heading UUID: {}", huuid);
    }

    if !output.parents.is_empty() {
        println!();
        println!("Parent chain (depends on):");
        for p in &output.parents {
            let state_display = p.todo_state.as_deref().unwrap_or("");
            let prio_display = p.priority.map(|c| format!(" [#{}]", c)).unwrap_or_default();
            println!(
                "  {} {}{} (line {}, level {})",
                state_display, p.title, prio_display, p.line_number, p.level
            );
        }
    }

    if !output.children.is_empty() {
        println!();
        println!("Subtasks (blocks):");
        for c in &output.children {
            let state_display = c.todo_state.as_deref().unwrap_or("");
            let prio_display = c.priority.map(|c| format!(" [#{}]", c)).unwrap_or_default();
            println!(
                "  {} {}{} (line {}, level {})",
                state_display, c.title, prio_display, c.line_number, c.level
            );
        }
    }

    if !output.outgoing.is_empty() {
        println!();
        println!("Outgoing links:");
        for link in &output.outgoing {
            let resolved = if link.link_type == "id" {
                graph
                    .find_node(&link.target)
                    .map(|n| format!(" → {}", n.title))
                    .unwrap_or_default()
            } else {
                String::new()
            };
            println!("  {}:{}{}", link.link_type, link.target, resolved);
        }
    }

    if !output.content.is_empty() {
        println!();
        println!("--- Content ---");
        println!("{}", output.content);
        println!("--- End Content ---");
    }
}

pub fn run(config: &Config, ctx: &OutputContext, opts: &ShowOptions) -> Result<()> {
    let graph = Graph::load(config)?;

    match ctx.format {
        OutputFormat::Text => {
            for target in &opts.targets {
                let output = process_one_show(&graph, config, target)?;
                print_one_show_text(&output, &graph);
                if opts.targets.len() > 1 {
                    println!();
                }
            }
        }
        OutputFormat::Json => {
            let mut all_outputs = Vec::new();
            for target in &opts.targets {
                all_outputs.push(process_one_show(&graph, config, target)?);
            }
            if all_outputs.len() == 1 {
                ctx.print_json(&all_outputs[0])?;
            } else {
                ctx.print_json(&all_outputs)?;
            }
        }
        OutputFormat::Ndjson => {
            for target in &opts.targets {
                let output = process_one_show(&graph, config, target)?;
                println!("{}", serde_json::to_string(&output)?);
            }
        }
    }

    Ok(())
}
