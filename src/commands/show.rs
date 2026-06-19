use crate::cli::OutputFormat;
use crate::config::ResolvedConfig;
use crate::graph::Graph;
use crate::output::{OutputContext, terminal_markup};
use crate::parser::{Link, strip_org_links};
use anyhow::Result;
use serde::Serialize;
use std::collections::HashMap;
use std::fmt::Write;

#[derive(Debug, Clone, Serialize)]
pub struct RelatedTaskHeading {
    pub id: usize,
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
    #[serde(skip)]
    pub resolved_title: Option<String>,
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
    pub parents: Vec<RelatedTaskHeading>,
    pub children: Vec<RelatedTaskHeading>,
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

type TaskIdMap = HashMap<(String, usize), usize>;

fn find_parents(
    headings: &[crate::parser::Heading],
    target_idx: usize,
    path: &std::path::Path,
    task_ids: &TaskIdMap,
) -> Vec<RelatedTaskHeading> {
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
        if let Some(id) = canonical_task_id(task_ids, path, h.line_number) {
            parents.push(related_task_heading(id, h));
        }
    }
    parents.reverse();
    parents
}

fn find_children(
    headings: &[crate::parser::Heading],
    target_idx: usize,
    end_line: usize,
    path: &std::path::Path,
    task_ids: &TaskIdMap,
) -> Vec<RelatedTaskHeading> {
    let target_level = headings[target_idx].level;
    let mut children = Vec::new();
    for h in headings.iter().skip(target_idx + 1) {
        if h.line_number >= end_line {
            break;
        }
        if h.level > target_level
            && let Some(id) = canonical_task_id(task_ids, path, h.line_number)
        {
            children.push(related_task_heading(id, h));
        }
    }
    children
}

fn canonical_task_id(
    task_ids: &TaskIdMap,
    path: &std::path::Path,
    line_number: usize,
) -> Option<usize> {
    task_ids
        .get(&(path.display().to_string(), line_number))
        .copied()
}

fn related_task_heading(id: usize, heading: &crate::parser::Heading) -> RelatedTaskHeading {
    RelatedTaskHeading {
        id,
        title: heading.title.clone(),
        todo_state: heading.todo_state.clone(),
        priority: heading.priority,
        line_number: heading.line_number,
        level: heading.level,
    }
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
                resolved_title: None,
            }
        })
        .collect()
}

struct HeadingShowContext<'a> {
    content: &'a str,
    headings: &'a [crate::parser::Heading],
    filetags: &'a [String],
    note_title: &'a str,
    note_uuid: &'a str,
    path: &'a std::path::Path,
    task_ids: &'a TaskIdMap,
}

fn show_heading_by_line(ctx: HeadingShowContext<'_>, line_number: usize) -> Result<ShowOutput> {
    let heading_idx = ctx
        .headings
        .iter()
        .position(|h| h.line_number == line_number)
        .ok_or_else(|| {
            anyhow::anyhow!(
                "No heading found at line {} in '{}'",
                line_number,
                ctx.note_title
            )
        })?;
    let heading = &ctx.headings[heading_idx];

    let end_line = find_heading_end(ctx.content, heading.line_number);
    let parents = find_parents(ctx.headings, heading_idx, ctx.path, ctx.task_ids);
    let children = find_children(ctx.headings, heading_idx, end_line, ctx.path, ctx.task_ids);

    let content_lines: Vec<&str> = ctx.content.lines().collect();
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
        for tag in ctx.filetags.iter().chain(heading.tags.iter()) {
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
        filetags: ctx.filetags.to_vec(),
        scheduled: heading.scheduled.clone(),
        deadline: heading.deadline.clone(),
        path: ctx.path.display().to_string(),
        note_title: ctx.note_title.to_string(),
        note_uuid: ctx.note_uuid.to_string(),
        heading_uuid: heading.uuid.clone(),
        parents,
        children,
        outgoing,
        content: block_content,
    })
}

