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

fn main() -> ExitCode {
    let cli = Cli::parse();
    let quiet = cli.quiet || cli.json;

    let cfg = match config::Config::load() {
        Ok(c) => c,
        Err(e) => {
            if cli.json {
                println!("{}", serde_json::json!({"error": e.to_string()}));
            }
            return ExitCode::from(2);
        }
    };

    let result: Result<ExitCode> = match &cli.command {
        Command::Check => {
            commands::check::run(&cfg, cli.json, cli.verbose, cli.db.as_deref())
                .map(|healthy| if healthy { ExitCode::SUCCESS } else { ExitCode::from(1) })
        }
        Command::Validate { target } => {
            commands::validate::run(&cfg, cli.json, cli.verbose, target, cli.db.as_deref())
                .map(|_| ExitCode::SUCCESS)
        }
        Command::Stats { days } => {
            commands::stats::run(&cfg, cli.json, cli.verbose, *days, cli.db.as_deref())
                .map(|_| ExitCode::SUCCESS)
        }
        Command::Orphans => {
            commands::orphans::run(&cfg, cli.json, cli.verbose, cli.db.as_deref())
                .map(|_| ExitCode::SUCCESS)
        }
        Command::Broken => {
            commands::broken::run(&cfg, cli.json, cli.verbose, cli.db.as_deref())
                .map(|_| ExitCode::SUCCESS)
        }
        Command::Hubs { limit } => {
            commands::hubs::run(&cfg, cli.json, cli.verbose, *limit, cli.db.as_deref())
                .map(|_| ExitCode::SUCCESS)
        }
        Command::Context { target, depth, max_tokens } => {
            commands::context::run(&cfg, cli.json, quiet, target, *depth, *max_tokens, cli.db.as_deref())
                .map(|_| ExitCode::SUCCESS)
        }
        Command::Resolve { target, tags, search, limit } => {
            commands::resolve::run(&cfg, cli.json, cli.verbose, target.as_deref(), tags.as_deref(), search.as_deref(), *limit, cli.db.as_deref())
                .map(|_| ExitCode::SUCCESS)
        }
        Command::Fix { broken_uuid, target, apply } => {
            commands::fix::run(&cfg, cli.json, cli.verbose, broken_uuid, target, *apply, cli.db.as_deref())
                .map(|_| ExitCode::SUCCESS)
        }
        Command::Suggest { target, limit } => {
            commands::suggest::run(&cfg, cli.json, cli.verbose, target, *limit, cli.db.as_deref())
                .map(|_| ExitCode::SUCCESS)
        }
        Command::New { title, create, tags, aliases } => {
            commands::new::run(&cfg, cli.json, title, *create, tags.as_deref(), aliases.as_deref(), cli.db.as_deref())
                .map(|_| ExitCode::SUCCESS)
        }
        Command::Get { target, depth, out, graph: show_graph } => {
            commands::get::run(&cfg, cli.json, cli.verbose, target, *depth, *out, *show_graph, cli.db.as_deref())
                .map(|_| ExitCode::SUCCESS)
        }
        Command::Query { terms, tag, limit } => {
            commands::query::run(&cfg, cli.json, cli.verbose, terms, tag.as_deref(), *limit, cli.db.as_deref())
                .map(|_| ExitCode::SUCCESS)
        }
        Command::Info => {
            commands::info::run(&cfg, cli.json, cli.db.as_deref())
                .map(|_| ExitCode::SUCCESS)
        }
        Command::InitConfig { db } => {
            init_config(db.as_deref(), cli.json)
        }
        Command::Path { from, to, max_depth } => {
            commands::path::run(&cfg, cli.json, cli.verbose, from, to, *max_depth, cli.db.as_deref())
                .map(|_| ExitCode::SUCCESS)
        }
        Command::Subgraph { target, depth } => {
            commands::subgraph::run(&cfg, cli.json, cli.verbose, target, *depth, cli.db.as_deref())
                .map(|_| ExitCode::SUCCESS)
        }
        Command::Tags { tag } => {
            commands::tags::run(&cfg, cli.json, cli.verbose, tag.as_deref(), cli.db.as_deref())
                .map(|_| ExitCode::SUCCESS)
        }
    };

    match result {
        Ok(code) => code,
        Err(e) => {
            if cli.json {
                println!("{}", serde_json::json!({"error": e.to_string()}));
            }
            ExitCode::from(1)
        }
    }
}

fn init_config(db: Option<&std::path::Path>, json: bool) -> Result<ExitCode> {
    let config_path = dirs::config_dir()
        .ok_or_else(|| anyhow::anyhow!("Could not find XDG config directory"))?
        .join("pkms.toml");
    if config_path.exists() {
        anyhow::bail!("Config already exists at {}", config_path.display());
    }
    let content = config::generate_default_config(db);
    std::fs::create_dir_all(config_path.parent().unwrap())?;
    std::fs::write(&config_path, &content)?;
    if json {
        println!("{}", serde_json::json!({"created": config_path.to_string_lossy()}));
    } else {
        println!("Created config at {}", config_path.display());
    }
    Ok(ExitCode::SUCCESS)
}
