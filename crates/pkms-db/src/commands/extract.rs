use crate::commands::new::{create_note_file_exclusive, title_to_slug, unique_note_filename};
use anyhow::{Context, Result};
use pkms_org::OrgConfig;
use pkms_org::graph::{Graph, HeadingLocation};
use pkms_org::org_edit::{
    is_heading_line, parsed_heading_subtree_end_index, read_lines, split_line_ending, write_lines,
};
use pkms_org::parser::{Heading, ID_PROPERTY_RE};
use serde::Serialize;
use std::fmt::Write;
use std::path::{Path, PathBuf};

pub struct ExtractOptions {
    pub heading_uuid: String,
    pub new_name: Option<String>,
    pub apply: bool,
}

#[derive(Serialize)]
pub struct ExtractOutput {
    pub uuid: String,
    pub title: String,
    pub source_path: PathBuf,
    pub new_path: PathBuf,
    pub replacement: String,
    pub created: bool,
    pub applied: bool,
}

pub fn execute(config: &OrgConfig, opts: &ExtractOptions) -> Result<ExtractOutput> {
    let graph = Graph::load(config)?;
    let location = resolve_heading_location(&graph, &opts.heading_uuid)?;
    let source_node = graph.nodes.get(&location.primary_uuid).ok_or_else(|| {
        anyhow::anyhow!("Source note not found for heading {}", opts.heading_uuid)
    })?;
    let source_path = source_node.path.clone();
    let source_result = graph
        .results
        .iter()
        .find(|result| result.path == source_path)
        .ok_or_else(|| {
            anyhow::anyhow!("Parsed source note not found: {}", source_path.display())
        })?;
    let heading = source_result
        .parsed
        .headings
        .iter()
        .find(|heading| {
            heading.line_number == location.line_number
                && heading.uuid.as_deref() == Some(opts.heading_uuid.as_str())
        })
        .ok_or_else(|| anyhow::anyhow!("Heading not found in {}", source_path.display()))?;

    let source_lines = read_lines(&source_path)
        .with_context(|| format!("Failed to read {}", source_path.display()))?;
    let start_idx = heading
        .line_number
        .checked_sub(1)
        .ok_or_else(|| anyhow::anyhow!("Invalid heading line number: {}", heading.line_number))?;
    ensure_heading_line(&source_lines, start_idx, heading.line_number)?;
    let end_idx = parsed_heading_subtree_end_index(
        &source_result.parsed.headings,
        heading,
        source_lines.len(),
    );
    let subtree = source_lines[start_idx..end_idx].to_vec();
    let copied_subtree = remove_root_heading_id(subtree, &opts.heading_uuid)?;
    let title = opts
        .new_name
        .clone()
        .unwrap_or_else(|| heading.title.clone());
    let note_content = build_new_note_content(&opts.heading_uuid, &title, &copied_subtree);
    let slug = title_to_slug(&title);
    let timestamp = chrono::Local::now().format("%Y%m%d%H%M%S").to_string();
    let new_notes_dir = config
        .new_notes_dir
        .as_deref()
        .context("new notes directory is not configured")?;

    let (new_path, created) = if opts.apply {
        std::fs::create_dir_all(new_notes_dir)
            .with_context(|| format!("Failed to create {}", new_notes_dir.display()))?;
        let (_filename, path) =
            create_note_file_exclusive(new_notes_dir, &timestamp, &slug, &note_content)?;
        (path, true)
    } else {
        let (_filename, path) = next_available_note_path(new_notes_dir, &timestamp, &slug);
        (path, false)
    };

    let replacement = render_replacement_heading(heading, &opts.heading_uuid);

    if opts.apply {
        let (_, newline) = split_line_ending(&source_lines[start_idx]);
        let mut updated_lines = source_lines;
        updated_lines.splice(start_idx..end_idx, [format!("{replacement}{newline}")]);
        if let Err(error) = write_lines(&source_path, &updated_lines) {
            let _ = std::fs::remove_file(&new_path);
            return Err(error).with_context(|| {
                format!(
                    "Created new note at {} but failed to rewrite source {}",
                    new_path.display(),
                    source_path.display()
                )
            });
        }
    }

    Ok(ExtractOutput {
        uuid: opts.heading_uuid.clone(),
        title,
        source_path,
        new_path,
        replacement,
        created,
        applied: opts.apply,
    })
}

