use crate::cli::NewArgs;
use crate::config::ResolvedConfig;
use crate::input;
use crate::output::OutputContext;
use crate::parser::ID_PROPERTY_RE;
use anyhow::{Context, Result};
use serde::Serialize;
use std::fmt::Write;
use std::io::Write as IoWrite;
use std::path::Path;
use std::path::PathBuf;

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
    let files = crate::discovery::walk_org_files(db_root, ignore).ok()?;
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

impl From<&NewArgs> for NewOptions {
    fn from(args: &NewArgs) -> Self {
        NewOptions {
            title: args.title.clone(),
            create: args.create,
            tags: input::comma_list(args.tags.as_deref()),
            aliases: input::comma_list(args.aliases.as_deref()),
            heading: args.heading.clone(),
        }
    }
}

pub fn run(config: &ResolvedConfig, ctx: &OutputContext, opts: &NewOptions) -> Result<()> {
    let db_root = config.resolved_db_root();
    let ignore = config.resolve_ignore_patterns();
    let new_notes_dir = config.resolve_new_notes_dir();

    let uuid = uuid::Uuid::new_v4().to_string();
    let slug = title_to_slug(&opts.title);
    let now = chrono::Local::now();
    let timestamp = now.format("%Y%m%d%H%M%S").to_string();
    let mut filename = unique_note_filename(&timestamp, &slug, 0);
    let mut path = new_notes_dir.join(&filename);

    if opts.create {
        std::fs::create_dir_all(&new_notes_dir)?;
    }

    let mut created = false;
    let heading_output = if let Some(ref heading_title) = opts.heading {
        if !opts.create {
            anyhow::bail!(
                "Cannot use --heading without --create. The note file must exist to add a heading UUID."
            );
        }
        let existing = find_note_by_title(db_root, &ignore, &opts.title);
        if let Some(note_path) = existing {
            let content = std::fs::read_to_string(&note_path)
                .with_context(|| format!("Failed to read {}", note_path.display()))?;
            let heading_uuid = insert_heading_uuid(&content, heading_title, &note_path)?;
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

        (filename, path) = create_note_file_exclusive(&new_notes_dir, &timestamp, &slug, &content)?;
        created = true;
    } else if opts.create && heading_output.is_some() {
        created = true;
    }

    let output = NewOutput {
        uuid,
        filename,
        path: path.clone(),
        title: opts.title.clone(),
        created,
        heading: heading_output,
    };

    if ctx.is_json() {
        ctx.print_json(&output)?;
    } else {
        println!("New note:");
        println!("  Title:    {}", output.title);
        println!("  UUID:     {}", output.uuid);
        println!("  Filename: {}", output.filename);
        println!("  Path:     {}", output.path.display());
        if let Some(ref h) = output.heading {
            println!("  Heading UUID: {} ({})", h.uuid, h.title);
        }
        if opts.create {
            println!("  Status:   created");
        } else {
            println!("  Status:   dry-run (use --create to write)");
        }
    }

    Ok(())
}

fn unique_note_filename(timestamp: &str, slug: &str, attempt: usize) -> String {
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

fn create_note_file_exclusive(
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

fn insert_heading_uuid(content: &str, heading_title: &str, path: &PathBuf) -> Result<String> {
    let heading_re = regex::Regex::new(r"^(\*+)\s+(.*?)(?:\s+:\w+(?::\w+)*:)?\s*$").unwrap();

    let lines: Vec<&str> = content.lines().collect();
    let mut heading_indices = Vec::new();

    let stripped_title = heading_title.trim();
    for (i, line) in lines.iter().enumerate() {
        if let Some(cap) = heading_re.captures(line) {
            let heading_text = cap.get(2).map_or("", |m| m.as_str()).trim();
            // Match heading text without leading TODO keywords
            let text = heading_text
                .split_whitespace()
                .last()
                .unwrap_or(heading_text);
            if heading_text == stripped_title || text == stripped_title {
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

    // Check if heading already has a PROPERTIES drawer with :ID:
    if idx + 1 < lines.len() && lines[idx + 1].trim() == ":PROPERTIES:" {
        let props_section: Vec<&str> = lines[idx + 1..]
            .iter()
            .take_while(|l| l.trim() != ":END:" || **l == lines[idx + 1])
            .copied()
            .collect();
        let props_text = props_section.join("\n");
        if let Some(cap) = ID_PROPERTY_RE.captures(&props_text) {
            return Ok(cap[1].to_string());
        }
    }

    let heading_uuid = uuid::Uuid::new_v4().to_string();

    let mut new_lines: Vec<String> = lines.iter().map(|l| l.to_string()).collect();
    new_lines.insert(idx + 1, String::new());
    new_lines.insert(idx + 2, ":PROPERTIES:".to_string());
    new_lines.insert(idx + 3, format!(":ID:       {heading_uuid}"));
    new_lines.insert(idx + 4, ":END:".to_string());

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

        let (filename, path) = create_note_file_exclusive(
            dir.path(),
            "20260604120000",
            "collision",
            "replacement",
        )
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
}
