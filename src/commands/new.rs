use crate::config::Config;
use crate::output::OutputContext;
use crate::parser::ID_PROPERTY_RE;
use anyhow::{Context, Result};
use serde::Serialize;
use std::fmt::Write;
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

#[allow(clippy::too_many_arguments)]
pub struct NewOptions {
    pub title: String,
    pub create: bool,
    pub tags: Option<Vec<String>>,
    pub aliases: Option<Vec<String>>,
    pub heading: Option<String>,
}

pub fn run(config: &Config, ctx: &OutputContext, opts: &NewOptions) -> Result<()> {
    let db_root = config.resolved_db_root()?;
    let ignore = config.resolve_ignore_patterns();
    let new_notes_dir = config.resolve_new_notes_dir(db_root);

    let uuid = uuid::Uuid::new_v4().to_string();
    let slug = title_to_slug(&opts.title);
    let now = chrono::Local::now();
    let timestamp = now.format("%Y%m%d%H%M%S").to_string();
    let filename = format!("{timestamp}-{slug}.org");
    let path = new_notes_dir.join(&filename);

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
        let mut content = format!(
            ":PROPERTIES:\n:ID:       {uuid}\n:END:\n#+title: {}\n",
            opts.title
        );

        if let Some(ref tags) = opts.tags
            && !tags.is_empty()
        {
            let ft = tags.iter().fold(String::new(), |mut acc, t| {
                let _ = write!(acc, ":{t}:");
                acc
            });
            let _ = writeln!(content, "#+filetags: {ft}");
        }

        if let Some(ref aliases) = opts.aliases
            && !aliases.is_empty()
        {
            content.push_str(":PROPERTIES:\n");
            let _ = writeln!(content, ":ROAM_ALIASES: {}", aliases.join(" "));
            content.push_str(":END:\n");
        }

        std::fs::write(&path, &content)?;
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
    title
        .to_lowercase()
        .chars()
        .map(|c| match c {
            'a'..='z' | '0'..='9' => c,
            '_' | '-' => '_',
            _ if c.is_whitespace() => '_',
            _ => '-',
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}
