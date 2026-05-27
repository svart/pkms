use clap::Parser;
use pkms::cli::Cli;
use pkms::logging;
use std::process::ExitCode;

fn main() -> ExitCode {
    logging::init();
    let cli = Cli::parse();
    pkms::runner::run(cli)
}
