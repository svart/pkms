//! Org parsing, discovery, corpus loading, and edit primitives for pkms.

use std::path::PathBuf;

pub mod corpus;
pub mod discovery;
pub mod domain;
pub mod org_date;
pub mod org_edit;
pub mod parser;
pub mod tokens;

pub use corpus::{Corpus, FileScanResult};
pub use domain::{LinkTarget, NoteId};
pub use parser::{Heading, Link, OrgPriority, OrgTodoState, ParsedNote, ParsedNoteSummary};

#[derive(Debug, Clone)]
pub struct OrgConfig {
    pub db_root: PathBuf,
    pub new_notes_dir: Option<PathBuf>,
    pub daily_notes_dir: Option<PathBuf>,
    pub ignore_patterns: Vec<String>,
}
