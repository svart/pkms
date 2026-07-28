use clap::{Args, Parser, Subcommand, ValueEnum};
use pkms_tokens as tokens;
use std::path::PathBuf;

mod task;

pub use task::*;

#[derive(Debug, Clone, PartialEq, ValueEnum)]
pub enum OutputFormat {
    Text,
    Json,
    Ndjson,
}

fn trim_delimited_value(value: &str) -> Result<String, String> {
    Ok(value.trim().to_string())
}

fn parse_encoding(value: &str) -> Result<tokens::Encoding, String> {
    value
        .parse()
        .map_err(|()| format!("unknown token encoding '{value}'"))
}

#[cfg(feature = "rag")]
fn parse_positive_usize(value: &str) -> Result<usize, String> {
    let parsed = value
        .parse::<usize>()
        .map_err(|_| format!("expected a positive integer, got '{value}'"))?;
    if parsed == 0 {
        return Err("value must be greater than zero".to_string());
    }
    Ok(parsed)
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
        help = "Output format"
    )]
    pub output_format: Option<OutputFormat>,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    #[command(about = "Verify health of the entire org-roam database")]
    Check(CheckArgs),
    #[command(about = "Validate health of a specific note")]
    Validate(TargetArgs),
    #[command(about = "Comprehensive database statistics")]
    Stats(StatsArgs),
    #[command(about = "List orphan notes (no incoming or outgoing links)")]
    Orphans(OrphansArgs),
    #[command(about = "Fast UUID/title resolution without full graph load")]
    Resolve(ResolveArgs),
    #[command(about = "Repair broken UUID links or misplaced attachments")]
    Fix(FixArgs),
    #[command(about = "Suggest related notes by multi-factor scoring (takes UUID only)")]
    Suggest(SuggestArgs),
    #[command(about = "Generate a filename and UUID for a new note")]
    New(NewArgs),
    #[command(about = "Extract a heading subtree into a new note")]
    Extract(ExtractArgs),
    #[command(about = "Retrieve a note with its neighbors at specified depth")]
    Get(GetArgs),
    #[command(
        about = "Fuzzy search across note titles and content. Match sources: title, alias, ref, tag, content"
    )]
    Query(QueryArgs),
    #[command(about = "Show current pkms configuration")]
    Info,
    #[command(about = "Generate default config file at ~/.config/pkms.toml")]
    InitConfig(InitConfigArgs),
    #[command(about = "List, inspect, and update tasks across configured sources")]
    Task(TaskArgs),
    #[command(about = "List tags by usage or suggest tags for a note or task")]
    Tags(TagsArgs),
    #[cfg(feature = "rag")]
    #[command(about = "Local retrieval over org-roam notes")]
    Rag(RagArgs),
    #[command(name = "path", about = "Find shortest path between two notes")]
    Path(PathArgs),
    #[cfg(feature = "web")]
    #[command(about = "Serve one rendered note and linked notes over local HTTP")]
    Serve(ServeArgs),
}

impl Command {
    pub fn name(&self) -> &'static str {
        match self {
            Command::Check(_) => "check",
            Command::Validate(_) => "validate",
            Command::Stats(_) => "stats",
            Command::Orphans(_) => "orphans",
            Command::Resolve(_) => "resolve",
            Command::Fix(_) => "fix",
            Command::Suggest(_) => "suggest",
            Command::New(_) => "new",
            Command::Extract(_) => "extract",
            Command::Get(_) => "get",
            Command::Query(_) => "query",
            Command::Info => "info",
            Command::InitConfig(_) => "init-config",
            Command::Task(_) => "task",
            Command::Tags(_) => "tags",
            #[cfg(feature = "rag")]
            Command::Rag(_) => "rag",
            Command::Path(_) => "path",
            #[cfg(feature = "web")]
            Command::Serve(_) => "serve",
        }
    }
}

#[derive(Debug, Args)]
pub struct TagsArgs {
    #[cfg(feature = "rag")]
    #[command(subcommand)]
    pub command: Option<TagsCommand>,
}

#[cfg(feature = "rag")]
#[derive(Debug, Subcommand)]
pub enum TagsCommand {
    #[command(about = "Suggest existing corpus tags for a note UUID or local task ID")]
    Suggest(TagSuggestArgs),
}

#[cfg(feature = "rag")]
#[derive(Debug, Args)]
pub struct TagSuggestArgs {
    #[arg(help = "Full note UUID or canonical local task ID, such as 12")]
    pub target: String,
    #[command(flatten)]
    pub options: TagSuggestionOptions,
}

