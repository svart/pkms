#![allow(
    clippy::too_many_arguments,
    clippy::fn_params_excessive_bools,
    clippy::cast_precision_loss,
    clippy::cast_lossless,
    clippy::ref_option
)]

mod cli;
mod commands;
mod config;
mod discovery;
mod graph;
mod parser;
mod util;

use anyhow::Result;
use clap::Parser;
use cli::{Cli, Command, OutputFormat};
use std::process::ExitCode;

#[allow(clippy::too_many_lines)]
fn main() -> ExitCode {
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
            } else {
                eprintln!("Error: {e:#}");
            }
            return ExitCode::from(2);
        }
    };

    let result: Result<ExitCode> = match &cli.command {
        Command::Check {
            file_links,
            attachment_links,
        } => commands::check::run(
            &cfg,
            use_json,
            cli.verbose,
            cli.db.as_deref(),
            *file_links,
            *attachment_links,
        ),
        Command::Validate { target, input_json } => commands::validate::run(
            &cfg,
            use_json,
            cli.verbose,
            target.as_deref(),
            input_json.as_ref(),
            cli.db.as_deref(),
        )
        .map(|()| ExitCode::SUCCESS),
        Command::Stats { days } => {
            commands::stats::run(&cfg, use_json, cli.verbose, *days, cli.db.as_deref())
                .map(|()| ExitCode::SUCCESS)
        }
        Command::Orphans => commands::orphans::run(
            &cfg,
            use_json,
            ndjson,
            no_header,
            count_only,
            cli.db.as_deref(),
        )
        .map(|()| ExitCode::SUCCESS),
        Command::Broken => commands::broken::run(
            &cfg,
            use_json,
            ndjson,
            no_header,
            count_only,
            cli.db.as_deref(),
        )
        .map(|()| ExitCode::SUCCESS),
        Command::Hubs { limit } => commands::hubs::run(
            &cfg,
            use_json,
            ndjson,
            no_header,
            count_only,
            *limit,
            cli.db.as_deref(),
        )
        .map(|()| ExitCode::SUCCESS),
        Command::Context {
            target,
            depth,
            max_tokens,
            include_outgoing,
            include_incoming,
            template,
            input_json,
        } => commands::context::run(
            &cfg,
            use_json,
            quiet,
            target.as_deref(),
            *depth,
            *max_tokens,
            *include_outgoing,
            *include_incoming,
            template.as_deref(),
            input_json.as_ref(),
            cli.db.as_deref(),
        )
        .map(|()| ExitCode::SUCCESS),
        Command::Resolve {
            target,
            tags,
            search,
            limit,
            fields,
        } => commands::resolve::run(
            &cfg,
            use_json,
            ndjson,
            no_header,
            count_only,
            target.as_deref(),
            tags.as_deref(),
            search.as_deref(),
            *limit,
            fields.as_deref(),
            cli.db.as_deref(),
        )
        .map(|()| ExitCode::SUCCESS),
        Command::Fix {
            broken_uuid,
            target,
            apply,
        } => commands::fix::run(
            &cfg,
            use_json,
            cli.verbose,
            broken_uuid,
            target,
            *apply,
            cli.db.as_deref(),
        )
        .map(|()| ExitCode::SUCCESS),
        Command::Suggest {
            target,
            limit,
            input_json,
        } => commands::suggest::run(
            &cfg,
            use_json,
            cli.verbose,
            target.as_deref(),
            *limit,
            input_json.as_ref(),
            cli.db.as_deref(),
        )
        .map(|()| ExitCode::SUCCESS),
        Command::New {
            title,
            create,
            tags,
            aliases,
        } => commands::new::run(
            &cfg,
            use_json,
            title,
            *create,
            tags.as_deref(),
            aliases.as_deref(),
            cli.db.as_deref(),
        )
        .map(|()| ExitCode::SUCCESS),
        Command::Get {
            target,
            depth,
            out,
            graph: show_graph,
            from_stdin,
            from_file,
            input_json,
        } => match load_get_params(input_json, target, *depth, *out, *show_graph) {
            Ok((t, d, o, g)) => commands::get::run(
                &cfg,
                use_json,
                cli.verbose,
                t.as_deref(),
                d,
                o,
                g,
                *from_stdin,
                from_file.as_ref(),
                cli.db.as_deref(),
            )
            .map(|()| ExitCode::SUCCESS),
            Err(e) => Err(e),
        },
        Command::Query {
            terms,
            tag,
            limit,
            input_json,
        } => commands::query::run(
            &cfg,
            use_json,
            ndjson,
            no_header,
            count_only,
            cli.verbose,
            terms.as_deref(),
            tag.as_deref(),
            *limit,
            input_json.as_ref(),
            cli.db.as_deref(),
        )
        .map(|()| ExitCode::SUCCESS),
        Command::Info => {
            commands::info::run(&cfg, use_json, cli.db.as_deref()).map(|()| ExitCode::SUCCESS)
        }
        Command::InitConfig { db } => init_config(db.as_deref(), cli.json),
        Command::Path {
            from,
            to,
            max_depth,
            input_json,
        } => match load_path_params(input_json, from, to, *max_depth) {
            Ok((f, t, md)) => commands::path::run(
                &cfg,
                use_json,
                cli.verbose,
                f.as_deref(),
                t.as_deref(),
                md,
                cli.db.as_deref(),
            )
            .map(|()| ExitCode::SUCCESS),
            Err(e) => Err(e),
        },
        Command::Subgraph {
            target,
            depth,
            input_json,
        } => commands::subgraph::run(
            &cfg,
            use_json,
            cli.verbose,
            target.as_deref(),
            *depth,
            input_json.as_ref(),
            cli.db.as_deref(),
        )
        .map(|()| ExitCode::SUCCESS),
        Command::Tags { tag } => commands::tags::run(
            &cfg,
            use_json,
            ndjson,
            no_header,
            count_only,
            cli.verbose,
            tag.as_deref(),
            cli.db.as_deref(),
        )
        .map(|()| ExitCode::SUCCESS),
    };

    match result {
        Ok(code) => code,
        Err(e) => {
            if machine {
                println!("{}", serde_json::json!({"error": e.to_string()}));
            } else {
                eprintln!("Error: {e}");
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
        let t = params
            .get("target")
            .and_then(|v| v.as_str().map(std::string::ToString::to_string));
        let d = params
            .get("depth")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(u64::from(depth)) as u32;
        let o = params
            .get("out")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(out);
        let g = params
            .get("graph")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(show_graph);
        Ok((t, d, o, g))
    } else {
        Ok((target.clone(), depth, out, show_graph))
    }
}

fn load_path_params(
    input_json: &Option<std::path::PathBuf>,
    from: &Option<String>,
    to: &Option<String>,
    max_depth: Option<u32>,
) -> Result<(Option<String>, Option<String>, Option<u32>)> {
    if let Some(json_path) = input_json {
        let content = std::fs::read_to_string(json_path)?;
        let params: serde_json::Value = serde_json::from_str(&content)?;
        let f = params
            .get("from")
            .and_then(|v| v.as_str().map(std::string::ToString::to_string));
        let t = params
            .get("to")
            .and_then(|v| v.as_str().map(std::string::ToString::to_string));
        let md = params
            .get("max_depth")
            .and_then(serde_json::Value::as_u64)
            .map(|v| v as u32)
            .or(max_depth);
        Ok((f, t, md))
    } else {
        Ok((from.clone(), to.clone(), max_depth))
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
        println!(
            "{}",
            serde_json::json!({"created": config_path.to_string_lossy()})
        );
    } else {
        println!("Created config at {}", config_path.display());
    }
    Ok(ExitCode::SUCCESS)
}
