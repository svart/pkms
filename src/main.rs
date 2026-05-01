mod cli;
mod commands;
mod config;
mod discovery;
mod graph;
mod parser;

use anyhow::Result;
use clap::Parser;
use cli::{Cli, Command, OutputFormat};
use std::process::ExitCode;

fn main() -> ExitCode {
    // Check for --example before full parsing (required args not needed)
    if std::env::args().any(|a| a == "--example") {
        let args: Vec<String> = std::env::args().collect();
        let cmd_pos = args[1..].iter().position(|a| !a.starts_with('-'));
        let example = cmd_pos.and_then(|pos| args.get(1 + pos)).map(|s| s.as_str()).unwrap_or("");
        match example {
            "check" => println!("pkms check"),
            "validate" => println!("pkms validate \"Note Title\""),
            "stats" => println!("pkms stats --days 30"),
            "orphans" => println!("pkms orphans"),
            "broken" => println!("pkms broken --count"),
            "hubs" => println!("pkms hubs --limit 5"),
            "context" => println!("pkms context \"Note Title\" --depth 2 --max-tokens 500"),
            "resolve" => println!("pkms resolve \"uuid-prefix\" --fields uuid,title"),
            "fix" => println!("pkms fix \"ffffffff-...\" \"Note Title\""),
            "suggest" => println!("pkms suggest \"Note Title\" --limit 5"),
            "new" => println!("pkms new \"My Note Title\" --create --tags foo,bar"),
            "get" => println!("pkms get \"Note Title\" --depth 2 --out"),
            "query" => println!("pkms query \"search terms\" --tag learning --limit 10"),
            "info" => println!("pkms info --json"),
            "init-config" => println!("pkms init-config"),
            "path" => println!("pkms path \"Note A\" \"Note B\""),
            "subgraph" => println!("pkms subgraph \"Note Title\" --depth 2"),
            "tags" => println!("pkms tags --tag learning"),
            _ => {
                eprintln!("Usage: pkms [global-options] <command> [args]");
                eprintln!("Try 'pkms --help' for more information.");
                return ExitCode::from(2);
            }
        }
        return ExitCode::SUCCESS;
    }

    let cli = Cli::parse();
    let ndjson = matches!(cli.output_format, Some(OutputFormat::Ndjson));
    let use_json = cli.json || ndjson;
    let machine = use_json;
    let quiet = cli.quiet || use_json;
    let no_header = cli.no_header;
    let count_only = cli.count_only;

    let cfg = match config::Config::load() {
        Ok(c) => c,
        Err(e) => {
            if machine {
                println!("{}", serde_json::json!({"error": e.to_string()}));
            }
            return ExitCode::from(2);
        }
    };

    let result: Result<ExitCode> = match &cli.command {
        Command::Check => {
            commands::check::run(&cfg, use_json, cli.verbose, cli.db.as_deref())
                .map(|healthy| if healthy { ExitCode::SUCCESS } else { ExitCode::from(1) })
        }
        Command::Validate { target } => {
            commands::validate::run(&cfg, use_json, cli.verbose, target, cli.db.as_deref())
                .map(|_| ExitCode::SUCCESS)
        }
        Command::Stats { days } => {
            commands::stats::run(&cfg, use_json, cli.verbose, *days, cli.db.as_deref())
                .map(|_| ExitCode::SUCCESS)
        }
        Command::Orphans => {
            commands::orphans::run(&cfg, use_json, ndjson, no_header, count_only, cli.db.as_deref())
                .map(|_| ExitCode::SUCCESS)
        }
        Command::Broken => {
            commands::broken::run(&cfg, use_json, ndjson, no_header, count_only, cli.db.as_deref())
                .map(|_| ExitCode::SUCCESS)
        }
        Command::Hubs { limit } => {
            commands::hubs::run(&cfg, use_json, ndjson, no_header, count_only, *limit, cli.db.as_deref())
                .map(|_| ExitCode::SUCCESS)
        }
        Command::Context { target, depth, max_tokens, include_outgoing, include_incoming, template } => {
            commands::context::run(&cfg, use_json, quiet, target, *depth, *max_tokens, *include_outgoing, *include_incoming, template.as_deref(), cli.db.as_deref())
                .map(|_| ExitCode::SUCCESS)
        }
        Command::Resolve { target, tags, search, limit, fields } => {
            commands::resolve::run(&cfg, use_json, ndjson, no_header, count_only, target.as_deref(), tags.as_deref(), search.as_deref(), *limit, fields.as_deref(), cli.db.as_deref())
                .map(|_| ExitCode::SUCCESS)
        }
        Command::Fix { broken_uuid, target, apply } => {
            commands::fix::run(&cfg, use_json, cli.verbose, broken_uuid, target, *apply, cli.db.as_deref())
                .map(|_| ExitCode::SUCCESS)
        }
        Command::Suggest { target, limit } => {
            commands::suggest::run(&cfg, use_json, cli.verbose, target, *limit, cli.db.as_deref())
                .map(|_| ExitCode::SUCCESS)
        }
        Command::New { title, create, tags, aliases } => {
            commands::new::run(&cfg, use_json, title, *create, tags.as_deref(), aliases.as_deref(), cli.db.as_deref())
                .map(|_| ExitCode::SUCCESS)
        }
        Command::Get { target, depth, out, graph: show_graph, from_stdin, from_file, input_json } => {
            match load_get_params(input_json, target, *depth, *out, *show_graph) {
                Ok((t, d, o, g)) => commands::get::run(&cfg, use_json, cli.verbose, t.as_deref(), d, o, g, *from_stdin, from_file.as_ref(), cli.db.as_deref())
                    .map(|_| ExitCode::SUCCESS),
                Err(e) => Err(e),
            }
        }
        Command::Query { terms, tag, limit } => {
            commands::query::run(&cfg, use_json, ndjson, no_header, count_only, cli.verbose, terms, tag.as_deref(), *limit, cli.db.as_deref())
                .map(|_| ExitCode::SUCCESS)
        }
        Command::Info => {
            commands::info::run(&cfg, use_json, cli.db.as_deref())
                .map(|_| ExitCode::SUCCESS)
        }
        Command::InitConfig { db } => {
            init_config(db.as_deref(), cli.json)
        }
        Command::Path { from, to, max_depth } => {
            commands::path::run(&cfg, use_json, cli.verbose, from, to, *max_depth, cli.db.as_deref())
                .map(|_| ExitCode::SUCCESS)
        }
        Command::Subgraph { target, depth } => {
            commands::subgraph::run(&cfg, use_json, cli.verbose, target, *depth, cli.db.as_deref())
                .map(|_| ExitCode::SUCCESS)
        }
        Command::Tags { tag } => {
            commands::tags::run(&cfg, use_json, ndjson, no_header, count_only, cli.verbose, tag.as_deref(), cli.db.as_deref())
                .map(|_| ExitCode::SUCCESS)
        }
    };

    match result {
        Ok(code) => code,
        Err(e) => {
            if machine {
                println!("{}", serde_json::json!({"error": e.to_string()}));
            } else {
                eprintln!("Error: {}", e);
            }
            ExitCode::from(1)
        }
    }
}

fn load_get_params(
    input_json: &Option<std::path::PathBuf>,
    target: &Option<String>,
    depth: u32,
    out: bool,
    show_graph: bool,
) -> Result<(Option<String>, u32, bool, bool)> {
    if let Some(json_path) = input_json {
        let content = std::fs::read_to_string(json_path)?;
        let params: serde_json::Value = serde_json::from_str(&content)?;
        let t = params.get("target").and_then(|v| v.as_str().map(|s| s.to_string()));
        let d = params.get("depth").and_then(|v| v.as_u64()).unwrap_or(depth as u64) as u32;
        let o = params.get("out").and_then(|v| v.as_bool()).unwrap_or(out);
        let g = params.get("graph").and_then(|v| v.as_bool()).unwrap_or(show_graph);
        Ok((t, d, o, g))
    } else {
        Ok((target.clone(), depth, out, show_graph))
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
