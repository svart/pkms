use clap::{Args, Subcommand};

const TASK_ID_ACTION_HELP: &str = "ID-first actions:\n  pkms task <ID> show\n  pkms task <ID> open [--editor <COMMAND>] [--line <LINE>]\n  pkms task <ID> state <STATE> [--dry-run]\n  pkms task <ID> done [--dry-run]\n  pkms task <ID> postpone --to <DATE>\n  pkms task <ID> mod <MODIFIER>...\n  pkms task <ID> mod dep:<PARENT-ID>";

const TASK_FILTER_HELP_WITH_DATES: &str = "Filters:\n  source:pkms|todoist|all, src:pkms|todoist|all\n  state:TODO|opened|closed, tags:tag,!other, type:SCHED,DEADL, prio:A[,B,C], project:Name\n  date:today|week|overdue|upcoming|YYYY-MM-DD|tom|fri[,value...], after:YYYY-MM-DD[ HH:MM]|tom|fri, before:YYYY-MM-DD[ HH:MM]|tom|fri\n  todoist.filter:<query> (requires source:todoist or source:all)";

const TASK_SIMPLE_FILTER_HELP: &str = "Filters:\n  source:pkms|todoist|all, src:pkms|todoist|all\n  state:TODO|opened|closed, tags:tag,!other, type:SCHED,DEADL, prio:A, project:Name\n  todoist.filter:<query> (requires source:todoist or source:all)";

const TASK_SHARED_MODIFIER_HELP: &str = "  state:<state>             PKMS TODO state from configured agenda states\n  tag:<label>, tags:<a,b>   Labels/tags; repeat or comma-separate\n  schedule:<date>, sch:<date>, due:<date>\n  deadline:<date>, dead:<date>, dl:<date>\n  project:<name-or-id>, proj:<name-or-id>, prio:A|B|C, desc:<text>, source:pkms|todoist\n";

const TASK_DATE_MODIFIER_HELP: &str = "Date shortcuts for schedule/deadline: unambiguous prefixes of today, tomorrow, or weekdays; YYYY-MM-DD; or YYYY-MM-DD HH:MM.";

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
    after_help = [
        TASK_ID_ACTION_HELP,
        "\n\nTask modifiers apply to `task add` and `task <ID> mod`.\n",
        "  title:<text>              Task title; non-modifier words are task text for add only\n",
        TASK_SHARED_MODIFIER_HELP,
        "  note:<uuid-title-or-path> PKMS add only; choose the note to append into\n",
        "  dep:<task-id>, depend:<task-id> PKMS add/mod; add as child or move under parent",
    ]
    .concat()
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
    after_help = [
        TASK_FILTER_HELP_WITH_DATES,
        "\n\nExamples:\n  pkms task list source:todoist tag:phone prio:A\n  pkms task list state:opened,!waiting prio:A,B,C\n  pkms task list source:all date:today,overdue project:Inbox",
    ]
    .concat()
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
    after_help = [
        TASK_FILTER_HELP_WITH_DATES,
        "\n\nExamples:\n  pkms task agenda source:todoist\n  pkms task agenda --days 7\n  pkms task agenda source:all date:today,overdue tag:waiting\n  pkms task agenda state:opened,!waiting prio:A,B,C\n  pkms task agenda today source:todoist",
    ]
    .concat()
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
    #[arg(long, help = "Number of days from today to show, plus overdue tasks")]
    pub days: Option<i64>,
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
    after_help = [
        TASK_SIMPLE_FILTER_HELP,
        "\n\nExamples:\n  pkms task agenda today source:todoist\n  pkms task inbox source:all state:!closed project:Inbox",
    ]
    .concat()
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
    after_help = [
        TASK_SIMPLE_FILTER_HELP,
        "\n\nExamples:\n  pkms task agenda upcoming --days 14 source:all tag:phone\n  pkms task agenda upcoming source:todoist state:opened prio:B",
    ]
    .concat()
)]
pub struct TaskUpcomingArgs {
    #[arg(value_name = "FILTER")]
    pub filters: Vec<String>,
    #[arg(long, help = "Number of upcoming days to show")]
    pub days: Option<i64>,
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
        default_value = crate::editor::DEFAULT_EDITOR,
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
    after_help = [
        "Add modifiers:\n",
        "  title:<text>              Task title; non-modifier words are task text\n",
        TASK_SHARED_MODIFIER_HELP,
        "  note:<uuid-title-or-path> PKMS only; choose the note to append into\n",
        "  dep:<task-id>, depend:<task-id> PKMS only; add as child of a task\n\n",
        TASK_DATE_MODIFIER_HELP,
        "\n\nExamples:\n  pkms task add title:\"This is title\" sch:mon dead:to prio:a tag:phone\n  pkms task add dep:2 title:\"Follow up\"\n  pkms task add source:todoist title:\"Call Alice\" proj:Inbox tag:phone prio:b\n  pkms task add note:\"Project Alpha\" title:\"Follow up\"",
    ]
    .concat()
)]
pub struct TaskAddArgs {
    #[arg(
        help = "Task text and add modifiers such as title:, state:, tag:, sch:, dead:, prio:, project:/proj:, note:, dep:"
    )]
    pub text: Vec<String>,
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
