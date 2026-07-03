use anyhow::{Context, Result};
use pkms_org::parser::{HEADING_RE, ID_PROPERTY_RE};
use serde::Serialize;
use std::fmt::Write;
use std::io::Write as IoWrite;
use std::path::{Path, PathBuf};

#[derive(Serialize)]
pub struct HeadingId {
    pub title: String,
    pub uuid: String,
}

#[derive(Serialize)]
pub struct NewOutput {
    pub uuid: String,
    pub filename: String,
    pub path: PathBuf,
    pub title: String,
    pub created: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub heading: Option<HeadingId>,
}

fn find_note_by_title(
    db_root: &std::path::Path,
    ignore: &[String],
    title: &str,
) -> Option<std::path::PathBuf> {
    let title_re = regex::Regex::new(r"(?im)^#\+title:\s*(.*)$").ok()?;
    let files = pkms_org::discovery::walk_org_files(db_root, ignore).ok()?;
    for path in files {
        let content = std::fs::read_to_string(&path).ok()?;
        if let Some(cap) = title_re.captures(&content) {
            let note_title = cap.get(1).map_or("", |m| m.as_str()).trim();
            if note_title == title {
                return Some(path);
            }
        }
    }
    None
}

pub struct NewOptions {
    pub title: String,
    pub create: bool,
    pub tags: Option<Vec<String>>,
    pub aliases: Option<Vec<String>>,
    pub heading: Option<String>,
}

pub fn execute(config: &pkms_org::OrgConfig, opts: &NewOptions) -> Result<NewOutput> {
    let db_root = config.db_root.as_path();
    let ignore = config.ignore_patterns.as_slice();
    let new_notes_dir = config
        .new_notes_dir
        .as_deref()
        .context("new notes directory is not configured")?;

    let mut uuid = uuid::Uuid::new_v4().to_string();
    let slug = title_to_slug(&opts.title);
    let now = chrono::Local::now();
    let timestamp = now.format("%Y%m%d%H%M%S").to_string();
    let mut filename = unique_note_filename(&timestamp, &slug, 0);
    let mut path = new_notes_dir.join(&filename);

    if opts.create {
        std::fs::create_dir_all(new_notes_dir)?;
    }

    let mut created = false;
    let heading_output = if let Some(ref heading_title) = opts.heading {
        if !opts.create {
            anyhow::bail!(
                "Cannot use --heading without --create. The note file must exist to add a heading UUID."
            );
        }
        let existing = find_note_by_title(db_root, ignore, &opts.title);
        if let Some(note_path) = existing {
            let content = std::fs::read_to_string(&note_path)
                .with_context(|| format!("Failed to read {}", note_path.display()))?;
            uuid = primary_uuid_from_content(&content).ok_or_else(|| {
                anyhow::anyhow!(
                    "Note \"{}\" has no primary :ID: property in {}",
                    opts.title,
                    note_path.display()
                )
            })?;
            let heading_uuid = insert_heading_uuid(&content, heading_title, &note_path)?;
            filename = note_path
                .file_name()
                .map(|name| name.to_string_lossy().to_string())
                .unwrap_or_default();
            path = note_path;
            Some(HeadingId {
                title: heading_title.to_string(),
                uuid: heading_uuid,
            })
        } else {
            anyhow::bail!(
                "Note not found with title \"{}\". \
                 Create the note first with `pkms new \"{}\" --create`.",
                opts.title,
                opts.title
            );
        }
    } else {
        None
    };

    if opts.create && heading_output.is_none() {
        let mut content = format!(":PROPERTIES:\n:ID:       {uuid}\n");

        if let Some(ref aliases) = opts.aliases
            && !aliases.is_empty()
        {
            let _ = writeln!(content, ":ROAM_ALIASES: {}", format_roam_aliases(aliases));
        }

        let _ = writeln!(content, ":END:\n#+title: {}", opts.title);

        if let Some(ref tags) = opts.tags
            && !tags.is_empty()
        {
            let ft = tags.iter().fold(String::new(), |mut acc, t| {
                let _ = write!(acc, ":{t}:");
                acc
            });
            let _ = writeln!(content, "#+filetags: {ft}");
        }

        (filename, path) = create_note_file_exclusive(new_notes_dir, &timestamp, &slug, &content)?;
        created = true;
    } else if opts.create && heading_output.is_some() {
        created = true;
    }

    Ok(NewOutput {
        uuid,
        filename,
        path: path.clone(),
        title: opts.title.clone(),
        created,
        heading: heading_output,
    })
}