#[cfg(feature = "rag")]
#[derive(Debug, Args)]
pub struct TagSuggestionOptions {
    #[arg(long, default_value_t = 5, value_parser = parse_positive_usize, help = "Maximum suggested tags")]
    pub limit: usize,
    #[arg(long, default_value_t = 20, value_parser = parse_positive_usize, help = "Number of similar chunks to evaluate")]
    pub neighbors: usize,
    #[arg(long, value_name = "PATH", help = "Path to RAG SQLite index")]
    pub rag_db: Option<PathBuf>,
    #[arg(long, help = "Add suggestions to the source note or task")]
    pub apply: bool,
}

#[cfg(feature = "rag")]
#[derive(Debug, Args)]
pub struct RagArgs {
    #[command(subcommand)]
    pub command: RagCommand,
}

#[cfg(feature = "rag")]
#[derive(Debug, Subcommand)]
pub enum RagCommand {
    #[command(about = "Show RAG index status")]
    Status(RagStatusArgs),
    #[command(about = "Ingest retrieval NDJSON into the RAG index")]
    Ingest(RagIngestArgs),
    #[command(about = "Rebuild the RAG index from org notes or NDJSON")]
    Index(RagIndexArgs),
    #[command(about = "Search the RAG index with SQLite FTS")]
    Search(RagSearchArgs),
    #[command(about = "Retrieve cited chunks with BM25, dense, or hybrid scoring")]
    Retrieve(RagRetrieveArgs),
    #[command(about = "Serve the RAG HTTP API")]
    Serve(RagServeArgs),
}

#[cfg(feature = "rag")]
#[derive(Debug, Args)]
pub struct RagStatusArgs {
    #[arg(long, value_name = "PATH", help = "Path to RAG SQLite index")]
    pub rag_db: Option<PathBuf>,
}

#[cfg(feature = "rag")]
#[derive(Debug, Args)]
pub struct RagIngestArgs {
    #[arg(value_name = "PATH", help = "Retrieval NDJSON file to ingest")]
    pub path: PathBuf,
    #[arg(long, value_name = "PATH", help = "Path to RAG SQLite index")]
    pub rag_db: Option<PathBuf>,
}

#[cfg(feature = "rag")]
#[derive(Debug, Args)]
pub struct RagIndexArgs {
    #[arg(long, value_name = "PATH", help = "Path to RAG SQLite index")]
    pub rag_db: Option<PathBuf>,
    #[arg(long, help = "Remove the current RAG SQLite index before rebuilding")]
    pub force_rebuild: bool,
    #[arg(
        long,
        value_name = "N",
        value_parser = parse_positive_usize,
        help = "FastEmbed batch size for this index run"
    )]
    pub embedding_batch_size: Option<usize>,
    #[arg(
        long,
        value_name = "N",
        value_parser = parse_positive_usize,
        help = "Maximum body characters included in embedding text for this index run"
    )]
    pub embedding_max_body_chars: Option<usize>,
    #[arg(
        long = "output-format",
        hide = true,
        value_name = "FMT",
        value_parser = reject_rag_index_output_format
    )]
    pub output_format: Option<OutputFormat>,
}

#[cfg(feature = "rag")]
fn reject_rag_index_output_format(value: &str) -> Result<OutputFormat, String> {
    Err(format!(
        "--output-format is not supported by pkms rag index; got '{value}'"
    ))
}

#[cfg(feature = "rag")]
#[derive(Debug, Args)]
pub struct RagSearchArgs {
    #[arg(help = "Search query")]
    pub query: String,
    #[arg(long, default_value_t = 10, help = "Maximum results")]
    pub limit: usize,
    #[arg(long, value_name = "PATH", help = "Path to RAG SQLite index")]
    pub rag_db: Option<PathBuf>,
}

#[cfg(feature = "rag")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum RagRetrieveMode {
    Hybrid,
    Bm25,
    Dense,
}

#[cfg(feature = "rag")]
#[derive(Debug, Args)]
pub struct RagRetrieveArgs {
    #[arg(help = "Retrieval query")]
    pub query: String,
    #[arg(long, default_value_t = 10, help = "Maximum results")]
    pub limit: usize,
    #[arg(long, value_enum, default_value_t = RagRetrieveMode::Hybrid, help = "Retrieval mode")]
    pub mode: RagRetrieveMode,
    #[arg(long, value_name = "N", help = "Maximum returned token budget")]
    pub max_token_budget: Option<usize>,
    #[arg(long, value_name = "PATH", help = "Path to RAG SQLite index")]
    pub rag_db: Option<PathBuf>,
}

