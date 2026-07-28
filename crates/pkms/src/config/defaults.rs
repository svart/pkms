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
# Task section: configure the PKMS inbox note used by `pkms task inbox` and `pkms task add`
# [tasks]
# inbox = "Inbox"

# Agenda section: configure TODO state keyword lists
# [agenda]
# open_todo_states = ["TODO"]
# closed_todo_states = ["DONE"]

# RAG retrieval index configuration. Relative paths are resolved under db_root.
# [rag]
# rag_db = ".data/pkms-rag.sqlite3"
# embedding_model = "sentence-transformers/paraphrase-multilingual-MiniLM-L12-v2"
# fastembed_model_dir = "models/paraphrase-multilingual-MiniLM-L12-v2"
"#
    )
}
