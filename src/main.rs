mod cli;
mod commands;
mod config;
mod discovery;
mod graph;
mod parser;

use anyhow::Result;
use clap::Parser;
use cli::{Cli, Command};
use std::process::ExitCode;

fn main() -> Result<ExitCode> {
    let cli = Cli::parse();
    let cfg = config::Config::load()?;

    match &cli.command {
        Command::Check => {
            let healthy = commands::check::run(&cfg, cli.json, cli.verbose, cli.db.as_deref())?;
            Ok(if healthy { ExitCode::SUCCESS } else { ExitCode::from(1) })
        }
        Command::Validate { target } => {
            commands::validate::run(&cfg, cli.json, cli.verbose, target, cli.db.as_deref())?;
            Ok(ExitCode::SUCCESS)
        }
        Command::Stats { days } => {
            commands::stats::run(&cfg, cli.json, cli.verbose, *days, cli.db.as_deref())?;
            Ok(ExitCode::SUCCESS)
        }
        Command::Orphans => {
            commands::orphans::run(&cfg, cli.json, cli.verbose, cli.db.as_deref())?;
            Ok(ExitCode::SUCCESS)
        }
        Command::Broken => {
            commands::broken::run(&cfg, cli.json, cli.verbose, cli.db.as_deref())?;
            Ok(ExitCode::SUCCESS)
        }
        Command::Hubs { limit } => {
            commands::hubs::run(&cfg, cli.json, cli.verbose, *limit, cli.db.as_deref())?;
            Ok(ExitCode::SUCCESS)
        }
        Command::Context { target, depth, max_tokens } => {
            commands::context::run(&cfg, cli.json, cli.verbose, target, *depth, *max_tokens, cli.db.as_deref())?;
            Ok(ExitCode::SUCCESS)
        }
        Command::New { title, create, tags, aliases } => {
            commands::new::run(&cfg, cli.json, title, *create, tags.as_deref(), aliases.as_deref(), cli.db.as_deref())?;
            Ok(ExitCode::SUCCESS)
        }
        Command::Get { target, depth, out, graph: show_graph } => {
            commands::get::run(&cfg, cli.json, cli.verbose, target, *depth, *out, *show_graph, cli.db.as_deref())?;
            Ok(ExitCode::SUCCESS)
        }
        Command::Query { terms, tag, limit } => {
            commands::query::run(&cfg, cli.json, cli.verbose, terms, tag.as_deref(), *limit, cli.db.as_deref())?;
            Ok(ExitCode::SUCCESS)
        }
        Command::Info => {
            commands::info::run(&cfg, cli.json, cli.db.as_deref())?;
            Ok(ExitCode::SUCCESS)
        }
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
            Ok(ExitCode::SUCCESS)
        }
        Command::Path { from, to, max_depth } => {
            commands::path::run(&cfg, cli.json, cli.verbose, from, to, *max_depth, cli.db.as_deref())?;
            Ok(ExitCode::SUCCESS)
        }
        Command::Subgraph { target, depth } => {
            commands::subgraph::run(&cfg, cli.json, cli.verbose, target, *depth, cli.db.as_deref())?;
            Ok(ExitCode::SUCCESS)
        }
        Command::Tags { tag } => {
            commands::tags::run(&cfg, cli.json, cli.verbose, tag.as_deref(), cli.db.as_deref())?;
            Ok(ExitCode::SUCCESS)
        }
    }
}