#[cfg(feature = "rag")]
#[derive(Debug, Args)]
pub struct RagServeArgs {
    #[arg(long, value_name = "PATH", help = "Org notes root to index")]
    pub notes_root: Option<PathBuf>,
    #[arg(long, value_name = "PATH", help = "Retrieval NDJSON source to index")]
    pub index_source: Option<PathBuf>,
    #[arg(long, value_name = "PATH", help = "Path to RAG SQLite index")]
    pub rag_db: Option<PathBuf>,
    #[arg(long, value_name = "HOST", help = "HTTP bind host")]
    pub host: Option<String>,
    #[arg(long, value_name = "PORT", help = "HTTP bind port")]
    pub port: Option<u16>,
}

#[derive(Debug, Args)]
pub struct CheckArgs {
    #[arg(long, help = "Show database statistics")]
    pub stats: bool,
    #[arg(long, help = "Check that file: link targets exist on disk")]
    pub file_links: bool,
    #[arg(
        long,
        help = "Check remote SSH file: link targets (requires --features ssh)"
    )]
    pub remote_file_links: bool,
    #[arg(long, help = "Check that attachment: link targets exist on disk")]
    pub attachment_links: bool,
    #[arg(long, help = "Check that id: link targets exist in the database")]
    pub id_links: bool,
    #[arg(long, help = "Check filetags format correctness")]
    pub filetags: bool,
    #[arg(
        long,
        help = "Check for self-referencing links (id: or file: pointing to self)"
    )]
    pub self_links: bool,
    #[arg(
        long,
        help = "Check for overlinking (2+ internal links to the same note)"
    )]
    pub overlinks: bool,
    #[arg(
        long = "cross-links",
        num_args = 2,
        value_names = ["NOTE_A", "NOTE_B"],
        help = "Check cross-links between two notes"
    )]
    pub cross_links: Option<Vec<String>>,
}

#[derive(Debug, Args)]
pub struct TargetArgs {
    #[arg(help = "UUID, file path, or note title")]
    pub target: Option<String>,
    #[arg(long, help = "Read UUIDs from NDJSON stdin")]
    pub from_stdin: bool,
}

#[derive(Debug, Args)]
pub struct StatsArgs {
    #[arg(short, long, help = "Show notes modified in last N days")]
    pub days: Option<u32>,
    #[arg(
        long,
        num_args = 0..=1,
        default_missing_value = "10",
        help = "Show most-connected notes (default limit: 10)"
    )]
    pub hubs: Option<usize>,
    #[arg(long, help = "List all filetags with note counts")]
    pub tags: bool,
    #[arg(long, help = "Show TODO/DONE statistics per note")]
    pub todos: bool,
}

#[derive(Debug, Args)]
pub struct OrphansArgs {
    #[arg(long, help = "Maximum results (default: unlimited)")]
    pub limit: Option<usize>,
    #[arg(long, help = "Include daily notes in the orphans list")]
    pub with_dailies: bool,
}

#[derive(Debug, Args)]
pub struct ResolveArgs {
    #[arg(
        long,
        help = "Search by UUID (substring match)",
        required_unless_present_any = ["title", "tags"]
    )]
    pub uuid: Option<String>,
    #[arg(
        long,
        help = "Search by title or alias (substring match)",
        required_unless_present_any = ["uuid", "tags"]
    )]
    pub title: Option<String>,
    #[arg(
        long,
        help = "Include notes with these filetags (comma-separated)",
        value_delimiter = ',',
        num_args = 1,
        action = clap::ArgAction::Set,
        value_parser = trim_delimited_value,
        required_unless_present_any = ["uuid", "title"]
    )]
    pub tags: Option<Vec<String>>,
    #[arg(long, help = "Maximum results")]
    pub limit: Option<usize>,
    #[arg(
        long,
        value_name = "FIELDS",
        value_delimiter = ',',
        num_args = 1,
        action = clap::ArgAction::Set,
        value_parser = trim_delimited_value,
        help = "Comma-separated fields: uuid,title,path,tags,aliases"
    )]
    pub fields: Option<Vec<String>>,
    #[arg(long, help = "Restrict to files with TODO headings")]
    pub todos: bool,
}

