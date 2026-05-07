use crate::config::Config;
use crate::output::OutputContext;
use anyhow::Result;
use serde::Serialize;
use std::fmt::Write;
use std::path::PathBuf;

#[derive(Serialize)]
pub struct NewOutput {
    pub uuid: String,
    pub filename: String,
    pub path: PathBuf,
    pub title: String,
    pub created: bool,
}

pub fn run(
    config: &Config,
    ctx: &OutputContext,
    title: &str,
    create: bool,
    tags: Option<&str>,
    aliases: Option<&str>,
    db_cli: Option<&std::path::Path>,
) -> Result<()> {
    let db_root = config.resolve_db_root(db_cli)?;
    let new_notes_dir = config.resolve_new_notes_dir(&db_root);

    let uuid = uuid::Uuid::new_v4().to_string();
    let slug = title_to_slug(title);
    let now = chrono::Local::now();
    let timestamp = now.format("%Y%m%d%H%M%S").to_string();
    let filename = format!("{timestamp}-{slug}.org");
    let path = new_notes_dir.join(&filename);

    if create {
        std::fs::create_dir_all(&new_notes_dir)?;
    }

    let mut created = false;
    if create {
        let mut content = format!(":PROPERTIES:\n:ID:       {uuid}\n:END:\n#+title: {title}\n");

        if let Some(tags_str) = tags {
            let tags_list: Vec<&str> = tags_str.split(',').map(str::trim).collect();
            if !tags_list.is_empty() {
                let ft = tags_list.iter().fold(String::new(), |mut acc, t| {
                    let _ = write!(acc, ":{t}:");
                    acc
                });
                let _ = writeln!(content, "#+filetags: {ft}");
            }
        }

        if let Some(aliases_str) = aliases {
            let aliases_list: Vec<&str> = aliases_str.split(',').map(str::trim).collect();
            if !aliases_list.is_empty() {
                content.push_str(":PROPERTIES:\n");
                let _ = writeln!(content, ":ROAM_ALIASES: {}", aliases_list.join(" "));
                content.push_str(":END:\n");
            }
        }

        std::fs::write(&path, &content)?;
        created = true;
    }

    let output = NewOutput {
        uuid,
        filename,
        path,
        title: title.to_string(),
        created,
    };

    if ctx.is_json() {
        ctx.print_json(&output)?;
    } else {
        println!("New note:");
        println!("  Title:    {}", output.title);
        println!("  UUID:     {}", output.uuid);
        println!("  Filename: {}", output.filename);
        println!("  Path:     {}", output.path.display());
        if create {
            println!("  Status:   created");
        } else {
            println!("  Status:   dry-run (use --create to write)");
        }
    }

    Ok(())
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
