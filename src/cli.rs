use clap::{Args, Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, ValueEnum)]
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
    #[command(about = "Fix broken links by replacing UUIDs across the database")]
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
        required_unless_present_any = ["uuid", "title"]
    )]
    pub tags: Option<String>,
    #[arg(long, help = "Maximum results")]
    pub limit: Option<usize>,
    #[arg(
        long,
        value_name = "FIELDS",
        help = "Comma-separated fields: uuid,title,path,tags,aliases"
    )]
    pub fields: Option<String>,
    #[arg(long, help = "Restrict to files with TODO headings")]
    pub todos: bool,
}

#[derive(Debug, Args)]
pub struct FixArgs {
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
    #[arg(long, help = "Comma-separated list of filetags")]
    pub tags: Option<String>,
    #[arg(long, help = "Comma-separated list of aliases")]
    pub aliases: Option<String>,
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
        help = "Token encoding: cl100k_base (GPT-4) or o200k_base (GPT-4o)"
    )]
    pub encoding: String,
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
pub struct TaskFilterArgs {
    #[arg(
        long,
        value_name = "STATE",
        help = "Filter by TODO states (comma-separated, ! for negation, applied as AND)"
    )]
    pub state: Option<String>,
    #[arg(
        long,
        value_name = "TAGS",
        help = "Filter by tags (comma-separated, ! for negation, applied as AND)"
    )]
    pub tags: Option<String>,
    #[arg(
        long = "type",
        value_name = "TYPE",
        help = "Filter by type: SCHED/DEADL (comma-separated, ! for negation, applied as AND)"
    )]
    pub kind: Option<String>,
}

#[derive(Debug, Args)]
pub struct TaskTableArgs {
    #[arg(long, help = "Add line separators between rows")]
    pub line_sep: bool,
    #[arg(
        long,
        value_name = "COLS",
        help = "Comma-separated column names, or +/- adjustments: Id,Date,State,Type,Prio,Tags,Project,Note,Heading"
    )]
    pub columns: Option<String>,
}

#[derive(Debug, Args)]
#[command(
    after_help = "ID-first actions:\n  pkms task <ID> show\n  pkms task <ID> open [--editor <COMMAND>] [--line <LINE>]\n  pkms task <ID> state <STATE> [--dry-run]\n  pkms task <ID> done [--dry-run]\n  pkms task <ID> postpone --to <DATE>\n  pkms task <ID> mod <MODIFIER>...\n  pkms task <ID> mod dep:<PARENT-ID>\n\nTask modifiers apply to `task add` and `task <ID> mod`.\n  title:<text>              Task title; non-modifier words are task text for add only\n  state:<state>             PKMS TODO state from configured agenda states\n  tag:<label>, tags:<a,b>   Labels/tags; repeat or comma-separate\n  schedule:<date>, sch:<date>, due:<date>\n  deadline:<date>, dead:<date>, dl:<date>\n  project:<name-or-id>, proj:<name-or-id>, prio:A|B|C, desc:<text>, source:pkms|todoist\n  note:<uuid-title-or-path> PKMS add only; choose the note to append into\n  dep:<task-id>, depend:<task-id> PKMS add/mod; add as child or move under parent"
)]
pub struct TaskArgs {
    #[command(subcommand)]
    pub command: TaskCommand,
}

#[derive(Debug, Subcommand)]
pub enum TaskCommand {
    #[command(about = "List tasks")]
    List(TaskListArgs),
    #[command(about = "Show scheduled and deadline tasks")]
    Agenda(TaskAgendaArgs),
    #[command(about = "Show inbox tasks")]
    Inbox(TaskShortcutArgs),
    #[command(about = "Show detailed task information", hide = true)]
    Show(TaskTargetArgs),
    #[command(about = "Open a task in an editor", hide = true)]
    Open(TaskOpenArgs),
    #[command(about = "Set a task TODO state", hide = true)]
    State(TaskStateArgs),
    #[command(about = "Set a task to the configured closed state", hide = true)]
    Done(TaskDoneArgs),
    #[command(about = "Add a task")]
    Add(TaskAddArgs),
    #[command(about = "Postpone a task", hide = true)]
    Postpone(TaskPostponeArgs),
    #[command(external_subcommand)]
    Target(Vec<String>),
}