#[derive(Debug, Args)]
pub struct FixArgs {
    #[command(subcommand)]
    pub command: FixCommand,
}

#[derive(Debug, Subcommand)]
pub enum FixCommand {
    #[command(about = "Fix broken links by replacing UUIDs across the database")]
    Uuid(FixUuidArgs),
    #[command(about = "Move or copy misplaced org-attach files to expected roots")]
    Attach(FixAttachArgs),
}

#[derive(Debug, Args)]
pub struct FixUuidArgs {
    #[arg(help = "Broken UUID (full UUID format with dashes)")]
    pub broken_uuid: String,
    #[arg(help = "Replacement UUID (full UUID format with dashes)")]
    pub target: String,
    #[arg(
        short,
        long,
        help = "Actually apply the fix (dry-run without this flag)"
    )]
    pub apply: bool,
}

#[derive(Debug, Args)]
pub struct FixAttachArgs {
    #[arg(
        short,
        long,
        help = "Actually repair attachments (dry-run without this flag)"
    )]
    pub apply: bool,
    #[arg(long, help = "Copy matched files instead of moving them")]
    pub copy: bool,
}

#[derive(Debug, Args)]
pub struct SuggestArgs {
    #[arg(help = "UUID of the target note")]
    pub target: Option<String>,
    #[arg(short, long, help = "Number of suggestions (default: unlimited)")]
    pub limit: Option<usize>,
    #[arg(long, help = "Exclude orphan notes from suggestions")]
    pub exclude_orphans: bool,
    #[arg(long, help = "Read UUIDs from NDJSON stdin")]
    pub from_stdin: bool,
}

#[derive(Debug, Args)]
pub struct NewArgs {
    #[arg(help = "Title of the new note")]
    pub title: String,
    #[arg(long, help = "Actually write the boilerplate file")]
    pub create: bool,
    #[arg(
        long,
        help = "Comma-separated list of filetags",
        value_delimiter = ',',
        num_args = 1,
        action = clap::ArgAction::Set,
        value_parser = trim_delimited_value
    )]
    pub tags: Option<Vec<String>>,
    #[arg(
        long,
        help = "Comma-separated list of aliases",
        value_delimiter = ',',
        num_args = 1,
        action = clap::ArgAction::Set,
        value_parser = trim_delimited_value
    )]
    pub aliases: Option<Vec<String>>,
    #[arg(long, help = "Heading title to generate :ID: for")]
    pub heading: Option<String>,
}

#[derive(Debug, Args)]
pub struct ExtractArgs {
    #[arg(help = "UUID of the heading to extract")]
    pub heading_uuid: String,
    #[arg(help = "Title for the new note; defaults to the heading title")]
    pub new_name: Option<String>,
    #[arg(
        long,
        help = "Actually create the new note and rewrite the source note"
    )]
    pub apply: bool,
}

#[derive(Debug, Args)]
pub struct GetArgs {
    #[arg(help = "UUID, file path, or note title")]
    pub target: Option<String>,
    #[arg(long, help = "Show forward and backward links (depth 1)")]
    pub links: bool,
    #[arg(long, help = "Show heading structure")]
    pub headings: bool,
    #[arg(
        long,
        value_name = "HEADING_TITLE",
        help = "Show only this heading block in note content"
    )]
    pub heading: Option<String>,
    #[arg(long, help = "Suppress note content output")]
    pub no_content: bool,
    #[arg(long, help = "Read UUIDs from NDJSON stdin")]
    pub from_stdin: bool,
    #[arg(
        long,
        default_value = "cl100k_base",
        value_parser = parse_encoding,
        help = "Token encoding: cl100k_base (GPT-4) or o200k_base (GPT-4o)"
    )]
    pub encoding: tokens::Encoding,
}

#[derive(Debug, Args)]
pub struct QueryArgs {
    #[arg(help = "Search terms")]
    pub terms: Option<String>,
    #[arg(long, help = "Maximum results (default: unlimited)")]
    pub limit: Option<usize>,
    #[arg(long, help = "Search only in filetags")]
    pub tags: bool,
    #[arg(long, help = "Search only in titles, aliases, and refs")]
    pub title: bool,
    #[arg(long, help = "Search only in file content")]
    pub content: bool,
    #[arg(long, help = "Restrict to files with TODO headings")]
    pub todos: bool,
}