pub fn render_text(output: &ExtractOutput) -> String {
    let mut text = String::new();

    if output.applied {
        let _ = writeln!(text, "Extracted heading:");
    } else {
        let _ = writeln!(text, "Would extract heading:");
    }
    let _ = writeln!(text, "  Title:       {}", output.title);
    let _ = writeln!(text, "  UUID:        {}", output.uuid);
    let _ = writeln!(text, "  Source:      {}", output.source_path.display());
    let _ = writeln!(text, "  New note:    {}", output.new_path.display());
    let _ = writeln!(text, "  Replacement: {}", output.replacement);
    if output.applied {
        let _ = writeln!(text, "  Status:      applied");
    } else {
        let _ = writeln!(text, "  Status:      dry-run (use --apply to write)");
    }

    text
}

fn resolve_heading_location(graph: &Graph, heading_uuid: &str) -> Result<HeadingLocation> {
    if let Some(duplicate) = graph
        .duplicates
        .duplicate_uuids
        .iter()
        .find(|entry| entry.value == heading_uuid)
    {
        anyhow::bail!(
            "Duplicate UUID state for {heading_uuid}; cannot extract ambiguous heading. Paths: {}",
            duplicate.paths.join(", ")
        );
    }

    if let Some(location) = graph.heading_location(heading_uuid) {
        return Ok(location);
    }

    if graph.nodes.contains_key(heading_uuid) {
        anyhow::bail!(
            "Cannot extract note-level UUID {heading_uuid}; provide a heading-level UUID"
        );
    }

    anyhow::bail!("Heading UUID not found: {heading_uuid}")
}

fn ensure_heading_line(lines: &[String], line_idx: usize, line_number: usize) -> Result<()> {
    let line = lines
        .get(line_idx)
        .ok_or_else(|| anyhow::anyhow!("Heading line {line_number} no longer exists"))?;
    let (body, _) = split_line_ending(line);
    if !is_heading_line(body) {
        anyhow::bail!("Line {line_number} is no longer an org heading");
    }
    Ok(())
}

fn remove_root_heading_id(mut subtree: Vec<String>, uuid: &str) -> Result<Vec<String>> {
    let Some((drawer_start, drawer_end)) = root_properties_drawer_range(&subtree)? else {
        anyhow::bail!("Heading {uuid} has no root properties drawer");
    };

    let id_idx = (drawer_start + 1..drawer_end).find(|idx| {
        let (body, _) = split_line_ending(&subtree[*idx]);
        ID_PROPERTY_RE
            .captures(body.trim())
            .and_then(|cap| cap.get(1).map(|m| m.as_str()))
            .is_some_and(|id| id == uuid)
    });
    let Some(id_idx) = id_idx else {
        anyhow::bail!("Heading {uuid} has no matching root :ID: property");
    };

    subtree.remove(id_idx);
    let adjusted_end = drawer_end - 1;
    let drawer_has_properties = subtree[drawer_start + 1..adjusted_end].iter().any(|line| {
        let (body, _) = split_line_ending(line);
        !body.trim().is_empty()
    });
    if !drawer_has_properties {
        subtree.drain(drawer_start..=adjusted_end);
    }

    Ok(subtree)
}

fn root_properties_drawer_range(subtree: &[String]) -> Result<Option<(usize, usize)>> {
    for idx in 1..subtree.len() {
        let (body, _) = split_line_ending(&subtree[idx]);
        if is_heading_line(body) {
            return Ok(None);
        }
        if body.trim() != ":PROPERTIES:" {
            continue;
        }

        for (end_idx, line) in subtree.iter().enumerate().skip(idx + 1) {
            let (end_body, _) = split_line_ending(line);
            if end_body.trim() == ":END:" {
                return Ok(Some((idx, end_idx)));
            }
        }

        anyhow::bail!("Root heading properties drawer has no :END:");
    }

    Ok(None)
}