fn process_one_show(
    graph: &Graph,
    config: &ResolvedConfig,
    target: &HeadingTarget,
    task_ids: &TaskIdMap,
) -> Result<ShowOutput> {
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
                        HeadingShowContext {
                            content: &content,
                            headings,
                            filetags: &node.filetags,
                            note_title: &note_title,
                            note_uuid: &node.uuid,
                            path: &node.path,
                            task_ids,
                        },
                        h.line_number,
                    );
                }
                None => {
                    anyhow::bail!(
                        "No TODO heading found in note '{}'. Use a numeric canonical ID from task list or task agenda, or --uuid to specify an exact note.",
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
                            HeadingShowContext {
                                content: &content,
                                headings,
                                filetags: &result.parsed.filetags,
                                note_title: &note_title,
                                note_uuid: "",
                                path: &result.path,
                                task_ids,
                            },
                            h.line_number,
                        );
                    }
                    None => {
                        found = Some(anyhow::anyhow!(
                            "No TODO heading found in note '{}'. Use a numeric canonical ID from task list or task agenda, or --uuid to specify an exact note.",
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
        HeadingShowContext {
            content: &content,
            headings: &parsed.headings,
            filetags: &parsed.filetags,
            note_title: &note_title,
            note_uuid: &note_uuid,
            path: path_ref,
            task_ids,
        },
        line_number,
    )
}

fn resolve_outgoing_titles(output: &mut ShowOutput, graph: &Graph) {
    for link in &mut output.outgoing {
        if link.link_type == "id" {
            link.resolved_title = graph.find_node(&link.target).map(|node| node.title.clone());
        }
    }
}

pub fn execute(config: &ResolvedConfig, opts: &ShowOptions) -> Result<Vec<ShowOutput>> {
    let graph = Graph::load(config)?;
    let task_ids: TaskIdMap = graph
        .all_task_entries(config)
        .into_iter()
        .map(|(id, path, line_number)| ((path, line_number), id))
        .collect();
    opts.targets
        .iter()
        .map(|target| {
            let mut output = process_one_show(&graph, config, target, &task_ids)?;
            resolve_outgoing_titles(&mut output, &graph);
            Ok(output)
        })
        .collect()
}

pub fn render(ctx: &OutputContext, outputs: &[ShowOutput]) -> Result<()> {
    match ctx.format {
        OutputFormat::Text => {
            let text = render_text(outputs);
            print!("{}", terminal_markup::format_if_terminal_supported(&text));
        }
        OutputFormat::Json => {
            ctx.print_json_adaptive(outputs)?;
        }
        OutputFormat::Ndjson => {
            ctx.print_ndjson(outputs)?;
        }
    }

    Ok(())
}

pub fn render_text(outputs: &[ShowOutput]) -> String {
    outputs
        .iter()
        .map(render_one_text)
        .collect::<Vec<_>>()
        .join("\n")
}

fn render_one_text(output: &ShowOutput) -> String {
    let mut text = String::new();

    let _ = writeln!(text, "Task:       {}", output.heading_title);
    let _ = writeln!(text, "  File:     {}", output.path);
    let _ = writeln!(
        text,
        "  Lines:    {} – {}",
        output.line_number, output.end_line
    );
    if let Some(ref state) = output.todo_state {
        let _ = writeln!(text, "  State:    {}", state);
    }
    if let Some(p) = output.priority {
        let _ = writeln!(text, "  Priority: [#{}]", p);
    }
    if !output.tags.is_empty() {
        let _ = writeln!(text, "  Tags:     {}", output.tags.join(", "));
    }
    if let Some(ref s) = output.scheduled {
        let _ = writeln!(text, "  Scheduled: {}", s);
    }
    if let Some(ref d) = output.deadline {
        let _ = writeln!(text, "  Deadline:  {}", d);
    }
    let _ = writeln!(text, "  Note:     {}", output.note_title);
    let _ = writeln!(text, "  Note UUID: {}", output.note_uuid);
    if let Some(ref huuid) = output.heading_uuid {
        let _ = writeln!(text, "  Heading UUID: {}", huuid);
    }

    if !output.parents.is_empty() {
        text.push('\n');
        text.push_str("Parent chain (depends on):\n");
        for p in &output.parents {
            let state_display = p.todo_state.as_deref().unwrap_or("");
            let prio_display = p.priority.map(|c| format!(" [#{}]", c)).unwrap_or_default();
            let _ = writeln!(
                text,
                "  p{} {} {}{} (line {}, level {})",
                p.id, state_display, p.title, prio_display, p.line_number, p.level
            );
        }
    }

    if !output.children.is_empty() {
        text.push('\n');
        text.push_str("Child chain (blocks):\n");
        for c in &output.children {
            let state_display = c.todo_state.as_deref().unwrap_or("");
            let prio_display = c.priority.map(|c| format!(" [#{}]", c)).unwrap_or_default();
            let _ = writeln!(
                text,
                "  p{} {} {}{} (line {}, level {})",
                c.id, state_display, c.title, prio_display, c.line_number, c.level
            );
        }
    }

    if !output.outgoing.is_empty() {
        text.push('\n');
        text.push_str("Outgoing links:\n");
        for link in &output.outgoing {
            let resolved = link
                .resolved_title
                .as_ref()
                .map(|title| format!(" → {title}"))
                .unwrap_or_default();
            let _ = writeln!(text, "  {}:{}{}", link.link_type, link.target, resolved);
        }
    }

    if !output.content.is_empty() {
        text.push('\n');
        text.push_str("--- Content ---\n");
        let _ = writeln!(text, "{}", output.content);
        text.push_str("--- End Content ---\n");
    }

    text
}

pub fn run(config: &ResolvedConfig, ctx: &OutputContext, opts: &ShowOptions) -> Result<()> {
    let outputs = execute(config, opts)?;
    render(ctx, &outputs)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn output() -> ShowOutput {
        ShowOutput {
            heading_title: "Task heading".to_string(),
            line_number: 10,
            end_line: 14,
            todo_state: Some("TODO".to_string()),
            priority: Some('A'),
            tags: vec!["work".to_string()],
            filetags: vec!["project".to_string()],
            scheduled: Some("<2026-05-28 Thu>".to_string()),
            deadline: None,
            path: "/notes/task.org".to_string(),
            note_title: "Task Note".to_string(),
            note_uuid: "11111111-1111-4111-8111-111111111111".to_string(),
            heading_uuid: Some("22222222-2222-4222-8222-222222222222".to_string()),
            parents: vec![RelatedTaskHeading {
                id: 1,
                title: "Parent".to_string(),
                todo_state: Some("TODO".to_string()),
                priority: None,
                line_number: 5,
                level: 1,
            }],
            children: vec![RelatedTaskHeading {
                id: 3,
                title: "Child".to_string(),
                todo_state: Some("NEXT".to_string()),
                priority: Some('B'),
                line_number: 12,
                level: 3,
            }],
            outgoing: vec![OutgoingLink {
                link_type: "id".to_string(),
                target: "33333333-3333-4333-8333-333333333333".to_string(),
                description: None,
                resolved_title: Some("Linked Note".to_string()),
            }],
            content: "* TODO Task heading\nBody".to_string(),
        }
    }

    #[test]
    fn renders_show_text_from_typed_output() {
        let text = render_text(&[output()]);

        assert!(text.contains("Task:       Task heading"));
        assert!(text.contains("  Lines:    10 – 14"));
        assert!(text.contains("  Priority: [#A]"));
        assert!(text.contains("Parent chain (depends on):"));
        assert!(text.contains("p1 TODO Parent (line 5, level 1)"));
        assert!(text.contains("Child chain (blocks):"));
        assert!(text.contains("p3 NEXT Child [#B] (line 12, level 3)"));
        assert!(text.contains("id:33333333-3333-4333-8333-333333333333 → Linked Note"));
        assert!(text.contains("--- Content ---\n* TODO Task heading\nBody\n--- End Content ---"));
    }
}
