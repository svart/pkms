use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "pkms", version, about = "org-roam PKMS navigation and validation tool")]
pub struct Cli {
    #[arg(global = true, long, value_name = "PATH", help = "Path to org-roam database root (overrides config)")]
    pub db: Option<PathBuf>,

    #[arg(global = true, long, help = "Structured JSON output")]
    pub json: bool,

    #[arg(global = true, short, long, help = "Verbose output")]
    pub verbose: bool,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    #[command(about = "Verify health of the entire org-roam database")]
    Check,
    #[command(about = "Validate health of a specific note")]
    Validate {
        #[arg(help = "UUID, file path, or note title")]
        target: String,
    },
    #[command(about = "Generate a filename and UUID for a new note")]
    New {
        #[arg(help = "Title of the new note")]
        title: String,
        #[arg(long, help = "Actualy write the boilerplate file")]
        create: bool,
        #[arg(long, help = "Comma-separated list of filetags")]
        tags: Option<String>,
        #[arg(long, help = "Comma-separated list of aliases")]
        aliases: Option<String>,
    },
    #[command(about = "Retrieve a note with its neighbors at specified depth")]
    Get {
        #[arg(help = "UUID, file path, or note title")]
        target: String,
        #[arg(short, long, default_value = "1", help = "Traversal depth")]
        depth: u32,
        #[arg(short, long, help = "Show full note content")]
        out: bool,
    },
    #[command(about = "Fuzzy search across note titles and content")]
    Query {
        #[arg(help = "Search terms")]
        terms: String,
        #[arg(short, long, help = "Filter by filetag")]
        tag: Option<String>,
        #[arg(short, long, help = "Maximum results")]
        limit: Option<usize>,
    },
    #[command(about = "Show current pkms configuration")]
    Info,
    #[command(about = "Generate default config file at ~/.config/pkms.toml")]
    InitConfig {
        #[arg(short, long, help = "Database root path to write into config")]
        db: Option<PathBuf>,
    },
}