#[derive(Debug, Args)]
#[command(
    after_help = "Filters:\n  source:pkms|todoist|all, src:pkms|todoist|all\n  state:TODO|opened|closed, tags:tag,!other, type:SCHED,DEADL, prio:A[,B,C], project:Name\n  date:today|week|overdue|upcoming|YYYY-MM-DD[,value...], after:YYYY-MM-DD, before:YYYY-MM-DD\n  todoist.filter:<query> (requires source:todoist or source:all)\n\nExamples:\n  pkms task list source:todoist tag:phone prio:A\n  pkms task list state:opened,!waiting prio:A,B,C\n  pkms task list source:all date:today,overdue project:Inbox"
)]
pub struct TaskListArgs {
    #[arg(value_name = "MODE_OR_FILTER")]
    pub filters: Vec<String>,
    #[arg(long, value_name = "SORT")]
    pub sort: Option<String>,
    #[arg(long, help = "Maximum results")]
    pub limit: Option<usize>,
    #[arg(
        long,
        help = "Group PKMS tasks by priority, state, or file. Matches `todo --group` for source:pkms."
    )]
    pub group: Option<String>,
    #[arg(long, help = "Read UUIDs from NDJSON stdin to use as PKMS scope")]
    pub from_stdin: bool,
    #[command(flatten)]
    pub table: TaskTableArgs,
}

#[derive(Debug, Args)]
#[command(
    after_help = "Filters:\n  source:pkms|todoist|all, src:pkms|todoist|all\n  state:TODO|opened|closed, tags:tag,!other, type:SCHED,DEADL, prio:A[,B,C], project:Name\n  date:today|week|overdue|upcoming|YYYY-MM-DD[,value...], after:YYYY-MM-DD, before:YYYY-MM-DD\n  todoist.filter:<query> (requires source:todoist or source:all)\n\nExamples:\n  pkms task agenda source:todoist\n  pkms task agenda source:all date:today,overdue tag:waiting\n  pkms task agenda state:opened,!waiting prio:A,B,C\n  pkms task agenda today source:todoist"
)]
pub struct TaskAgendaArgs {
    #[command(subcommand)]
    pub command: Option<TaskAgendaCommand>,
    #[arg(value_name = "FILTER")]
    pub filters: Vec<String>,
    #[arg(long, value_name = "SORT")]
    pub sort: Option<String>,
    #[arg(long, help = "Maximum results")]
    pub limit: Option<usize>,
    #[command(flatten)]
    pub table: TaskTableArgs,
}

#[derive(Debug, Subcommand)]
pub enum TaskAgendaCommand {
    #[command(about = "Show today's tasks")]
    Today(TaskShortcutArgs),
    #[command(about = "Show this week's tasks")]
    Week(TaskShortcutArgs),
    #[command(about = "Show overdue tasks")]
    Overdue(TaskShortcutArgs),
    #[command(about = "Show upcoming tasks")]
    Upcoming(TaskUpcomingArgs),
}

#[derive(Debug, Args)]
#[command(
    after_help = "Filters:\n  source:pkms|todoist|all, src:pkms|todoist|all\n  state:TODO|opened|closed, tags:tag,!other, type:SCHED,DEADL, prio:A, project:Name\n  todoist.filter:<query> (requires source:todoist or source:all)\n\nExamples:\n  pkms task agenda today source:todoist\n  pkms task inbox source:all state:!closed project:Inbox"
)]
pub struct TaskShortcutArgs {
    #[arg(value_name = "FILTER")]
    pub filters: Vec<String>,
    #[arg(long, help = "Maximum results")]
    pub limit: Option<usize>,
    #[command(flatten)]
    pub table: TaskTableArgs,
}

#[derive(Debug, Args)]
#[command(
    after_help = "Filters:\n  source:pkms|todoist|all, src:pkms|todoist|all\n  state:TODO|opened|closed, tags:tag,!other, type:SCHED,DEADL, prio:A, project:Name\n  todoist.filter:<query> (requires source:todoist or source:all)\n\nExamples:\n  pkms task agenda upcoming --days 14 source:all tag:phone\n  pkms task agenda upcoming source:todoist state:opened prio:B"
)]
pub struct TaskUpcomingArgs {
    #[arg(value_name = "FILTER")]
    pub filters: Vec<String>,
    #[arg(long, default_value_t = 7, help = "Number of upcoming days to show")]
    pub days: i64,
    #[arg(long, help = "Maximum results")]
    pub limit: Option<usize>,
    #[command(flatten)]
    pub table: TaskTableArgs,
}

