mod app;
mod cli;
mod commands;
mod config;
mod corpus;
mod discovery;
#[cfg(feature = "embed")]
mod embed;
mod graph;
mod input;
mod org_date;
mod output;
mod parser;
pub mod tasks;
mod tokens;
mod util;
mod workspace;

use anyhow::Result;
use app::App;
use clap::Parser;
use cli::{Cli, Command, OutputFormat};
use output::OutputContext;
use std::process::ExitCode;

fn main() -> ExitCode {
    let cli = Cli::parse();
    let app = match App::from_cli(&cli) {
        Ok(app) => app,
        Err(e) => {
            let ctx = OutputContext {
                format: cli.output_format.clone().unwrap_or(OutputFormat::Text),
            };
            return app::startup_error(&ctx, e);
        }
    };

    match dispatch(&cli, &app.config, &app.output) {
        Ok(code) => code,
        Err(e) => app::command_error(&app.output, e),
    }
}

fn dispatch(cli: &Cli, cfg: &config::ResolvedConfig, ctx: &OutputContext) -> Result<ExitCode> {
    Ok(match &cli.command {
        Command::Check(args) => {
            commands::check::run(cfg, ctx, &commands::check::CheckOptions::from(args))?
        }
        Command::Validate(args) => {
            let targets = input::resolve_targets(&args.target, args.from_stdin)?;
            commands::validate::run(cfg, ctx, &commands::validate::ValidateOptions { targets })
                .map(|()| ExitCode::SUCCESS)?
        }
        Command::Stats(args) => {
            commands::stats::run(cfg, ctx, &commands::stats::StatsOptions::from(args))
                .map(|()| ExitCode::SUCCESS)?
        }
        Command::Orphans(args) => {
            commands::orphans::run(cfg, ctx, &commands::orphans::OrphansOptions::from(args))
                .map(|()| ExitCode::SUCCESS)?
        }
        Command::Info => commands::info::run(cfg, ctx).map(|()| ExitCode::SUCCESS)?,
        Command::InitConfig(args) => init_config(args.db.as_deref(), ctx)?,
        Command::Context(args) => {
            let targets = input::resolve_targets(&args.target, args.from_stdin)?;
            commands::context::run(
                cfg,
                ctx,
                &commands::context::ContextOptions {
                    targets,
                    depth: args.depth,
                    max_tokens: args.max_tokens,
                    encoding: tokens::Encoding::from_str(&args.encoding)
                        .ok_or_else(|| anyhow::anyhow!("Unknown encoding: {}", args.encoding))?,
                },
            )
            .map(|()| ExitCode::SUCCESS)?
        }
        Command::Resolve(args) => {
            commands::resolve::run(cfg, ctx, &commands::resolve::ResolveOptions::from(args))
                .map(|()| ExitCode::SUCCESS)?
        }
        Command::Fix(args) => {
            commands::fix::run(cfg, ctx, &args.try_into()?).map(|()| ExitCode::SUCCESS)?
        }
        Command::Suggest(args) => {
            #[cfg(not(feature = "embed"))]
            let embed = &false;
            #[cfg(feature = "embed")]
            let embed = &args.embed;
            let targets = input::resolve_targets(&args.target, args.from_stdin)?;
            commands::suggest::run(
                cfg,
                ctx,
                &commands::suggest::SuggestOptions {
                    targets,
                    limit: args.limit,
                    exclude_orphans: args.exclude_orphans,
                    use_embed: *embed,
                },
            )
            .map(|()| ExitCode::SUCCESS)?
        }
        Command::New(args) => commands::new::run(cfg, ctx, &commands::new::NewOptions::from(args))
            .map(|()| ExitCode::SUCCESS)?,
        Command::Get(args) => {
            let targets = input::resolve_targets(&args.target, args.from_stdin)?;
            commands::get::run(
                cfg,
                ctx,
                &commands::get::GetOptions {
                    targets,
                    show_links: args.links,
                    show_headings: args.headings,
                    no_content: args.no_content,
                    encoding: tokens::Encoding::from_str(&args.encoding)
                        .ok_or_else(|| anyhow::anyhow!("Unknown encoding: {}", args.encoding))?,
                },
            )
            .map(|()| ExitCode::SUCCESS)?
        }
        Command::Query(args) => {
            commands::query::run(cfg, ctx, &args.try_into()?).map(|()| ExitCode::SUCCESS)?
        }
        Command::Task(args) => {
            commands::task::run(cfg, ctx, &args.command).map(|()| ExitCode::SUCCESS)?
        }
        Command::Path(args) => {
            commands::path::run(cfg, ctx, &args.try_into()?).map(|()| ExitCode::SUCCESS)?
        }
        Command::Open(args) => {
            let targets = input::resolve_targets(&args.target, args.from_stdin)?;
            commands::open::run(
                cfg,
                ctx,
                &commands::open::OpenOptions {
                    targets,
                    editor: args.editor.clone(),
                    line: args.line,
                },
            )
            .map(|()| ExitCode::SUCCESS)?
        }
        Command::Show(args) => {
            let targets = if args.from_stdin {
                commands::show::read_stdin_targets()?
            } else if let Some(t) = &args.target {
                vec![commands::show::HeadingTarget::from_arg(t.clone())?]
            } else if let Some(u) = &args.uuid {
                vec![commands::show::HeadingTarget {
                    note_target: u.clone(),
                    canonical_id: None,
                }]
            } else {
                anyhow::bail!(
                    "No target specified and no stdin pipe detected. \
                     Provide a target or use --from-stdin."
                );
            };
            commands::show::run(cfg, ctx, &commands::show::ShowOptions { targets })
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
