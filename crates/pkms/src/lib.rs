mod app;
mod cli;
mod command_context;
mod commands;
mod config;
mod editor;
mod environment;
mod input;
mod logging;
mod output;
mod runner;
mod util;

use clap::Parser;
use std::process::ExitCode;

/// Initializes the process, parses CLI arguments, and runs one command.
pub fn run() -> ExitCode {
    logging::init();
    runner::run(cli::Cli::parse())
}
