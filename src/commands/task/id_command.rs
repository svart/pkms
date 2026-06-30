use crate::cli::{
    TaskDoneArgs, TaskModArgs, TaskOpenArgs, TaskPostponeArgs, TaskStateArgs, TaskTargetArgs,
};
use crate::command_context::CommandContext;
use anyhow::{Context, Result, bail};
use std::process::ExitCode;

pub(super) fn run(ctx: &CommandContext<'_>, args: &[String]) -> Result<ExitCode> {
    let config = ctx.config();
    let output = ctx.output();
    let Some((id, rest)) = args.split_first() else {
        bail!("Expected task ID and subcommand");
    };
    let Some((command, command_args)) = rest.split_first() else {
        bail!("Expected subcommand after task ID. Use: pkms task <ID> <SUBCOMMAND>");
    };

    match command.as_str() {
        "show" => {
            let args = parse_show_args(id, command_args)?;
            super::run_show(ctx, &args).map(|()| ExitCode::SUCCESS)
        }
        "open" => {
            let args = parse_open_args(id, command_args)?;
            super::run_open(ctx, &args).map(|()| ExitCode::SUCCESS)
        }
        "state" => {
            let args = parse_state_args(id, command_args)?;
            super::mutations::run_state(config, output, &args).map(|()| ExitCode::SUCCESS)
        }
        "done" => {
            let args = parse_done_args(id, command_args)?;
            super::mutations::run_done(config, output, &args).map(|()| ExitCode::SUCCESS)
        }
        "postpone" => {
            let args = parse_postpone_args(id, command_args)?;
            super::mutations::run_postpone(config, output, &args).map(|()| ExitCode::SUCCESS)
        }
        "mod" => {
            let args = parse_mod_args(id, command_args)?;
            super::mutations::run_mod(config, output, &args)
        }
        other => bail!(
            "Unknown task subcommand '{other}' after ID. Expected one of: show, open, state, done, postpone, mod"
        ),
    }
}

fn parse_show_args(id: &str, raw: &[String]) -> Result<TaskTargetArgs> {
    if let Some(arg) = raw.first() {
        bail!("Unexpected argument for task show: {arg}");
    }
    Ok(TaskTargetArgs { id: id.to_string() })
}

fn parse_open_args(id: &str, raw: &[String]) -> Result<TaskOpenArgs> {
    let mut editor = "emacsclient -n".to_string();
    let mut line = None;
    let mut iter = raw.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--editor" => {
                editor = iter
                    .next()
                    .cloned()
                    .context("Expected value after --editor")?;
            }
            "--line" | "-l" => {
                let value = iter.next().context("Expected value after --line")?;
                line = Some(
                    value
                        .parse::<usize>()
                        .with_context(|| format!("Invalid line number: {value}"))?,
                );
            }
            other => bail!("Unexpected argument for task open: {other}"),
        }
    }
    Ok(TaskOpenArgs {
        id: id.to_string(),
        editor,
        line,
    })
}

fn parse_state_args(id: &str, raw: &[String]) -> Result<TaskStateArgs> {
    let mut state = None;
    let mut dry_run = false;
    for arg in raw {
        match arg.as_str() {
            "--dry-run" => dry_run = true,
            other if state.is_none() => state = Some(other.to_string()),
            other => bail!("Unexpected argument for task state: {other}"),
        }
    }
    Ok(TaskStateArgs {
        id: id.to_string(),
        state: state.context("Expected TODO state after task state")?,
        dry_run,
    })
}

fn parse_done_args(id: &str, raw: &[String]) -> Result<TaskDoneArgs> {
    let mut dry_run = false;
    for arg in raw {
        match arg.as_str() {
            "--dry-run" => dry_run = true,
            other => bail!("Unexpected argument for task done: {other}"),
        }
    }
    Ok(TaskDoneArgs {
        id: id.to_string(),
        dry_run,
    })
}

fn parse_postpone_args(id: &str, raw: &[String]) -> Result<TaskPostponeArgs> {
    let to = parse_single_value_option(raw, "--to", "task postpone")?;
    Ok(TaskPostponeArgs {
        id: id.to_string(),
        to,
    })
}

fn parse_mod_args(id: &str, raw: &[String]) -> Result<TaskModArgs> {
    if raw.is_empty() {
        bail!("Expected at least one task modifier after task mod");
    }
    Ok(TaskModArgs {
        id: id.to_string(),
        modifiers: raw.to_vec(),
    })
}

fn parse_single_value_option(raw: &[String], option: &str, command: &str) -> Result<String> {
    let mut value = None;
    let mut iter = raw.iter();
    while let Some(arg) = iter.next() {
        if arg == option {
            if value.is_some() {
                bail!("Option {option} can only be provided once");
            }
            value = Some(
                iter.next()
                    .cloned()
                    .with_context(|| format!("Expected value after {option}"))?,
            );
        } else {
            bail!("Unexpected argument for {command}: {arg}");
        }
    }
    value.with_context(|| format!("Expected {option} for {command}"))
}