pub fn render_text(output: &NewOutput) -> String {
    let mut text = String::new();

    let _ = writeln!(text, "New note:");
    let _ = writeln!(text, "  Title:    {}", output.title);
    let _ = writeln!(text, "  UUID:     {}", output.uuid);
    let _ = writeln!(text, "  Filename: {}", output.filename);
    let _ = writeln!(text, "  Path:     {}", output.path.display());
    if let Some(ref h) = output.heading {
        let _ = writeln!(text, "  Heading UUID: {} ({})", h.uuid, h.title);
    }
    if output.created {
        let _ = writeln!(text, "  Status:   created");
    } else {
        let _ = writeln!(text, "  Status:   dry-run (use --create to write)");
    }

    text
}

pub fn unique_note_filename(timestamp: &str, slug: &str, attempt: usize) -> String {
    if attempt == 0 {
        format!("{timestamp}-{slug}.org")
    } else {
        format!("{timestamp}-{slug}-{attempt}.org")
    }
}

fn format_roam_aliases(aliases: &[String]) -> String {
    aliases
        .iter()
        .map(|alias| {
            let escaped = alias.replace('\\', "\\\\").replace('"', "\\\"");
            format!("\"{escaped}\"")
        })
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn create_note_file_exclusive(
    new_notes_dir: &Path,
    timestamp: &str,
    slug: &str,
    content: &str,
) -> Result<(String, PathBuf)> {
    for attempt in 0.. {
        let filename = unique_note_filename(timestamp, slug, attempt);
        let path = new_notes_dir.join(&filename);
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(mut file) => {
                file.write_all(content.as_bytes())?;
                return Ok((filename, path));
            }
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(err) => {
                return Err(err).with_context(|| format!("Failed to create {}", path.display()));
            }
        }
    }

    unreachable!("unbounded filename retry loop should return or error")
}

fn primary_uuid_from_content(content: &str) -> Option<String> {
    ID_PROPERTY_RE
        .captures(content)
        .and_then(|cap| cap.get(1).map(|m| m.as_str().to_string()))
}

fn insert_heading_uuid(content: &str, heading_title: &str, path: &PathBuf) -> Result<String> {
    let lines: Vec<&str> = content.lines().collect();
    let mut heading_indices = Vec::new();

    let stripped_title = heading_title.trim();
    for (i, line) in lines.iter().enumerate() {
        if let Some(cap) = HEADING_RE.captures(line) {
            let heading_text = cap.get(4).map_or("", |m| m.as_str()).trim();
            let full_heading_text = cap
                .get(2)
                .map(|state| format!("{} {heading_text}", state.as_str()))
                .unwrap_or_else(|| heading_text.to_string());
            if heading_text == stripped_title || full_heading_text == stripped_title {
                heading_indices.push(i);
            }
        }
    }

    if heading_indices.is_empty() {
        anyhow::bail!(
            "Heading \"{heading_title}\" not found in {}",
            path.display()
        );
    }
    if heading_indices.len() > 1 {
        anyhow::bail!(
            "Multiple headings \"{heading_title}\" found in {}. \
             Heading UUID generation requires a unique heading.",
            path.display()
        );
    }

    let idx = heading_indices[0];

    if idx + 1 < lines.len() && lines[idx + 1].trim() == ":PROPERTIES:" {
        let end_idx = lines[idx + 2..]
            .iter()
            .position(|line| line.trim() == ":END:")
            .map(|offset| idx + 2 + offset)
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Heading \"{heading_title}\" has a PROPERTIES drawer without :END: in {}",
                    path.display()
                )
            })?;
        let props_text = lines[idx + 1..=end_idx].join("\n");
        if let Some(cap) = ID_PROPERTY_RE.captures(&props_text) {
            return Ok(cap[1].to_string());
        }

        let heading_uuid = uuid::Uuid::new_v4().to_string();
        let mut new_lines: Vec<String> = lines.iter().map(|l| l.to_string()).collect();
        new_lines.insert(end_idx, format!(":ID:       {heading_uuid}"));
        std::fs::write(path, new_lines.join("\n"))?;
        return Ok(heading_uuid);
    }

    let heading_uuid = uuid::Uuid::new_v4().to_string();

    let mut new_lines: Vec<String> = lines.iter().map(|l| l.to_string()).collect();
    new_lines.insert(idx + 1, ":PROPERTIES:".to_string());
    new_lines.insert(idx + 2, format!(":ID:       {heading_uuid}"));
    new_lines.insert(idx + 3, ":END:".to_string());

    std::fs::write(path, new_lines.join("\n"))?;

    Ok(heading_uuid)
}

