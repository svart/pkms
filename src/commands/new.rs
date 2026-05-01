use crate::config::Config;
use anyhow::Result;
use serde::Serialize;
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
    json: bool,
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
    let now = chrono::Utc::now();
    let timestamp = now.format("%Y%m%d%H%M%S").to_string();
    let filename = format!("{}-{}.org", timestamp, slug);
    let path = new_notes_dir.join(&filename);

    // Ensure target directory exists
    if create {
        std::fs::create_dir_all(&new_notes_dir)?;
    }

    let mut created = false;
    if create {
        let mut content = format!(
            ":PROPERTIES:\n:ID:       {}\n:END:\n#+title: {}\n",
            uuid, title
        );

        if let Some(tags_str) = tags {
            let tags_list: Vec<&str> = tags_str.split(',').map(|t| t.trim()).collect();
            if !tags_list.is_empty() {
                let ft = tags_list
                    .iter()
                    .map(|t| format!(":{}:", t))
                    .collect::<Vec<_>>()
                    .join("");
                content.push_str(&format!("#+filetags: {}\n", ft));
            }
        }

        if let Some(aliases_str) = aliases {
            let aliases_list: Vec<&str> = aliases_str.split(',').map(|a| a.trim()).collect();
            if !aliases_list.is_empty() {
                content.push_str(":PROPERTIES:\n");
                content.push_str(&format!(":ROAM_ALIASES: {}\n", aliases_list.join(" ")));
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

    if json {
        println!("{}", serde_json::to_string_pretty(&output)?);
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

fn title_to_slug(title: &str) -> String {
    title
        .to_lowercase()
        .chars()
        .map(|c| match c {
            'a'..='z' | '0'..='9' => c,
            '_' => '_',
            ' ' | '-' => '_',
            _ if c.is_whitespace() => '_',
            _ => '-',
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}
