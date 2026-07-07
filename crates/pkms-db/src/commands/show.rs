use anyhow::Result;
use pkms_org::OrgConfig;
use pkms_org::graph::{FileScanResult, Graph, Node};
use pkms_org::org_edit::parsed_heading_subtree_end_index;
use pkms_org::parser::{Heading, Link, strip_org_links};
use serde::Serialize;
use std::collections::HashMap;
use std::fmt::Write;
use std::path::{Path, PathBuf};

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
    pub task_ids: Vec<TaskIdEntry>,
}

pub enum HeadingTarget {
    Note(String),
    Location { path: PathBuf, line_number: usize },
}

pub struct TaskIdEntry {
    pub id: usize,
    pub path: String,
    pub line_number: usize,
}

type TaskIdMap = HashMap<(String, usize), usize>;

fn find_parents(
    headings: &[pkms_org::parser::Heading],
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
    headings: &[pkms_org::parser::Heading],
    target_idx: usize,
    end_line: usize,
    path: &std::path::Path,
    task_ids: &TaskIdMap,
) -> Vec<RelatedTaskHeading> {
    let target_level = headings[target_idx].level;
    let mut children = Vec::new();
    for h in headings.iter().skip(target_idx + 1) {
        if h.line_number > end_line {
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

fn related_task_heading(id: usize, heading: &pkms_org::parser::Heading) -> RelatedTaskHeading {
    RelatedTaskHeading {
        id,
        title: heading.title.clone(),
        todo_state: heading.todo_state.as_ref().map(|state| state.to_string()),
        priority: heading.priority.map(|priority| priority.as_char()),
        line_number: heading.line_number,
        level: heading.level,
    }
}

fn extract_outgoing(links: &[Link]) -> Vec<OutgoingLink> {
    links
        .iter()
        .map(|link| {
            let (link_type, target) = match link {
                Link::Internal(u) => ("id".to_string(), u.to_string()),
                Link::File(f) => ("file".to_string(), f.to_string()),
                Link::Url(u) => ("url".to_string(), u.to_string()),
                Link::Attachment(a) => ("attachment".to_string(), a.to_string()),
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
    headings: &'a [Heading],
    filetags: &'a [String],
    note_title: &'a str,
    note_uuid: &'a str,
    path: &'a Path,
    task_ids: &'a TaskIdMap,
}

struct ResolvedHeadingTarget<'a> {
    content: String,
    headings: &'a [Heading],
    filetags: &'a [String],
    note_title: String,
    note_uuid: String,
    path: PathBuf,
    line_number: usize,
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

    let content_lines: Vec<&str> = ctx.content.lines().collect();
    let end_idx = parsed_heading_subtree_end_index(ctx.headings, heading, content_lines.len())
        .min(content_lines.len());
    let end_line = end_idx;
    let parents = find_parents(ctx.headings, heading_idx, ctx.path, ctx.task_ids);
    let children = find_children(ctx.headings, heading_idx, end_line, ctx.path, ctx.task_ids);

    let block_content = if heading.line_number <= content_lines.len() {
        let start = heading.line_number - 1;
        if end_idx > start {
            content_lines[start..end_idx].join("\n")
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
        todo_state: heading.todo_state.as_ref().map(|state| state.to_string()),
        priority: heading.priority.map(|priority| priority.as_char()),
        tags: all_tags,
        filetags: ctx.filetags.to_vec(),
        scheduled: heading.scheduled.clone(),
        deadline: heading.deadline.clone(),
        path: ctx.path.display().to_string(),
        note_title: ctx.note_title.to_string(),
        note_uuid: ctx.note_uuid.to_string(),
        heading_uuid: heading.uuid.as_ref().map(|uuid| uuid.to_string()),
        parents,
        children,
        outgoing,
        content: block_content,
    })
}

fn process_one_show(
    graph: &Graph,
    target: &HeadingTarget,
    task_ids: &TaskIdMap,
) -> Result<ShowOutput> {
    let resolved = resolve_heading_target(graph, target)?;

    show_heading_by_line(
        HeadingShowContext {
            content: &resolved.content,
            headings: resolved.headings,
            filetags: resolved.filetags,
            note_title: &resolved.note_title,
            note_uuid: &resolved.note_uuid,
            path: &resolved.path,
            task_ids,
        },
        resolved.line_number,
    )
}

fn resolve_heading_target<'a>(
    graph: &'a Graph,
    target: &HeadingTarget,
) -> Result<ResolvedHeadingTarget<'a>> {
    match target {
        HeadingTarget::Location { path, line_number } => {
            resolve_location_heading_target(graph, path, *line_number)
        }
        HeadingTarget::Note(note_target) => resolve_note_heading_target(graph, note_target),
    }
}

fn resolve_location_heading_target<'a>(
    graph: &'a Graph,
    path: &Path,
    line_number: usize,
) -> Result<ResolvedHeadingTarget<'a>> {
    let content = std::fs::read_to_string(path)?;

    let result = graph.results.iter().find(|r| r.path.as_path() == path);
    let parsed = match result {
        Some(r) => &r.parsed,
        None => anyhow::bail!("No parsed data for path: {}", path.display()),
    };

    Ok(ResolvedHeadingTarget {
        content,
        headings: &parsed.headings,
        filetags: &parsed.filetags,
        note_title: note_title_from_parsed(path, parsed.title.as_deref()),
        note_uuid: parsed
            .uuids
            .first()
            .map(ToString::to_string)
            .unwrap_or_default(),
        path: path.to_path_buf(),
        line_number,
    })
}

fn resolve_note_heading_target<'a>(
    graph: &'a Graph,
    note_target: &str,
) -> Result<ResolvedHeadingTarget<'a>> {
    if let Ok(node) = graph.resolve_target(note_target) {
        return resolve_node_heading_target(graph, node);
    }

    resolve_fallback_heading_target(graph, note_target)
}

