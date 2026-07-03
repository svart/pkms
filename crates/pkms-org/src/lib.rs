//! Org parsing, discovery, corpus loading, and edit primitives for pkms.

use std::path::PathBuf;

pub mod attachments;
pub mod corpus;
pub mod discovery;
pub mod domain;
pub mod graph;
pub mod link_check;
pub mod org_date;
pub mod org_edit;
pub mod parser;
pub mod tokens;
pub mod workspace;

pub use corpus::{Corpus, FileScanResult};
pub use domain::{LinkTarget, NoteId};
pub use graph::Graph;
pub use parser::{Heading, Link, OrgPriority, OrgTodoState, ParsedNote, ParsedNoteSummary};
pub use workspace::Workspace;

#[derive(Debug, Clone)]
pub struct OrgConfig {
    pub db_root: PathBuf,
    pub new_notes_dir: Option<PathBuf>,
    pub daily_notes_dir: Option<PathBuf>,
    pub ignore_patterns: Vec<String>,
}