#[derive(Debug, Args)]
pub struct TaskTargetArgs {
    #[arg(help = "Task ID: 12, p12, pkms:12, or todoist:<remote-id>")]
    pub id: String,
}

#[derive(Debug, Args)]
pub struct TaskOpenArgs {
    #[arg(help = "Task ID: 12, p12, pkms:12, or todoist:<remote-id>")]
    pub id: String,
    #[arg(
        long,
        default_value = crate::commands::open::DEFAULT_EDITOR,
        help = "Editor command (default: emacsclient -n)"
    )]
    pub editor: String,
    #[arg(short, long, value_name = "LINE", help = "Line number to open at")]
    pub line: Option<usize>,
}

#[derive(Debug, Args)]
pub struct TaskStateArgs {
    #[arg(help = "Task ID: 12, p12, pkms:12, or todoist:<remote-id>")]
    pub id: String,
    #[arg(help = "TODO state from configured open_todo_states or closed_todo_states")]
    pub state: String,
    #[arg(long, help = "Print the planned change without writing")]
    pub dry_run: bool,
}

#[derive(Debug, Args)]
pub struct TaskDoneArgs {
    #[arg(help = "Task ID: 12, p12, pkms:12, or todoist:<remote-id>")]
    pub id: String,
    #[arg(long, help = "Print the planned change without writing")]
    pub dry_run: bool,
}

#[derive(Debug, Args)]
#[command(
    after_help = "Add modifiers:\n  title:<text>              Task title; non-modifier words are task text\n  state:<state>             PKMS TODO state from configured agenda states\n  tag:<label>, tags:<a,b>   Labels/tags; repeat or comma-separate\n  schedule:<date>, sch:<date>, due:<date>\n  deadline:<date>, dead:<date>, dl:<date>\n  project:<name-or-id>, proj:<name-or-id>, prio:A|B|C, desc:<text>, source:pkms|todoist\n  note:<uuid-title-or-path> PKMS only; choose the note to append into\n  dep:<task-id>, depend:<task-id> PKMS only; add as child of a task\n\nDate shortcuts for schedule/deadline: unambiguous prefixes of today, tomorrow, or weekdays; YYYY-MM-DD; or YYYY-MM-DD HH:MM.\n\nExamples:\n  pkms task add title:\"This is title\" sch:mon dead:to prio:a tag:phone\n  pkms task add dep:2 title:\"Follow up\"\n  pkms task add source:todoist title:\"Call Alice\" proj:Inbox tag:phone prio:b\n  pkms task add note:\"Project Alpha\" title:\"Follow up\""
)]
pub struct TaskAddArgs {
    #[arg(
        help = "Task text and add modifiers such as title:, state:, tag:, sch:, dead:, prio:, project:/proj:, note:, dep:"
    )]
    pub text: Vec<String>,
}

#[derive(Debug, Args)]
pub struct TaskModArgs {
    #[arg(help = "Task ID: 12, p12, pkms:12, or todoist:<remote-id>")]
    pub id: String,
    #[arg(
        help = "Task modifiers such as title:, state:, tag:, sch:, dead:, prio:, project:/proj:, desc:, dep:"
    )]
    pub modifiers: Vec<String>,
}

#[derive(Debug, Args)]
pub struct TaskPostponeArgs {
    #[arg(help = "Task ID: 12, p12, pkms:12, or todoist:<remote-id>")]
    pub id: String,
    #[arg(
        long,
        value_name = "DATE",
        help = "New due date: today, tomorrow, weekday, or YYYY-MM-DD"
    )]
    pub to: String,
}

#[derive(Debug, Args)]
pub struct PathArgs {
    #[arg(help = "Source note (UUID, path, or title)")]
    pub from: Option<String>,
    #[arg(help = "Target note (UUID, path, or title)")]
    pub to: Option<String>,
}
