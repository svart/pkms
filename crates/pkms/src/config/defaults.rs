use std::path::Path;

pub(super) fn default_open_todo_states() -> Vec<String> {
    vec!["TODO".to_string()]
}

pub(super) fn default_closed_todo_states() -> Vec<String> {
    vec!["DONE".to_string()]
}

pub fn generate_default_config(db_root: Option<&Path>) -> String {
    let root_line = match db_root {
        Some(p) => format!(r#"db_root = "{}""#, p.display()),
        None => r#"# db_root = "/path/to/your/org/directory""#.to_string(),
    };

    format!(
        r#"# pkms configuration
{root_line}

# Directory where new notes are created (relative to db_root or absolute)
# new_notes_dir = "roam"

# Directory where daily notes are created (defaults to new_notes_dir)
# daily_notes_dir = "roam/daily"

# Glob patterns to ignore during file discovery
# ignore_patterns = [".attach", "*.bak"]

# Default task table columns (overridable by --columns flag)
# Available: Id, Date, State, Type, Prio, Tags, Project, Note, Heading
# Global default:
# columns = ["Id", "Date", "State", "Type", "Prio", "Tags", "Project", "Note", "Heading"]
#
# Source/view-specific defaults:
# [columns.pkms]
# tasks = ["Id", "State", "Prio", "Tags", "Note", "Heading"]
# agenda = ["Id", "Date", "State", "Type", "Prio", "Tags", "Note", "Heading"]
#
# [columns.todoist]
# tasks = ["Id", "State", "Prio", "Tags", "Project", "Heading"]
# agenda = ["Id", "Date", "State", "Type", "Prio", "Tags", "Project", "Heading"]

# Task section: configure the PKMS inbox note used by `pkms task inbox` and `pkms task add`
# [tasks]
# inbox = "Inbox"

# Agenda section: configure TODO state keyword lists
# [agenda]
# open_todo_states = ["TODO"]
# closed_todo_states = ["DONE"]

# Todoist is disabled by default. Prefer storing the token in the environment.
# [todoist]
# enabled = false
# token = "..." # optional; env var below takes precedence
# token_env = "TODOIST_API_TOKEN"
# default_filter = "today | overdue"

# RAG retrieval index configuration. Relative paths are resolved under db_root.
# [rag]
# rag_db = ".data/pkms-rag.sqlite3"
# notes_root = "/path/to/org/notes"
# index_source = "retrieval-export.ndjson"

# SSH file-link checks are disabled unless pkms is built with --features ssh and
# `pkms check --remote-file-links` is passed. Password prompts are not used.
# [ssh]
# identity_file = "~/.ssh/id_ed25519"
# known_hosts = "~/.ssh/known_hosts"
# connect_timeout_ms = 5000
# operation_timeout_ms = 5000
# max_connections = 4
# agent = true
"#,
    )
}
