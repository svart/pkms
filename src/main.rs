mod cli;
mod commands;
mod config;
mod discovery;
mod graph;
mod output;
mod parser;
mod util;

use anyhow::Result;
use clap::Parser;
use cli::{Cli, Command, OutputFormat};
use output::OutputContext;
use std::process::ExitCode;

fn main() -> ExitCode {
    let cli = Cli::parse();
    let ctx = OutputContext {
        format: cli.output_format.clone().unwrap_or(OutputFormat::Text),
    };
    let machine = ctx.is_json();

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

    match dispatch(&cli, &cfg, &ctx) {
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

fn dispatch(cli: &Cli, cfg: &config::Config, ctx: &OutputContext) -> Result<ExitCode> {
    if let Some(result) = dispatch_simple(cli, cfg, ctx)? {
        return Ok(result);
    }
    dispatch_complex(cli, cfg, ctx)
}

fn dispatch_simple(
    cli: &Cli,
    cfg: &config::Config,
    ctx: &OutputContext,
) -> Result<Option<ExitCode>> {
    Ok(Some(match &cli.command {
        Command::Check {
            file_links,
            attachment_links,
            id_links,
        } => commands::check::run(
            cfg,
            ctx,
            cli.db.as_deref(),
            *file_links,
            *attachment_links,
            *id_links,
        )?,
        Command::Validate { target } => {
            commands::validate::run(cfg, ctx, target.as_deref(), cli.db.as_deref())
                .map(|()| ExitCode::SUCCESS)?
        }
        Command::Stats { days, hubs, tags } => {
            commands::stats::run(cfg, ctx, *days, *hubs, *tags, cli.db.as_deref())
                .map(|()| ExitCode::SUCCESS)?
        }
        Command::Orphans { limit } => commands::orphans::run(cfg, ctx, *limit, cli.db.as_deref())
            .map(|()| ExitCode::SUCCESS)?,
        Command::Info => {
            commands::info::run(cfg, ctx, cli.db.as_deref()).map(|()| ExitCode::SUCCESS)?
        }
        Command::InitConfig { db } => init_config(db.as_deref(), ctx)?,
        _ => return Ok(None),
    }))
}

fn dispatch_complex(cli: &Cli, cfg: &config::Config, ctx: &OutputContext) -> Result<ExitCode> {
    if let Some(result) = dispatch_mutating(cli, cfg, ctx)? {
        return Ok(result);
    }
    dispatch_query(cli, cfg, ctx)
}

fn dispatch_mutating(
    cli: &Cli,
    cfg: &config::Config,
    ctx: &OutputContext,
) -> Result<Option<ExitCode>> {
    Ok(Some(match &cli.command {
        Command::Context {
            target,
            depth,
            max_tokens,
        } => commands::context::run(
            cfg,
            ctx,
            &commands::context::ContextOptions {
                target: target.as_deref(),
                depth: *depth,
                max_tokens: *max_tokens,
            },
            cli.db.as_deref(),
        )
        .map(|()| ExitCode::SUCCESS)?,
        Command::Resolve {
            uuid,
            title,
            tags,
            limit,
            fields,
        } => commands::resolve::run(
            cfg,
            ctx,
            &commands::resolve::ResolveOptions {
                uuid: uuid.as_deref(),
                title: title.as_deref(),
                tags: tags.as_deref(),
                limit: *limit,
                fields: fields.as_deref(),
            },
            cli.db.as_deref(),
        )
        .map(|()| ExitCode::SUCCESS)?,
        Command::Fix {
            broken_uuid,
            target,
            apply,
        } => commands::fix::run(cfg, ctx, broken_uuid, target, *apply, cli.db.as_deref())
            .map(|()| ExitCode::SUCCESS)?,
        Command::Suggest {
            target,
            limit,
            exclude_orphans,
        } => commands::suggest::run(
            cfg,
            ctx,
            target.as_deref(),
            *limit,
            *exclude_orphans,
            cli.db.as_deref(),
        )
        .map(|()| ExitCode::SUCCESS)?,
        Command::New {
            title,
            create,
            tags,
            aliases,
        } => commands::new::run(
            cfg,
            ctx,
            title,
            *create,
            tags.as_deref(),
            aliases.as_deref(),
            cli.db.as_deref(),
        )
        .map(|()| ExitCode::SUCCESS)?,
        Command::Get {
            target,
            links,
            no_content,
        } => commands::get::run(
            cfg,
            ctx,
            &commands::get::GetOptions {
                target: target.as_deref(),
                show_links: *links,
                no_content: *no_content,
            },
            cli.db.as_deref(),
        )
        .map(|()| ExitCode::SUCCESS)?,
        _ => return Ok(None),
    }))
}

fn dispatch_query(cli: &Cli, cfg: &config::Config, ctx: &OutputContext) -> Result<ExitCode> {
    Ok(match &cli.command {
        Command::Query {
            terms,
            limit,
            tags,
            title,
            content,
        } => commands::query::run(
            cfg,
            ctx,
            terms.as_deref(),
            *limit,
            *tags,
            *title,
            *content,
            cli.db.as_deref(),
        )
        .map(|()| ExitCode::SUCCESS)?,
        Command::Path { from, to } => {
            commands::path::run(cfg, ctx, from.as_deref(), to.as_deref(), cli.db.as_deref())
                .map(|()| ExitCode::SUCCESS)?
        }
        _ => unreachable!(),
    })
}

fn init_config(db: Option<&std::path::Path>, ctx: &OutputContext) -> Result<ExitCode> {
    let config_path = dirs::config_dir()
        .ok_or_else(|| anyhow::anyhow!("Could not find XDG config directory"))?
        .join("pkms.toml");
    if config_path.exists() {
        anyhow::bail!("Config already exists at {}", config_path.display());
    }
    let content = config::generate_default_config(db);
    std::fs::create_dir_all(config_path.parent().unwrap())?;
    std::fs::write(&config_path, &content)?;
    if ctx.is_json() {
        println!(
            "{}",
            serde_json::json!({"created": config_path.to_string_lossy()})
        );
    } else {
        println!("Created config at {}", config_path.display());
    }
    Ok(ExitCode::SUCCESS)
}
