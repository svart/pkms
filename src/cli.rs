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
    #[command(about = "Comprehensive database statistics")]
    Stats {
        #[arg(short, long, help = "Show notes modified in last N days")]
        days: Option<u32>,
    },
    #[command(about = "List orphan notes (no incoming or outgoing links)")]
    Orphans,
    #[command(about = "List broken/dangling links")]
    Broken,
    #[command(about = "List most-connected notes (hubs)")]
    Hubs {
        #[arg(short, long, default_value = "10", help = "Number of top hubs to show")]
        limit: usize,
    },
    #[command(about = "Build a context window for AI consumption")]
    Context {
        #[arg(help = "UUID, file path, or note title")]
        target: String,
        #[arg(short, long, default_value = "1", help = "Traversal depth")]
        depth: u32,
        #[arg(short, long, help = "Maximum tokens in output")]
        max_tokens: Option<usize>,
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
        #[arg(short, long, help = "Show ASCII graph visualization")]
        graph: bool,
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
    #[command(name = "path", about = "Find shortest path between two notes")]
    Path {
        #[arg(help = "Source note (UUID, path, or title)")]
        from: String,
        #[arg(help = "Target note (UUID, path, or title)")]
        to: String,
        #[arg(short, long, help = "Maximum traversal depth")]
        max_depth: Option<u32>,
    },
    #[command(about = "Export subgraph around a note")]
    Subgraph {
        #[arg(help = "Root note (UUID, path, or title)")]
        target: String,
        #[arg(short, long, default_value = "1", help = "Traversal depth")]
        depth: u32,
    },
    #[command(about = "List all filetags with note counts")]
    Tags {
        #[arg(short, long, help = "List all notes with this tag")]
        tag: Option<String>,
    },
}