fn resolve_node_heading_target<'a>(
    graph: &'a Graph,
    node: &'a Node,
) -> Result<ResolvedHeadingTarget<'a>> {
    let parsed = graph
        .results
        .iter()
        .find(|r| r.path == node.path)
        .map(|r| &r.parsed);
    let headings = parsed.map(|p| p.headings.as_slice()).unwrap_or(&[]);
    let content = std::fs::read_to_string(&node.path)?;
    let note_title = strip_org_links(&node.title);
    let line_number = first_todo_line(headings, &note_title)?;

    Ok(ResolvedHeadingTarget {
        content,
        headings,
        filetags: &node.filetags,
        note_title,
        note_uuid: node.uuid.to_string(),
        path: node.path.clone(),
        line_number,
    })
}

fn resolve_fallback_heading_target<'a>(
    graph: &'a Graph,
    note_target: &str,
) -> Result<ResolvedHeadingTarget<'a>> {
    let target_lower = note_target.to_lowercase();
    let mut found = None;

    for result in &graph.results {
        if !fallback_result_matches(result, &target_lower) {
            continue;
        }

        let content = std::fs::read_to_string(&result.path)?;
        let headings = &result.parsed.headings;
        let note_title = note_title_from_parsed(&result.path, result.parsed.title.as_deref());
        let line_number = match first_todo_line(headings, &note_title) {
            Ok(line_number) => line_number,
            Err(e) => {
                found = Some(e);
                continue;
            }
        };

        return Ok(ResolvedHeadingTarget {
            content,
            headings,
            filetags: &result.parsed.filetags,
            note_title,
            note_uuid: String::new(),
            path: result.path.clone(),
            line_number,
        });
    }

    match found {
        Some(e) => Err(e),
        None => anyhow::bail!("Note not found: {}", note_target),
    }
}

fn fallback_result_matches(result: &FileScanResult, target_lower: &str) -> bool {
    let path_str = result.path.to_string_lossy().to_lowercase();
    path_str.contains(target_lower)
        || result
            .path
            .file_stem()
            .is_some_and(|s| s.to_string_lossy().to_lowercase() == target_lower)
        || result
            .parsed
            .title
            .as_ref()
            .is_some_and(|t| t.to_lowercase() == target_lower)
}

fn note_title_from_parsed(path: &Path, title: Option<&str>) -> String {
    let raw_title = title.map(str::to_string).unwrap_or_else(|| {
        path.file_stem()
            .map(|s| s.display().to_string())
            .unwrap_or_default()
    });
    strip_org_links(&raw_title)
}

fn first_todo_line(headings: &[Heading], note_title: &str) -> Result<usize> {
    headings
        .iter()
        .find(|h| h.todo_state.is_some())
        .map(|h| h.line_number)
        .ok_or_else(|| {
            anyhow::anyhow!(
                "No TODO heading found in note '{}'. Use a numeric canonical ID from task list or task agenda, or --uuid to specify an exact note.",
                note_title
            )
        })
}

fn resolve_outgoing_titles(output: &mut ShowOutput, graph: &Graph) {
    for link in &mut output.outgoing {
        if link.link_type == "id" {
            link.resolved_title = graph.find_node(&link.target).map(|node| node.title.clone());
        }
    }
}

pub fn execute(org_config: &OrgConfig, opts: &ShowOptions) -> Result<Vec<ShowOutput>> {
    let graph = Graph::load(org_config)?;
    let task_ids: TaskIdMap = opts
        .task_ids
        .iter()
        .map(|entry| ((entry.path.clone(), entry.line_number), entry.id))
        .collect();
    opts.targets
        .iter()
        .map(|target| {
            let mut output = process_one_show(&graph, target, &task_ids)?;
            resolve_outgoing_titles(&mut output, &graph);
            Ok(output)
        })
        .collect()
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
