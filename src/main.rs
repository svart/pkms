mod cli;
mod commands;
mod config;
mod discovery;
mod graph;
mod parser;

use anyhow::Result;
use clap::Parser;
use cli::{Cli, Command};

fn main() -> Result<()> {
    let cli = Cli::parse();
    let cfg = config::Config::load()?;

    match &cli.command {
        Command::Check => commands::check::run(&cfg, cli.json, cli.verbose, cli.db.as_deref())?,
        Command::Validate { target } => {
            commands::validate::run(&cfg, cli.json, cli.verbose, target, cli.db.as_deref())?
        }
        Command::New {
            title,
            create,
            tags,
            aliases,
        } => commands::new::run(
            &cfg,
            cli.json,
            title,
            *create,
            tags.as_deref(),
            aliases.as_deref(),
            cli.db.as_deref(),
        )?,
        Command::Get {
            target,
            depth,
            out,
        } => commands::get::run(
            &cfg,
            cli.json,
            cli.verbose,
            target,
            *depth,
            *out,
            cli.db.as_deref(),
        )?,
        Command::Query {
            terms,
            tag,
            limit,
        } => commands::query::run(
            &cfg,
            cli.json,
            cli.verbose,
            terms,
            tag.as_deref(),
            *limit,
            cli.db.as_deref(),
        )?,
        Command::Info => commands::info::run(&cfg, cli.json, cli.db.as_deref())?,
        Command::InitConfig { db } => {
            let config_path = dirs::config_dir()
                .ok_or_else(|| anyhow::anyhow!("Could not find XDG config directory"))?
                .join("pkms.toml");
            if config_path.exists() {
                anyhow::bail!("Config already exists at {}", config_path.display());
            }
            let content = config::generate_default_config(db.as_deref());
            std::fs::create_dir_all(config_path.parent().unwrap())?;
            std::fs::write(&config_path, &content)?;
            println!("Created config at {}", config_path.display());
        }
    }

    Ok(())
}
