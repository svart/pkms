use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

#[derive(Clone, ValueEnum)]
pub enum OutputFormat {
    Json,
    Ndjson,
}

#[derive(Parser)]
#[command(
    name = "pkms",
    version,
    about = "org-roam PKMS navigation and validation tool"
)]
#[allow(clippy::struct_excessive_bools)]
pub struct Cli {
    #[arg(
        global = true,
        long,
        value_name = "PATH",
        help = "Path to org-roam database root (overrides config)"
    )]
    pub db: Option<PathBuf>,

    #[arg(global = true, long, help = "Structured JSON output")]
    pub json: bool,

    #[arg(global = true, short, long, help = "Verbose output")]
    pub verbose: bool,

    #[arg(
        global = true,
        short = 'q',
        long,
        help = "Suppress non-essential stderr output"
    )]
    pub quiet: bool,

    #[arg(
        global = true,
        long,
        value_enum,
        value_name = "FMT",
        help = "Output format: json, ndjson (implies machine-readable output)"
    )]
    pub output_format: Option<OutputFormat>,

    #[arg(global = true, long, help = "Suppress column headers in human output")]
    pub no_header: bool,

    #[arg(global = true, long = "count", help = "Show only the count of results")]
    pub count_only: bool,

    #[arg(
        global = true,
        long,
        help = "Show usage example for the given command and exit"
    )]
    pub example: bool,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    #[command(about = "Verify health of the entire org-roam database")]
    Check {
        #[arg(long, help = "Check that file: link targets exist on disk")]
        file_links: bool,
        #[arg(long, help = "Check that attachment: link targets exist on disk")]
        attachment_links: bool,
    },
    #[command(about = "Validate health of a specific note")]
    Validate {
        #[arg(help = "UUID, file path, or note title")]
        target: Option<String>,
        #[arg(
            long,
            value_name = "FILE",
            help = "Read command parameters from JSON file"
        )]
        input_json: Option<PathBuf>,
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
        target: Option<String>,
        #[arg(short, long, default_value = "1", help = "Traversal depth")]
        depth: u32,
        #[arg(short, long, help = "Maximum tokens in output")]
        max_tokens: Option<usize>,
        #[arg(long, help = "Include forward/outgoing links (default: true)")]
        include_outgoing: Option<bool>,
        #[arg(long, help = "Include backlinks/incoming links (default: true)")]
        include_incoming: Option<bool>,
        #[arg(
            long,
            value_name = "TEMPLATE",
            help = "Template with {{title}}, {{content}}, {{neighbors}}, {{backlinks}} placeholders"
        )]
        template: Option<String>,
        #[arg(
            long,
            value_name = "FILE",
            help = "Read command parameters from JSON file"
        )]
        input_json: Option<PathBuf>,
    },
    #[command(about = "Fast UUID/title resolution without full graph load")]
    Resolve {
        #[arg(help = "UUID prefix, full UUID, or title to resolve")]
        target: Option<String>,
        #[arg(
            short,
            long,
            help = "Include notes with these filetags (comma-separated)"
        )]
        tags: Option<String>,
        #[arg(short, long, help = "Search in aliases and titles (substring)")]
        search: Option<String>,
        #[arg(short, long, default_value = "30", help = "Maximum results")]
        limit: Option<usize>,
        #[arg(
            long,
            value_name = "FIELDS",
            help = "Comma-separated fields: uuid,title,path,tags,aliases"
        )]
        fields: Option<String>,
    },
    #[command(about = "Fix broken links by replacing UUIDs across the database")]
    Fix {
        #[arg(help = "Broken UUID to replace")]
        broken_uuid: String,
        #[arg(help = "Replacement UUID or note title to resolve to")]
        target: String,
        #[arg(
            short,
            long,
            help = "Actually apply the fix (dry-run without this flag)"
        )]
        apply: bool,
    },
    #[command(about = "Suggest related notes for a target note")]
    Suggest {
        #[arg(help = "UUID, file path, or note title")]
        target: Option<String>,
        #[arg(short, long, default_value = "10", help = "Number of suggestions")]
        limit: Option<usize>,
        #[arg(
            long,
            value_name = "FILE",
            help = "Read command parameters from JSON file"
        )]
        input_json: Option<PathBuf>,
    },
    #[command(about = "Generate a filename and UUID for a new note")]
    New {
        #[arg(help = "Title of the new note")]
        title: String,
        #[arg(long, help = "Actually write the boilerplate file")]
        create: bool,
        #[arg(long, help = "Comma-separated list of filetags")]
        tags: Option<String>,
        #[arg(long, help = "Comma-separated list of aliases")]
        aliases: Option<String>,
    },
    #[command(about = "Retrieve a note with its neighbors at specified depth")]
    Get {
        #[arg(help = "UUID, file path, or note title")]
        target: Option<String>,
        #[arg(short, long, default_value = "1", help = "Traversal depth")]
        depth: u32,
        #[arg(short, long, help = "Show full note content")]
        out: bool,
        #[arg(short, long, help = "Show ASCII graph visualization")]
        graph: bool,
        #[arg(long, help = "Read targets from stdin, one per line")]
        from_stdin: bool,
        #[arg(
            long,
            value_name = "FILE",
            help = "Read targets from file, one per line"
        )]
        from_file: Option<PathBuf>,
        #[arg(
            long,
            value_name = "FILE",
            help = "Read command parameters from JSON file"
        )]
        input_json: Option<PathBuf>,
    },
    #[command(about = "Fuzzy search across note titles and content")]
    Query {
        #[arg(help = "Search terms")]
        terms: Option<String>,
        #[arg(short, long, help = "Filter by filetag")]
        tag: Option<String>,
        #[arg(short, long, help = "Maximum results")]
        limit: Option<usize>,
        #[arg(
            long,
            value_name = "FILE",
            help = "Read command parameters from JSON file"
        )]
        input_json: Option<PathBuf>,
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
        from: Option<String>,
        #[arg(help = "Target note (UUID, path, or title)")]
        to: Option<String>,
        #[arg(short, long, help = "Maximum traversal depth")]
        max_depth: Option<u32>,
        #[arg(
            long,
            value_name = "FILE",
            help = "Read command parameters from JSON file"
        )]
        input_json: Option<PathBuf>,
    },
    #[command(about = "Export subgraph around a note")]
    Subgraph {
        #[arg(help = "Root note (UUID, path, or title)")]
        target: Option<String>,
        #[arg(short, long, default_value = "1", help = "Traversal depth")]
        depth: u32,
        #[arg(
            long,
            value_name = "FILE",
            help = "Read command parameters from JSON file"
        )]
        input_json: Option<PathBuf>,
    },
    #[command(about = "List all filetags with note counts")]
    Tags {
        #[arg(short, long, help = "List all notes with this tag")]
        tag: Option<String>,
    },
}
