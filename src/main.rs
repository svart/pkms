mod cli;
mod commands;
mod config;
mod discovery;
#[cfg(feature = "embed")]
mod embed;
mod graph;
mod org_date;
mod output;
mod parser;
mod tokens;
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
    Ok(match &cli.command {
        Command::Check {
            stats,
            file_links,
            attachment_links,
            id_links,
            filetags,
            agenda,
        } => commands::check::run(
            cfg,
            ctx,
            cli.db.as_deref(),
            *stats,
            *file_links,
            *attachment_links,
            *id_links,
            *filetags,
            *agenda,
        )?,
        Command::Validate { target, from_stdin } => {
            commands::validate::run(cfg, ctx, target.as_deref(), *from_stdin, cli.db.as_deref())
                .map(|()| ExitCode::SUCCESS)?
        }
        Command::Stats {
            days,
            hubs,
            tags,
            todos,
        } => commands::stats::run(cfg, ctx, *days, *hubs, *tags, *todos, cli.db.as_deref())
            .map(|()| ExitCode::SUCCESS)?,
        Command::Orphans {
            limit,
            with_dailies,
        } => commands::orphans::run(cfg, ctx, *limit, *with_dailies, cli.db.as_deref())
            .map(|()| ExitCode::SUCCESS)?,
        Command::Info => {
            commands::info::run(cfg, ctx, cli.db.as_deref()).map(|()| ExitCode::SUCCESS)?
        }
        Command::InitConfig { db } => init_config(db.as_deref(), ctx)?,
        Command::Context {
            target,
            depth,
            max_tokens,
            encoding,
            from_stdin,
        } => commands::context::run(
            cfg,
            ctx,
            &commands::context::ContextOptions {
                target: target.as_deref(),
                depth: *depth,
                max_tokens: *max_tokens,
                encoding: tokens::Encoding::from_str(encoding)
                    .ok_or_else(|| anyhow::anyhow!("Unknown encoding: {encoding}"))?,
                from_stdin: *from_stdin,
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
            todos,
        } => commands::resolve::run(
            cfg,
            ctx,
            &commands::resolve::ResolveOptions {
                uuid: uuid.as_deref(),
                title: title.as_deref(),
                tags: tags.as_deref(),
                limit: *limit,
                fields: fields.as_deref(),
                todos: *todos,
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
        #[cfg(feature = "embed")]
        Command::Suggest {
            target,
            limit,
            exclude_orphans,
            from_stdin,
            embed,
        } => commands::suggest::run(
            cfg,
            ctx,
            target.as_deref(),
            *limit,
            *exclude_orphans,
            *from_stdin,
            *embed,
            cli.db.as_deref(),
        )
        .map(|()| ExitCode::SUCCESS)?,
        #[cfg(not(feature = "embed"))]
        Command::Suggest {
            target,
            limit,
            exclude_orphans,
            from_stdin,
            ..
        } => commands::suggest::run(
            cfg,
            ctx,
            target.as_deref(),
            *limit,
            *exclude_orphans,
            *from_stdin,
            false,
            cli.db.as_deref(),
        )
        .map(|()| ExitCode::SUCCESS)?,
        Command::New {
            title,
            create,
            tags,
            aliases,
            heading,
        } => commands::new::run(
            cfg,
            ctx,
            title,
            *create,
            tags.as_deref(),
            aliases.as_deref(),
            heading.as_deref(),
            cli.db.as_deref(),
        )
        .map(|()| ExitCode::SUCCESS)?,
        Command::Get {
            target,
            links,
            headings,
            no_content,
            from_stdin,
        } => commands::get::run(
            cfg,
            ctx,
            &commands::get::GetOptions {
                target: target.as_deref(),
                show_links: *links,
                show_headings: *headings,
                no_content: *no_content,
                from_stdin: *from_stdin,
            },
            cli.db.as_deref(),
        )
        .map(|()| ExitCode::SUCCESS)?,
        #[cfg(feature = "embed")]
        Command::Query {
            terms,
            limit,
            tags,
            title,
            content,
            todos,
            embed,
        } => commands::query::run(
            cfg,
            ctx,
            terms.as_deref(),
            *limit,
            *tags,
            *title,
            *content,
            *todos,
            *embed,
            cli.db.as_deref(),
        )
        .map(|()| ExitCode::SUCCESS)?,
        #[cfg(not(feature = "embed"))]
        Command::Query {
            terms,
            limit,
            tags,
            title,
            content,
            todos,
            ..
        } => commands::query::run(
            cfg,
            ctx,
            terms.as_deref(),
            *limit,
            *tags,
            *title,
            *content,
            *todos,
            false,
            cli.db.as_deref(),
        )
        .map(|()| ExitCode::SUCCESS)?,
        Command::Agenda {
            missing_agenda,
            include,
            exclude,
            overdue,
            date,
            sort,
            limit,
            today,
            week,
        } => commands::agenda::run(
            cfg,
            ctx,
            *missing_agenda,
            include.as_deref(),
            exclude.as_deref(),
            *overdue,
            date.as_deref(),
            sort.as_deref(),
            *limit,
            *today,
            *week,
            cli.db.as_deref(),
        )
        .map(|()| ExitCode::SUCCESS)?,
        Command::Path { from, to } => {
            commands::path::run(cfg, ctx, from.as_deref(), to.as_deref(), cli.db.as_deref())
                .map(|()| ExitCode::SUCCESS)?
        }
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
