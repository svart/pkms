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
use output::{ALL_COLUMNS, Column, OutputContext};
use std::process::ExitCode;

fn main() -> ExitCode {
    let cli = Cli::parse();
    let ctx = OutputContext {
        format: cli.output_format.clone().unwrap_or(OutputFormat::Text),
    };
    let machine = ctx.is_json();

    let mut cfg = match config::Config::load() {
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

    let resolved = cli
        .db
        .clone()
        .or_else(|| {
            std::env::var("PKMS_DB_ROOT")
                .ok()
                .map(std::path::PathBuf::from)
        })
        .or_else(|| cfg.db_root.clone())
        .ok_or_else(|| {
            anyhow::anyhow!(
                "No database root specified. Provide --db PATH, set PKMS_DB_ROOT env var, \
                 or set db_root in ~/.config/pkms.toml"
            )
        })
        .map(|p| config::canonicalize_or_abs(&p));

    cfg.db_root = match resolved {
        Ok(db_root) => Some(db_root),
        Err(e) => {
            if machine {
                println!("{}", serde_json::json!({"error": e.to_string()}));
            } else {
                eprintln!("Error: {e}");
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
    use util::{is_stdin_piped, read_stdin_ndjson};

    Ok(match &cli.command {
        Command::Check {
            stats,
            file_links,
            attachment_links,
            id_links,
            filetags,
            agenda,
            self_links,
            overlinks,
            cross_links,
        } => commands::check::run(
            cfg,
            ctx,
            &commands::check::CheckOptions {
                stats: *stats,
                file_links: *file_links,
                attachment_links: *attachment_links,
                id_links: *id_links,
                filetags: *filetags,
                agenda: *agenda,
                self_links: *self_links,
                overlinks: *overlinks,
                cross_links: cross_links.clone(),
            },
        )?,
        Command::Validate { target, from_stdin } => {
            let targets = if *from_stdin || (target.is_none() && is_stdin_piped()) {
                read_stdin_ndjson()?
            } else if let Some(t) = target {
                vec![t.clone()]
            } else {
                anyhow::bail!(
                    "No target specified and no stdin pipe detected. \
                     Provide a target or use --from-stdin."
                );
            };
            commands::validate::run(cfg, ctx, &commands::validate::ValidateOptions { targets })
                .map(|()| ExitCode::SUCCESS)?
        }
        Command::Stats {
            days,
            hubs,
            tags,
            todos,
        } => commands::stats::run(
            cfg,
            ctx,
            &commands::stats::StatsOptions {
                days: *days,
                hubs: *hubs,
                tags: *tags,
                todos: *todos,
            },
        )
        .map(|()| ExitCode::SUCCESS)?,
        Command::Orphans {
            limit,
            with_dailies,
        } => commands::orphans::run(
            cfg,
            ctx,
            &commands::orphans::OrphansOptions {
                limit: *limit,
                with_dailies: *with_dailies,
            },
        )
        .map(|()| ExitCode::SUCCESS)?,
        Command::Info => commands::info::run(cfg, ctx).map(|()| ExitCode::SUCCESS)?,
        Command::InitConfig { db } => init_config(db.as_deref(), ctx)?,
        Command::Context {
            target,
            depth,
            max_tokens,
            encoding,
            from_stdin,
        } => {
            let targets = if *from_stdin || (target.is_none() && is_stdin_piped()) {
                read_stdin_ndjson()?
            } else if let Some(t) = target {
                vec![t.clone()]
            } else {
                anyhow::bail!(
                    "No target specified and no stdin pipe detected. \
                     Provide a target or use --from-stdin."
                );
            };
            commands::context::run(
                cfg,
                ctx,
                &commands::context::ContextOptions {
                    targets,
                    depth: *depth,
                    max_tokens: *max_tokens,
                    encoding: tokens::Encoding::from_str(encoding)
                        .ok_or_else(|| anyhow::anyhow!("Unknown encoding: {encoding}"))?,
                },
            )
            .map(|()| ExitCode::SUCCESS)?
        }
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
                uuid: uuid.clone(),
                title: title.clone(),
                tags: tags
                    .as_deref()
                    .map(|s| s.split(',').map(|s| s.trim().to_string()).collect()),
                limit: *limit,
                fields: fields
                    .as_deref()
                    .map(|s| s.split(',').map(|s| s.trim().to_string()).collect()),
                todos: *todos,
            },
        )
        .map(|()| ExitCode::SUCCESS)?,
        Command::Fix {
            broken_uuid,
            target,
            apply,
        } => {
            let broken = uuid::Uuid::parse_str(broken_uuid)
                .map_err(|_| anyhow::anyhow!("Invalid UUID format: {broken_uuid}"))?
                .to_string();
            let target_uuid = uuid::Uuid::parse_str(target)
                .map_err(|_| anyhow::anyhow!("Invalid UUID format: {target}"))?
                .to_string();
            commands::fix::run(
                cfg,
                ctx,
                &commands::fix::FixOptions {
                    broken_uuid: broken,
                    target_uuid,
                    apply: *apply,
                },
            )
            .map(|()| ExitCode::SUCCESS)?
        }
        #[cfg(feature = "embed")]
        Command::Suggest {
            target,
            limit,
            exclude_orphans,
            from_stdin,
            embed,
        } => {
            let targets = if *from_stdin || (target.is_none() && is_stdin_piped()) {
                read_stdin_ndjson()?
            } else if let Some(t) = target {
                vec![t.clone()]
            } else {
                anyhow::bail!(
                    "No target specified and no stdin pipe detected. \
                     Provide a target or use --from-stdin."
                );
            };
            commands::suggest::run(
                cfg,
                ctx,
                &commands::suggest::SuggestOptions {
                    targets,
                    limit: *limit,
                    exclude_orphans: *exclude_orphans,
                    use_embed: *embed,
                },
            )
            .map(|()| ExitCode::SUCCESS)?
        }
        #[cfg(not(feature = "embed"))]
        Command::Suggest {
            target,
            limit,
            exclude_orphans,
            from_stdin,
            ..
        } => {
            let targets = if *from_stdin || (target.is_none() && is_stdin_piped()) {
                read_stdin_ndjson()?
            } else if let Some(t) = target {
                vec![t.clone()]
            } else {
                anyhow::bail!(
                    "No target specified and no stdin pipe detected. \
                     Provide a target or use --from-stdin."
                );
            };
            commands::suggest::run(
                cfg,
                ctx,
                &commands::suggest::SuggestOptions {
                    targets,
                    limit: *limit,
                    exclude_orphans: *exclude_orphans,
                    use_embed: false,
                },
            )
            .map(|()| ExitCode::SUCCESS)?
        }
        Command::New {
            title,
            create,
            tags,
            aliases,
            heading,
        } => commands::new::run(
            cfg,
            ctx,
            &commands::new::NewOptions {
                title: title.clone(),
                create: *create,
                tags: tags
                    .as_deref()
                    .map(|s| s.split(',').map(|s| s.trim().to_string()).collect()),
                aliases: aliases
                    .as_deref()
                    .map(|s| s.split(',').map(|s| s.trim().to_string()).collect()),
                heading: heading.clone(),
            },
        )
        .map(|()| ExitCode::SUCCESS)?,
        Command::Get {
            target,
            links,
            headings,
            no_content,
            from_stdin,
        } => {
            let targets = if *from_stdin || (target.is_none() && is_stdin_piped()) {
                read_stdin_ndjson()?
            } else if let Some(t) = target {
                vec![t.clone()]
            } else {
                anyhow::bail!(
                    "No target specified and no stdin pipe detected. \
                     Provide a target or use --from-stdin."
                );
            };
            commands::get::run(
                cfg,
                ctx,
                &commands::get::GetOptions {
                    targets,
                    show_links: *links,
                    show_headings: *headings,
                    no_content: *no_content,
                },
            )
            .map(|()| ExitCode::SUCCESS)?
        }
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
            &commands::query::QueryOptions {
                terms: terms
                    .clone()
                    .ok_or_else(|| anyhow::anyhow!("No search terms specified. Provide terms"))?,
                limit: *limit,
                tags: *tags,
                title: *title,
                content: *content,
                todos: *todos,
                embed: *embed,
            },
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
            &commands::query::QueryOptions {
                terms: terms
                    .clone()
                    .ok_or_else(|| anyhow::anyhow!("No search terms specified. Provide terms"))?,
                limit: *limit,
                tags: *tags,
                title: *title,
                content: *content,
                todos: *todos,
                embed: false,
            },
        )
        .map(|()| ExitCode::SUCCESS)?,
        Command::Todo {
            state,
            tags,
            type_,
            sort,
            limit,
            group,
            scope,
            after,
            before,
            prio,
            from_stdin,
            line_sep,
            columns,
            open,
        } => {
            let resolved_scope = if *from_stdin {
                read_stdin_ndjson()?
            } else {
                scope.clone().unwrap_or_default()
            };
            let todo_cols = resolve_columns(columns.as_deref(), &cfg.columns);
            commands::todo::run(
                cfg,
                ctx,
                &commands::todo::TodoOptions {
                    state: state.clone(),
                    tags: tags.clone(),
                    type_: type_.clone(),
                    sort: sort.clone(),
                    limit: *limit,
                    group: group.clone(),
                    scope: resolved_scope,
                    after: after.as_deref().and_then(|d| {
                        chrono::NaiveDateTime::parse_from_str(d, "%Y-%m-%d %H:%M")
                            .ok()
                            .or_else(|| {
                                chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d")
                                    .ok()
                                    .map(|dt| dt.and_hms_opt(0, 0, 0).unwrap())
                            })
                    }),
                    before: before.as_deref().and_then(|d| {
                        chrono::NaiveDateTime::parse_from_str(d, "%Y-%m-%d %H:%M")
                            .ok()
                            .or_else(|| {
                                chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d")
                                    .ok()
                                    .map(|dt| dt.and_hms_opt(0, 0, 0).unwrap())
                            })
                    }),
                    prio: prio.clone(),
                    line_sep: *line_sep,
                    columns: todo_cols,
                    open: *open,
                },
            )
            .map(|()| ExitCode::SUCCESS)?
        }
        Command::Agenda {
            state,
            tags,
            type_,
            prio,
            overdue,
            date,
            sort,
            limit,
            today,
            week,
            upcoming,
            line_sep,
            columns,
            open,
        } => {
            let agenda_cols = resolve_columns(columns.as_deref(), &cfg.columns);
            commands::agenda::run(
                cfg,
                ctx,
                &commands::agenda::AgendaOptions {
                    state: state.clone(),
                    tags: tags.clone(),
                    type_: type_.clone(),
                    prio: prio.clone(),
                    overdue: *overdue,
                    upcoming: *upcoming,
                    date: date
                        .as_deref()
                        .and_then(|d| chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d").ok()),
                    sort: sort.clone(),
                    limit: *limit,
                    today: *today,
                    week: *week,
                    line_sep: *line_sep,
                    columns: agenda_cols,
                    open: *open,
                },
            )
            .map(|()| ExitCode::SUCCESS)?
        }
        Command::Path { from, to } => {
            let from = from
                .clone()
                .ok_or_else(|| anyhow::anyhow!("No source specified. Provide --from"))?;
            let to = to
                .clone()
                .ok_or_else(|| anyhow::anyhow!("No target specified. Provide --to"))?;
            commands::path::run(cfg, ctx, &commands::path::PathOptions { from, to })
                .map(|()| ExitCode::SUCCESS)?
        }
        Command::Show {
            target,
            uuid,
            from_stdin,
        } => {
            let targets = if *from_stdin {
                commands::show::read_stdin_targets()?
            } else if let Some(t) = target {
                vec![commands::show::HeadingTarget::from_arg(t.clone())?]
            } else if let Some(u) = uuid {
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

fn resolve_columns(cli_cols: Option<&str>, config_cols: &Option<Vec<String>>) -> Vec<Column> {
    if let Some(s) = cli_cols {
        let cols: Vec<Column> = s
            .split(',')
            .filter_map(|c| Column::from_str(c.trim()))
            .collect();
        if !cols.is_empty() {
            return cols;
        }
    }
    if let Some(names) = config_cols {
        let cols: Vec<Column> = names.iter().filter_map(|c| Column::from_str(c)).collect();
        if !cols.is_empty() {
            return cols;
        }
    }
    ALL_COLUMNS.to_vec()
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