#[cfg(feature = "web")]
#[derive(Debug, Args)]
pub struct ServeArgs {
    #[arg(help = "UUID, file path, or note title to render first")]
    pub target: String,
    #[arg(long, default_value = "127.0.0.1", help = "Address to bind")]
    pub host: String,
    #[arg(
        long,
        default_value_t = 8765,
        help = "Port to bind, or 0 for any free port"
    )]
    pub port: u16,
}

#[derive(Debug, Args)]
pub struct InitConfigArgs {
    #[arg(short, long, help = "Database root path to write into config")]
    pub db: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct PathArgs {
    #[arg(help = "Source note (UUID, path, or title)")]
    pub from: String,
    #[arg(help = "Target note (UUID, path, or title)")]
    pub to: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(feature = "rag")]
    use clap::CommandFactory;

    fn parse(args: &[&str]) -> Cli {
        Cli::try_parse_from(args).unwrap()
    }

    #[test]
    fn resolve_parses_delimited_tags_and_fields() {
        let cli = parse(&[
            "pkms",
            "resolve",
            "--tags",
            "alpha, beta",
            "--fields",
            "uuid, title",
        ]);

        let Command::Resolve(args) = cli.command else {
            panic!("expected resolve command");
        };
        assert_eq!(args.tags, Some(vec!["alpha".into(), "beta".into()]));
        assert_eq!(args.fields, Some(vec!["uuid".into(), "title".into()]));
    }

    #[test]
    fn new_parses_delimited_tags_and_aliases() {
        let cli = parse(&[
            "pkms",
            "new",
            "Tagged New",
            "--tags",
            "foo, bar",
            "--aliases",
            "Alias One, Alias Two",
        ]);

        let Command::New(args) = cli.command else {
            panic!("expected new command");
        };
        assert_eq!(args.tags, Some(vec!["foo".into(), "bar".into()]));
        assert_eq!(
            args.aliases,
            Some(vec!["Alias One".into(), "Alias Two".into()])
        );
    }

    #[test]
    fn get_parses_encoding_at_cli_boundary() {
        let cli = parse(&["pkms", "get", "Note A"]);
        let Command::Get(args) = cli.command else {
            panic!("expected get command");
        };
        assert_eq!(args.encoding, tokens::Encoding::Cl100kBase);

        let cli = parse(&["pkms", "get", "Note A", "--encoding", "o200k"]);
        let Command::Get(args) = cli.command else {
            panic!("expected get command");
        };
        assert_eq!(args.encoding, tokens::Encoding::O200kBase);

        let err = match Cli::try_parse_from(["pkms", "get", "Note A", "--encoding", "unknown"]) {
            Ok(_) => panic!("expected invalid encoding to fail at CLI boundary"),
            Err(err) => err,
        };
        assert_eq!(err.kind(), clap::error::ErrorKind::ValueValidation);
    }

    #[test]
    fn path_requires_source_and_target_at_parse_time() {
        assert!(Cli::try_parse_from(["pkms", "path"]).is_err());
        assert!(Cli::try_parse_from(["pkms", "path", "Note A"]).is_err());

        let cli = parse(&["pkms", "path", "Note A", "Note C"]);
        let Command::Path(args) = cli.command else {
            panic!("expected path command");
        };
        assert_eq!(args.from, "Note A");
        assert_eq!(args.to, "Note C");
    }

    #[test]
    fn task_calendar_parses_month_count() {
        let cli = parse(&["pkms", "task", "calendar"]);
        let Command::Task(TaskArgs {
            command: TaskCommand::Calendar(args),
        }) = cli.command
        else {
            panic!("expected task calendar command");
        };
        assert_eq!(args.months, 1);

        let cli = parse(&["pkms", "task", "calendar", "-m", "3"]);
        let Command::Task(TaskArgs {
            command: TaskCommand::Calendar(args),
        }) = cli.command
        else {
            panic!("expected task calendar command");
        };
        assert_eq!(args.months, 3);

        let cli = parse(&["pkms", "task", "calendar", "-m", "-1"]);
        let Command::Task(TaskArgs {
            command: TaskCommand::Calendar(args),
        }) = cli.command
        else {
            panic!("expected task calendar command");
        };
        assert_eq!(args.months, -1);

        assert!(Cli::try_parse_from(["pkms", "task", "calendar", "--months", "0"]).is_err());
    }

