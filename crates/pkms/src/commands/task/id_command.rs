use crate::command_context::CommandContext;
use crate::editor::DEFAULT_EDITOR;
use anyhow::{Context, Result, bail};
use std::process::ExitCode;

pub(super) fn run(
    ctx: &CommandContext<'_>,
    args: &[String],
    runtime: super::TaskRuntime<'_>,
) -> Result<ExitCode> {
    let Some((id, rest)) = args.split_first() else {
        bail!("Expected task ID and subcommand");
    };
    let Some((command, command_args)) = rest.split_first() else {
        bail!("Expected subcommand after task ID. Use: pkms task <ID> <SUBCOMMAND>");
    };

    match command.as_str() {
        "show" => {
            expect_no_args("task show", command_args)?;
            super::run_show(ctx, id).map(|()| ExitCode::SUCCESS)
        }
        "open" => {
            let args = parse_open_args(command_args)?;
            super::run_open(ctx, id, &args.editor, args.line).map(|()| ExitCode::SUCCESS)
        }
        "state" => {
            let args = parse_state_args(command_args)?;
            super::mutations::run_state(runtime, id, &args.state, args.dry_run)
                .map(|()| ExitCode::SUCCESS)
        }
        "done" => {
            let dry_run = parse_done_args(command_args)?;
            super::mutations::run_done(runtime, id, dry_run).map(|()| ExitCode::SUCCESS)
        }
        "postpone" => {
            let to = parse_postpone_args(command_args)?;
            super::mutations::run_postpone(runtime, id, to.as_deref()).map(|()| ExitCode::SUCCESS)
        }
        "mod" => {
            parse_mod_args(command_args)?;
            super::mutations::run_mod(runtime, id, command_args)
        }
        other => bail!(
            "Unknown task subcommand '{other}' after ID. Expected one of: show, open, state, done, postpone, mod"
        ),
    }
}

fn expect_no_args(command: &str, raw: &[String]) -> Result<()> {
    if let Some(arg) = raw.first() {
        bail!("Unexpected argument for {command}: {arg}");
    }
    Ok(())
}

struct ParsedOpenArgs {
    editor: String,
    line: Option<usize>,
}

fn parse_open_args(raw: &[String]) -> Result<ParsedOpenArgs> {
    let mut editor = DEFAULT_EDITOR.to_string();
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
    Ok(ParsedOpenArgs { editor, line })
}

struct ParsedStateArgs {
    state: String,
    dry_run: bool,
}

fn parse_state_args(raw: &[String]) -> Result<ParsedStateArgs> {
    let mut state = None;
    let mut dry_run = false;
    for arg in raw {
        match arg.as_str() {
            "--dry-run" => dry_run = true,
            other if state.is_none() => state = Some(other.to_string()),
            other => bail!("Unexpected argument for task state: {other}"),
        }
    }
    Ok(ParsedStateArgs {
        state: state.context("Expected TODO state after task state")?,
        dry_run,
    })
}

fn parse_done_args(raw: &[String]) -> Result<bool> {
    let mut dry_run = false;
    for arg in raw {
        match arg.as_str() {
            "--dry-run" => dry_run = true,
            other => bail!("Unexpected argument for task done: {other}"),
        }
    }
    Ok(dry_run)
}

fn parse_postpone_args(raw: &[String]) -> Result<Option<String>> {
    parse_optional_value_option(raw, "--to", "task postpone")
}

fn parse_mod_args(raw: &[String]) -> Result<()> {
    if raw.is_empty() {
        bail!("Expected at least one task modifier after task mod");
    }
    Ok(())
}

fn parse_optional_value_option(
    raw: &[String],
    option: &str,
    command: &str,
) -> Result<Option<String>> {
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
                    .with_context(|| format!("Expected value after {arg}"))?,
            );
        } else {
            bail!("Unexpected argument for {command}: {arg}");
        }
    }
    Ok(value)
}