fn build_new_note_content(uuid: &str, title: &str, subtree: &[String]) -> String {
    let mut content = format!(":PROPERTIES:\n:ID:       {uuid}\n:END:\n#+title: {title}\n\n");
    content.push_str(&subtree.concat());
    content
}

fn render_replacement_heading(heading: &Heading, uuid: &str) -> String {
    let mut replacement = format!("{} ", "*".repeat(heading.level));

    if let Some(state) = heading.todo_state.as_deref() {
        replacement.push_str(state);
        replacement.push(' ');
    }

    if let Some(priority) = heading.priority {
        replacement.push_str(&format!("[#{priority}] "));
    }

    replacement.push_str(&format!("[[id:{uuid}][{}]]", heading.title));

    if !heading.tags.is_empty() {
        replacement.push(' ');
        replacement.push(':');
        replacement.push_str(&heading.tags.join(":"));
        replacement.push(':');
    }

    replacement
}

fn next_available_note_path(
    new_notes_dir: &Path,
    timestamp: &str,
    slug: &str,
) -> (String, PathBuf) {
    for attempt in 0.. {
        let filename = unique_note_filename(timestamp, slug, attempt);
        let path = new_notes_dir.join(&filename);
        if !path.exists() {
            return (filename, path);
        }
    }

    unreachable!("unbounded filename retry loop should return")
}

#[cfg(test)]
mod tests {
    use super::*;
    use pkms_org::parser::parse_note;

    #[test]
    fn subtree_end_stops_before_same_or_lower_heading() {
        let parsed = parse_note(
            r#":PROPERTIES:
:ID:       aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa
:END:
#+title: Test

* Parent
Body
** Target
Target body
*** Child
Child body
** Sibling
Sibling body
"#,
        );
        let target = &parsed.headings[1];

        assert_eq!(
            parsed_heading_subtree_end_index(&parsed.headings, target, 13),
            11
        );
    }

    #[test]
    fn root_heading_id_removal_preserves_non_id_properties_and_child_ids() {
        let subtree = lines(
            r#"** TODO Original Heading :tag:
:PROPERTIES:
:ID:       11111111-1111-4111-8111-111111111111
:PROJECT: Alpha
:END:
Body
*** Child
:PROPERTIES:
:ID:       22222222-2222-4222-8222-222222222222
:END:
"#,
        );

        let updated =
            remove_root_heading_id(subtree, "11111111-1111-4111-8111-111111111111").unwrap();
        let content = updated.concat();

        assert!(content.contains(":PROJECT: Alpha"));
        assert!(!content.contains(":ID:       11111111-1111-4111-8111-111111111111"));
        assert!(content.contains(":ID:       22222222-2222-4222-8222-222222222222"));
    }

    #[test]
    fn root_heading_id_removal_removes_empty_drawer() {
        let subtree = lines(
            r#"** TODO Original Heading :tag:
:PROPERTIES:
:ID:       11111111-1111-4111-8111-111111111111
:END:
Body
"#,
        );

        let updated =
            remove_root_heading_id(subtree, "11111111-1111-4111-8111-111111111111").unwrap();

        assert_eq!(updated.concat(), "** TODO Original Heading :tag:\nBody\n");
    }

    #[test]
    fn replacement_heading_preserves_todo_priority_and_tags() {
        let parsed = parse_note(
            r#":PROPERTIES:
:ID:       aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa
:END:
#+title: Test

*** TODO [#A] Original Heading :tag:
"#,
        );

        assert_eq!(
            render_replacement_heading(&parsed.headings[0], "11111111-1111-4111-8111-111111111111"),
            "*** TODO [#A] [[id:11111111-1111-4111-8111-111111111111][Original Heading]] :tag:"
        );
    }

    fn lines(content: &str) -> Vec<String> {
        content.split_inclusive('\n').map(str::to_string).collect()
    }
}
