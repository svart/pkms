use crate::app::{self, App};
use crate::cli::{Cli, Command, OutputFormat};
use crate::command_context::CommandContext;
use crate::commands;
use crate::config;
use crate::input;
use crate::output::OutputContext;
use anyhow::Result;
use std::process::ExitCode;

pub fn run(cli: Cli) -> ExitCode {
    tracing::debug!(command = cli.command.name(), "dispatching command");
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
            tracing::debug!(command = cli.command.name(), "command completed");
            code
        }
        Err(e) => {
            tracing::error!(command = cli.command.name(), error = %e, "command failed");
            app::command_error(&app.output, e)
        }
    }
}

fn dispatch(cli: &Cli, command_ctx: &CommandContext<'_>) -> Result<ExitCode> {
    let ctx = command_ctx.output();
    Ok(match &cli.command {
        Command::Check(args) => {
            commands::check::run(command_ctx, &commands::check::CheckOptions::from(args))?
        }
        Command::Validate(args) => {
            let targets = input::resolve_targets(&args.target, args.from_stdin)?;
            success(commands::validate::run(
                command_ctx,
                &commands::validate::ValidateOptions { targets },
            ))?
        }
        Command::Stats(args) => success(commands::stats::run(
            command_ctx,
            &commands::stats::options_from_args(args),
        ))?,
        Command::Orphans(args) => success(commands::orphans::run(
            command_ctx,
            &commands::orphans::options_from_args(args),
        ))?,
        Command::Info => success(commands::info::run(command_ctx))?,
        Command::InitConfig(args) => success(init_config(args.db.as_deref(), ctx))?,
        Command::Resolve(args) => success(commands::resolve::run(
            command_ctx,
            &commands::resolve::options_from_args(args),
        ))?,
        Command::Fix(args) => success(commands::fix::run(command_ctx, args))?,
        Command::Suggest(args) => {
            let targets = input::resolve_targets(&args.target, args.from_stdin)?;
            success(commands::suggest::run(
                command_ctx,
                &commands::suggest::SuggestOptions {
                    targets,
                    limit: args.limit,
                    exclude_orphans: args.exclude_orphans,
                },
            ))?
        }
        Command::New(args) => success(commands::new::run(
            command_ctx,
            &commands::new::options_from_args(args),
        ))?,
        Command::Extract(args) => success(commands::extract::run(command_ctx, &args.try_into()?))?,
        Command::Get(args) => {
            let targets = input::resolve_targets(&args.target, args.from_stdin)?;
            success(commands::get::run(
                command_ctx,
                &commands::get::GetOptions {
                    targets,
                    show_links: args.links,
                    show_headings: args.headings,
                    heading: args.heading.clone(),
                    no_content: args.no_content,
                    encoding: args.encoding,
                },
            ))?
        }
        Command::Query(args) => success(commands::query::run(
            command_ctx,
            &commands::query::options_from_args(args)?,
        ))?,
        Command::Task(args) => commands::task::run(command_ctx, &args.command)?,
        Command::Path(args) => success(commands::path::run(
            command_ctx,
            &commands::path::options_from_args(args),
        ))?,
        #[cfg(feature = "web")]
        Command::Serve(args) => success(commands::serve::run(
            command_ctx,
            &commands::serve::ServeOptions {
                target: args.target.clone(),
                host: args.host.clone(),
                port: args.port,
            },
        ))?,
    })
}

fn success(result: Result<()>) -> Result<ExitCode> {
    result.map(|()| ExitCode::SUCCESS)
}

fn init_config(db: Option<&std::path::Path>, ctx: &OutputContext) -> Result<()> {
    let config_path = dirs::config_dir()
        .ok_or_else(|| anyhow::anyhow!("Could not find XDG config directory"))?
        .join("pkms.toml");
    if config_path.exists() {
        anyhow::bail!("Config already exists at {}", config_path.display());
    }
    let content = config::generate_default_config(db);
    std::fs::create_dir_all(config_path.parent().unwrap())?;
    std::fs::write(&config_path, &content)?;
    if ctx.is_structured() {
        ctx.print_structured(&serde_json::json!({"created": config_path.to_string_lossy()}))?;
    } else {
        println!("Created config at {}", config_path.display());
    }
    Ok(())
}
