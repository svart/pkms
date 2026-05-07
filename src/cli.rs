use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

#[derive(Clone, PartialEq, ValueEnum)]
pub enum OutputFormat {
    Text,
    Json,
    Ndjson,
}

#[derive(Parser)]
#[command(
    name = "pkms",
    version,
    about = "org-roam PKMS navigation and validation tool"
)]
pub struct Cli {
    #[arg(
        global = true,
        long,
        value_name = "PATH",
        help = "Path to org-roam database root (overrides config)"
    )]
    pub db: Option<PathBuf>,

    #[arg(
        global = true,
        long,
        value_enum,
        value_name = "FMT",
        help = "Output format: text, json, ndjson"
    )]
    pub output_format: Option<OutputFormat>,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    #[command(about = "Verify health of the entire org-roam database")]
    Check {
        #[arg(long, help = "Check that file: link targets exist on disk")]
        file_links: bool,
        #[arg(long, help = "Check that attachment: link targets exist on disk")]
        attachment_links: bool,
        #[arg(long, help = "Check that id: link targets exist in the database")]
        id_links: bool,
        #[arg(long, help = "Check filetags format correctness")]
        filetags: bool,
    },
    #[command(about = "Validate health of a specific note")]
    Validate {
        #[arg(help = "UUID, file path, or note title")]
        target: Option<String>,
        #[arg(long, help = "Read UUIDs from NDJSON stdin")]
        from_stdin: bool,
    },
    #[command(about = "Comprehensive database statistics")]
    Stats {
        #[arg(short, long, help = "Show notes modified in last N days")]
        days: Option<u32>,
        #[arg(
            long,
            num_args = 0..=1,
            default_missing_value = "10",
            help = "Show most-connected notes (default limit: 10)"
        )]
        hubs: Option<usize>,
        #[arg(long, help = "List all filetags with note counts")]
        tags: bool,
    },
    #[command(about = "List orphan notes (no incoming or outgoing links)")]
    Orphans {
        #[arg(long, help = "Maximum results (default: unlimited)")]
        limit: Option<usize>,
    },
    #[command(about = "Build a context window for AI consumption")]
    Context {
        #[arg(help = "UUID, file path, or note title")]
        target: Option<String>,
        #[arg(short, long, default_value = "1", help = "Traversal depth")]
        depth: u32,
        #[arg(short, long, help = "Maximum tokens in output")]
        max_tokens: Option<usize>,
        #[arg(
            long,
            default_value = "cl100k_base",
            help = "Token encoding: cl100k_base (GPT-4) or o200k_base (GPT-4o)"
        )]
        encoding: String,
        #[arg(long, help = "Read UUIDs from NDJSON stdin")]
        from_stdin: bool,
    },
    #[command(about = "Fast UUID/title resolution without full graph load")]
    Resolve {
        #[arg(
            long,
            help = "Search by UUID (substring match)",
            required_unless_present_any = ["title", "tags"]
        )]
        uuid: Option<String>,
        #[arg(
            long,
            help = "Search by title or alias (substring match)",
            required_unless_present_any = ["uuid", "tags"]
        )]
        title: Option<String>,
        #[arg(
            long,
            help = "Include notes with these filetags (comma-separated)",
            required_unless_present_any = ["uuid", "title"]
        )]
        tags: Option<String>,
        #[arg(long, help = "Maximum results")]
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
        #[arg(help = "Broken UUID (full UUID format with dashes)")]
        broken_uuid: String,
        #[arg(help = "Replacement UUID (full UUID format with dashes)")]
        target: String,
        #[arg(
            short,
            long,
            help = "Actually apply the fix (dry-run without this flag)"
        )]
        apply: bool,
    },
    #[command(about = "Suggest related notes by multi-factor scoring (takes UUID only)")]
    Suggest {
        #[arg(help = "UUID of the target note")]
        target: Option<String>,
        #[arg(short, long, help = "Number of suggestions (default: unlimited)")]
        limit: Option<usize>,
        #[arg(long, help = "Exclude orphan notes from suggestions")]
        exclude_orphans: bool,
        #[arg(long, help = "Read UUIDs from NDJSON stdin")]
        from_stdin: bool,
        #[arg(long, help = "Use embedding-based similarity")]
        embed: bool,
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
        #[arg(long, help = "Show forward and backward links (depth 1)")]
        links: bool,
        #[arg(long, help = "Show heading structure")]
        headings: bool,
        #[arg(long, help = "Suppress note content output")]
        no_content: bool,
        #[arg(long, help = "Read UUIDs from NDJSON stdin")]
        from_stdin: bool,
    },
    #[command(
        about = "Fuzzy search across note titles and content. Match sources: title, alias, ref, tag, content"
    )]
    Query {
        #[arg(help = "Search terms")]
        terms: Option<String>,
        #[arg(long, help = "Maximum results (default: unlimited)")]
        limit: Option<usize>,
        #[arg(long, help = "Search only in filetags")]
        tags: bool,
        #[arg(long, help = "Search only in titles, aliases, and refs")]
        title: bool,
        #[arg(long, help = "Search only in file content")]
        content: bool,
        #[arg(long, help = "Use embedding-based similarity")]
        embed: bool,
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
    },
}