pub fn title_to_slug(title: &str) -> String {
    let s = title.to_lowercase();
    let s = regex::Regex::new(r"\s+").unwrap().replace_all(&s, "_");
    let s = s.replace('-', "_");
    let s = regex::Regex::new(r"[^a-z0-9_]")
        .unwrap()
        .replace_all(&s, "-");
    s.trim_matches('-').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_title_to_slug_basic() {
        assert_eq!(title_to_slug("Hello World"), "hello_world");
        assert_eq!(title_to_slug("hello-world"), "hello_world");
        assert_eq!(title_to_slug("-"), "_");
        assert_eq!(title_to_slug("a@b"), "a-b");
    }

    #[test]
    fn create_note_file_exclusive_uses_suffix_when_candidate_exists() {
        let dir = tempfile::tempdir().unwrap();
        let existing = dir.path().join("20260604120000-collision.org");
        std::fs::write(&existing, "original").unwrap();

        let (filename, path) =
            create_note_file_exclusive(dir.path(), "20260604120000", "collision", "replacement")
                .unwrap();

        assert_eq!(filename, "20260604120000-collision-1.org");
        assert_eq!(path.file_name().unwrap(), "20260604120000-collision-1.org");
        assert_eq!(std::fs::read_to_string(existing).unwrap(), "original");
        assert_eq!(std::fs::read_to_string(path).unwrap(), "replacement");
    }

    #[test]
    fn format_roam_aliases_quotes_multi_word_aliases() {
        let aliases = vec![
            "Alias One".to_string(),
            "Alias \"Two\"".to_string(),
            "Alias\\Three".to_string(),
        ];

        assert_eq!(
            format_roam_aliases(&aliases),
            r#""Alias One" "Alias \"Two\"" "Alias\\Three""#
        );
    }

    #[test]
    fn render_text_preserves_new_note_ordering_and_created_status() {
        let output = NewOutput {
            uuid: "11111111-1111-4111-8111-111111111111".to_string(),
            filename: "20260702120000-test-note.org".to_string(),
            path: PathBuf::from("/tmp/db/roam/common/20260702120000-test-note.org"),
            title: "Test Note".to_string(),
            created: true,
            heading: Some(HeadingId {
                title: "Section".to_string(),
                uuid: "22222222-2222-4222-8222-222222222222".to_string(),
            }),
        };

        assert_eq!(
            render_text(&output),
            "New note:\n  Title:    Test Note\n  UUID:     11111111-1111-4111-8111-111111111111\n  Filename: 20260702120000-test-note.org\n  Path:     /tmp/db/roam/common/20260702120000-test-note.org\n  Heading UUID: 22222222-2222-4222-8222-222222222222 (Section)\n  Status:   created\n"
        );
    }
}
