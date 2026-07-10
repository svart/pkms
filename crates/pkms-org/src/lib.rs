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
pub mod org_task_edit;
pub mod org_task_mutation;
pub mod parser;
pub mod snapshot;

pub use corpus::{Corpus, FileScanResult};
pub use domain::{LinkTarget, NoteId};
pub use graph::Graph;
pub use parser::{Heading, Link, OrgPriority, OrgTodoState, ParsedNote, ParsedNoteSummary};
pub use snapshot::OrgSnapshot;

#[derive(Debug, Clone)]
pub struct ScanConfig {
    pub db_root: PathBuf,
    pub ignore_patterns: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct LinkResolutionContext {
    pub db_root: PathBuf,
    pub home_dir: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct OrgConfig {
    pub db_root: PathBuf,
    pub ignore_patterns: Vec<String>,
    pub home_dir: Option<PathBuf>,
}

impl OrgConfig {
    pub fn scan_config(&self) -> ScanConfig {
        ScanConfig {
            db_root: self.db_root.clone(),
            ignore_patterns: self.ignore_patterns.clone(),
        }
    }

    pub fn link_resolution_context(&self) -> LinkResolutionContext {
        LinkResolutionContext {
            db_root: self.db_root.clone(),
            home_dir: self.home_dir.clone(),
        }
    }
}
