use crate::config::Config;
use crate::graph::Graph;
use anyhow::Result;
use serde::Serialize;
use std::fs;

#[derive(Serialize)]
pub struct AddLinkOutput {
    pub source_uuid: String,
    pub source_path: String,
    pub target_uuid: String,
    pub target_title: String,
    pub link_text: String,
}

pub fn run(
    config: &Config,
    json: bool,
    verbose: bool,
    source: &str,
    target: &str,
    description: Option<&str>,
    db_cli: Option<&std::path::Path>,
) -> Result<()> {
    let graph = Graph::load(config, db_cli, verbose)?;

    let source_node = graph
        .find_node(source)
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("Source note not found: {}", source))?;
    let target_node = graph
        .find_node(target)
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("Target note not found: {}", target))?;

    let link_text = description
        .map(|d| d.to_string())
        .unwrap_or_else(|| target_node.title.clone());

    let link = format!("[[id:{}][{}]]", target_node.uuid, link_text);
    let link_line = format!("\n{}", link);

    let mut content = fs::read_to_string(&source_node.path)?;
    content.push_str(&link_line);
    fs::write(&source_node.path, &content)?;

    if json {
        let output = AddLinkOutput {
            source_uuid: source_node.uuid,
            source_path: source_node.path.to_string_lossy().to_string(),
            target_uuid: target_node.uuid,
            target_title: target_node.title,
            link_text,
        };
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        println!("Added link:");
        println!("  Source: {} ({})", source_node.title, source_node.path.display());
        println!("  Target: {} ({})", target_node.title, target_node.uuid);
        println!("  Link:   {}", link);
    }

    Ok(())
}
