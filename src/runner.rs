use crate::app::{self, App};
use crate::cli::{Cli, Command, OutputFormat};
use crate::command_context::CommandContext;
use crate::commands;
use crate::config;
use crate::input;
use crate::output::OutputContext;
use crate::tokens;
use anyhow::Result;
use std::process::ExitCode;

pub fn run(cli: Cli) -> ExitCode {
    tracing::debug!(command = command_name(&cli.command), "dispatching command");
    let app = match App::from_cli(&cli) {
        Ok(app) => app,
        Err(e) => {
            let ctx = OutputContext {
                format: cli.output_format.clone().unwrap_or(OutputFormat::Text),
            };
            return app::startup_error(&ctx, e);
        }
    };

    let command_ctx = CommandContext::new(&app.config, &app.output);
    match dispatch(&cli, &command_ctx) {
        Ok(code) => {
            tracing::debug!(command = command_name(&cli.command), "command completed");
            code
        }
        Err(e) => {
            tracing::error!(command = command_name(&cli.command), error = %e, "command failed");
            app::command_error(&app.output, e)
        }
    }
}

fn command_name(command: &Command) -> &'static str {
    match command {
        Command::Check(_) => "check",
        Command::Validate(_) => "validate",
        Command::Stats(_) => "stats",
        Command::Orphans(_) => "orphans",
        Command::Context(_) => "context",
        Command::Resolve(_) => "resolve",
        Command::Fix(_) => "fix",
        Command::Suggest(_) => "suggest",
        Command::New(_) => "new",
        Command::Get(_) => "get",
        Command::Query(_) => "query",
        Command::Info => "info",
        Command::InitConfig(_) => "init-config",
        Command::Task(_) => "task",
        Command::Path(_) => "path",
        #[cfg(feature = "web")]
        Command::Serve(_) => "serve",
    }
}

fn dispatch(cli: &Cli, command_ctx: &CommandContext<'_>) -> Result<ExitCode> {
    let cfg = command_ctx.config();
    let ctx = command_ctx.output();
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
                    encoding: args
                        .encoding
                        .parse::<tokens::Encoding>()
                        .map_err(|_| anyhow::anyhow!("Unknown encoding: {}", args.encoding))?,
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
                    encoding: args
                        .encoding
                        .parse::<tokens::Encoding>()
                        .map_err(|_| anyhow::anyhow!("Unknown encoding: {}", args.encoding))?,
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
            commands::path::run(command_ctx, &args.try_into()?).map(|()| ExitCode::SUCCESS)?
        }
        #[cfg(feature = "web")]
        Command::Serve(args) => commands::serve::run(
            cfg,
            ctx,
            &commands::serve::ServeOptions {
                target: args.target.clone(),
                host: args.host.clone(),
                port: args.port,
            },
        )
        .map(|()| ExitCode::SUCCESS)?,
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