    #[cfg(feature = "rag")]
    #[test]
    fn rag_index_parses_force_rebuild_and_rag_db() {
        let cli = parse(&[
            "pkms",
            "rag",
            "index",
            "--force-rebuild",
            "--rag-db",
            "/tmp/rag.sqlite3",
        ]);
        let Command::Rag(rag) = cli.command else {
            panic!("expected rag command");
        };
        let RagCommand::Index(args) = rag.command else {
            panic!("expected rag index command");
        };
        assert!(args.force_rebuild);
        assert_eq!(args.rag_db, Some(PathBuf::from("/tmp/rag.sqlite3")));
    }

    #[cfg(feature = "rag")]
    #[test]
    fn tags_suggest_parses_safe_defaults_and_apply() {
        let target = "11111111-1111-1111-1111-111111111111";
        let cli = parse(&["pkms", "tags", "suggest", target, "--apply"]);

        let Command::Tags(TagsArgs {
            command: Some(TagsCommand::Suggest(args)),
        }) = cli.command
        else {
            panic!("expected tags suggest command");
        };
        assert_eq!(args.target, target);
        assert_eq!(args.options.limit, 5);
        assert_eq!(args.options.neighbors, 20);
        assert!(args.options.apply);
    }

    #[cfg(feature = "rag")]
    #[test]
    fn tags_suggest_rejects_zero_limits_at_cli_boundary() {
        let error = match Cli::try_parse_from(["pkms", "tags", "suggest", "1", "--neighbors", "0"])
        {
            Ok(_) => panic!("expected zero neighbors to fail"),
            Err(error) => error,
        };

        assert_eq!(error.kind(), clap::error::ErrorKind::ValueValidation);
    }

    #[cfg(feature = "rag")]
    #[test]
    fn rag_index_parses_embedding_controls() {
        let cli = parse(&[
            "pkms",
            "rag",
            "index",
            "--embedding-batch-size",
            "8",
            "--embedding-max-body-chars",
            "4096",
        ]);
        let Command::Rag(rag) = cli.command else {
            panic!("expected rag command");
        };
        let RagCommand::Index(args) = rag.command else {
            panic!("expected rag index command");
        };

        assert_eq!(args.embedding_batch_size, Some(8));
        assert_eq!(args.embedding_max_body_chars, Some(4096));
    }

    #[cfg(feature = "rag")]
    #[test]
    fn rag_index_rejects_zero_embedding_controls() {
        assert!(
            Cli::try_parse_from(["pkms", "rag", "index", "--embedding-batch-size", "0"]).is_err()
        );
        assert!(
            Cli::try_parse_from(["pkms", "rag", "index", "--embedding-max-body-chars", "0"])
                .is_err()
        );
    }

    #[cfg(feature = "rag")]
    #[test]
    fn rag_serve_parses_source_and_bind_options() {
        let cli = parse(&[
            "pkms",
            "rag",
            "serve",
            "--notes-root",
            "/tmp/notes",
            "--index-source",
            "/tmp/export.ndjson",
            "--rag-db",
            "/tmp/rag.sqlite3",
            "--host",
            "0.0.0.0",
            "--port",
            "7444",
        ]);
        let Command::Rag(rag) = cli.command else {
            panic!("expected rag command");
        };
        let RagCommand::Serve(args) = rag.command else {
            panic!("expected rag serve command");
        };

        assert_eq!(args.notes_root, Some(PathBuf::from("/tmp/notes")));
        assert_eq!(args.index_source, Some(PathBuf::from("/tmp/export.ndjson")));
        assert_eq!(args.rag_db, Some(PathBuf::from("/tmp/rag.sqlite3")));
        assert_eq!(args.host.as_deref(), Some("0.0.0.0"));
        assert_eq!(args.port, Some(7444));
    }

    #[cfg(feature = "rag")]
    #[test]
    fn rag_index_help_exposes_supported_local_flags() {
        let mut command = Cli::command();
        let index = command
            .find_subcommand_mut("rag")
            .and_then(|rag| rag.find_subcommand_mut("index"))
            .expect("rag index command exists");
        let mut help = Vec::new();
        index.write_long_help(&mut help).expect("help renders");
        let help = String::from_utf8(help).expect("help is utf8");

        assert!(help.contains("--force-rebuild"));
        assert!(help.contains("--rag-db"));
        assert!(help.contains("--embedding-batch-size"));
        assert!(help.contains("--embedding-max-body-chars"));
    }
}
