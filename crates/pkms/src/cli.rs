use clap::{Args, Parser, Subcommand, ValueEnum};
use pkms_org::tokens;
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
            Command::Path(_) => "path",
            #[cfg(feature = "web")]
            Command::Serve(_) => "serve",
        }
    }
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
}
