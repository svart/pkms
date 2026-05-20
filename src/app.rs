use crate::cli::{Cli, OutputFormat};
use crate::config::Config;
use crate::output::OutputContext;
use anyhow::Result;
use std::process::ExitCode;

pub struct App {
    pub config: Config,
    pub output: OutputContext,
}

impl App {
    pub fn from_cli(cli: &Cli) -> Result<Self> {
        let output = OutputContext {
            format: cli.output_format.clone().unwrap_or(OutputFormat::Text),
        };
        let config = Config::load()?.with_resolved_db_root(cli.db.clone())?;
        Ok(App { config, output })
    }
}

pub fn print_error(ctx: Option<&OutputContext>, err: &anyhow::Error) {
    if ctx.is_some_and(OutputContext::is_json) {
        println!("{}", serde_json::json!({"error": err.to_string()}));
    } else {
        eprintln!("Error: {err}");
    }
}

pub fn startup_error(err: anyhow::Error) -> ExitCode {
    eprintln!("Error: {err:#}");
    ExitCode::from(2)
}

pub fn command_error(ctx: &OutputContext, err: anyhow::Error) -> ExitCode {
    print_error(Some(ctx), &err);
    ExitCode::from(1)
}
