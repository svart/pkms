//! CLI subcommand implementations.
//!
//! Each subcommand in [`Command`](crate::cli::Command) has a corresponding module here
//! with a small `run(...)` entry point. Commands receive shared resolved config and output
//! access through [`CommandContext`](crate::command_context::CommandContext). Commands are
//! stateless: they load domain data when needed, compute results, and dispatch output via
//! [`OutputContext`](crate::output::OutputContext).

pub mod check;
pub mod extract;
pub mod fix;
pub mod get;
pub mod info;
pub mod new;
pub mod orphans;
pub mod path;
pub mod query;
#[cfg(feature = "rag")]
pub mod rag;
pub mod resolve;
#[cfg(feature = "web")]
pub mod serve;
pub mod show;
pub mod stats;
pub mod suggest;
pub mod task;
pub mod task_common;
pub mod validate;
