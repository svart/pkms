use crate::cli::{
    TaskDeadlineArgs, TaskDoneArgs, TaskOpenArgs, TaskPostponeArgs, TaskScheduleArgs,
    TaskStateArgs, TaskTargetArgs,
};
use crate::config::ResolvedConfig;
use crate::output::OutputContext;
use anyhow::{Context, Result, bail};

pub(super) fn run(config: &ResolvedConfig, ctx: &OutputContext, args: &[String]) -> Result<()> {
    let Some((id, rest)) = args.split_first() else {
        bail!("Expected task ID and subcommand");
    };
    let Some((command, command_args)) = rest.split_first() else {
        bail!("Expected subcommand after task ID. Use: pkms task <ID> <SUBCOMMAND>");
    };

    match command.as_str() {
        "show" => {
            let args = parse_show_args(id, command_args)?;
            super::run_show(config, ctx, &args)
        }
        "open" => {
            let args = parse_open_args(id, command_args)?;
            super::run_open(config, ctx, &args)
        }
        "state" => {
            let args = parse_state_args(id, command_args)?;
            super::run_state(config, ctx, &args)
        }
        "done" => {
            let args = parse_done_args(id, command_args)?;
            super::run_done(config, ctx, &args)
        }
        "postpone" => {
            let args = parse_postpone_args(id, command_args)?;
            super::run_postpone(config, ctx, &args)
        }
        "schedule" => {
            let args = parse_schedule_args(id, command_args)?;
            super::run_schedule(config, ctx, &args)
        }
        "deadline" => {
            let args = parse_deadline_args(id, command_args)?;
            super::run_deadline(config, ctx, &args)
        }
        other => bail!(
            "Unknown task subcommand '{other}' after ID. Expected one of: show, open, state, done, postpone, schedule, deadline"
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

fn parse_schedule_args(id: &str, raw: &[String]) -> Result<TaskScheduleArgs> {
    let due = parse_single_value_option(raw, "--due", "task schedule")?;
    Ok(TaskScheduleArgs {
        id: id.to_string(),
        due,
    })
}

fn parse_deadline_args(id: &str, raw: &[String]) -> Result<TaskDeadlineArgs> {
    let deadline = parse_single_value_option(raw, "--deadline", "task deadline")?;
    Ok(TaskDeadlineArgs {
        id: id.to_string(),
        deadline,
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
