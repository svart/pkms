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
mod tokens;
mod util;

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

fn dispatch(cli: &Cli, cfg: &config::Config, ctx: &OutputContext) -> Result<ExitCode> {
    Ok(match &cli.command {
        Command::Check(args) => commands::check::run(
            cfg,
            ctx,
            &commands::check::CheckOptions {
                stats: args.stats,
                file_links: args.file_links,
                attachment_links: args.attachment_links,
                id_links: args.id_links,
                filetags: args.filetags,
                agenda: args.agenda,
                self_links: args.self_links,
                overlinks: args.overlinks,
                cross_links: args.cross_links.clone(),
            },
        )?,
        Command::Validate(args) => {
            let targets = input::resolve_targets(&args.target, args.from_stdin)?;
            commands::validate::run(cfg, ctx, &commands::validate::ValidateOptions { targets })
                .map(|()| ExitCode::SUCCESS)?
        }
        Command::Stats(args) => commands::stats::run(
            cfg,
            ctx,
            &commands::stats::StatsOptions {
                days: args.days,
                hubs: args.hubs,
                tags: args.tags,
                todos: args.todos,
            },
        )
        .map(|()| ExitCode::SUCCESS)?,
        Command::Orphans(args) => commands::orphans::run(
            cfg,
            ctx,
            &commands::orphans::OrphansOptions {
                limit: args.limit,
                with_dailies: args.with_dailies,
            },
        )
        .map(|()| ExitCode::SUCCESS)?,
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
        Command::Resolve(args) => commands::resolve::run(
            cfg,
            ctx,
            &commands::resolve::ResolveOptions {
                uuid: args.uuid.clone(),
                title: args.title.clone(),
                tags: input::comma_list(args.tags.as_deref()),
                limit: args.limit,
                fields: input::comma_list(args.fields.as_deref()),
                todos: args.todos,
            },
        )
        .map(|()| ExitCode::SUCCESS)?,
        Command::Fix(args) => {
            let broken = uuid::Uuid::parse_str(&args.broken_uuid)
                .map_err(|_| anyhow::anyhow!("Invalid UUID format: {}", args.broken_uuid))?
                .to_string();
            let target_uuid = uuid::Uuid::parse_str(&args.target)
                .map_err(|_| anyhow::anyhow!("Invalid UUID format: {}", args.target))?
                .to_string();
            commands::fix::run(
                cfg,
                ctx,
                &commands::fix::FixOptions {
                    broken_uuid: broken,
                    target_uuid,
                    apply: args.apply,
                },
            )
            .map(|()| ExitCode::SUCCESS)?
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
        Command::New(args) => commands::new::run(
            cfg,
            ctx,
            &commands::new::NewOptions {
                title: args.title.clone(),
                create: args.create,
                tags: input::comma_list(args.tags.as_deref()),
                aliases: input::comma_list(args.aliases.as_deref()),
                heading: args.heading.clone(),
            },
        )
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
            #[cfg(not(feature = "embed"))]
            let embed = &false;
            #[cfg(feature = "embed")]
            let embed = &args.embed;
            let terms = args
                .terms
                .clone()
                .ok_or_else(|| anyhow::anyhow!("No search terms specified. Provide terms"))?;
            commands::query::run(
                cfg,
                ctx,
                &commands::query::QueryOptions {
                    terms,
                    limit: args.limit,
                    tags: args.tags,
                    title: args.title,
                    content: args.content,
                    todos: args.todos,
                    embed: *embed,
                },
            )
            .map(|()| ExitCode::SUCCESS)?
        }
        Command::Todo(args) => {
            let resolved_scope = if args.from_stdin {
                util::read_stdin_ndjson()?
            } else {
                args.scope.clone().unwrap_or_default()
            };
            let todo_cols = input::resolve_columns(args.table.columns.as_deref(), &cfg.columns);
            commands::todo::run(
                cfg,
                ctx,
                &commands::todo::TodoOptions {
                    state: args.filters.state.clone(),
                    tags: args.filters.tags.clone(),
                    kind: args.filters.kind.clone(),
                    sort: args.sort.clone(),
                    limit: args.limit,
                    group: args.group.clone(),
                    scope: resolved_scope,
                    after: input::parse_datetime(args.after.as_deref()),
                    before: input::parse_datetime(args.before.as_deref()),
                    prio: args.prio.clone(),
                    line_sep: args.table.line_sep,
                    columns: todo_cols,
                },
            )
            .map(|()| ExitCode::SUCCESS)?
        }
        Command::Agenda(args) => {
            let agenda_cols = input::resolve_columns(args.table.columns.as_deref(), &cfg.columns);
            commands::agenda::run(
                cfg,
                ctx,
                &commands::agenda::AgendaOptions {
                    state: args.filters.state.clone(),
                    tags: args.filters.tags.clone(),
                    kind: args.filters.kind.clone(),
                    prio: args.prio.clone(),
                    overdue: args.overdue,
                    upcoming: args.upcoming,
                    date: input::parse_date(args.date.as_deref()),
                    sort: args.sort.clone(),
                    limit: args.limit,
                    today: args.today,
                    week: args.week,
                    line_sep: args.table.line_sep,
                    columns: agenda_cols,
                },
            )
            .map(|()| ExitCode::SUCCESS)?
        }
        Command::Path(args) => {
            let from = args
                .from
                .clone()
                .ok_or_else(|| anyhow::anyhow!("No source specified. Provide --from"))?;
            let to = args
                .to
                .clone()
                .ok_or_else(|| anyhow::anyhow!("No target specified. Provide --to"))?;
            commands::path::run(cfg, ctx, &commands::path::PathOptions { from, to })
                .map(|()| ExitCode::SUCCESS)?
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
