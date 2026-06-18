//! CLI subcommand implementations.
//!
//! Each subcommand in [`Command`](crate::cli::Command) has a corresponding module here
//! with a small `run(...)` entry point. Most commands accept resolved config, output
//! context, and typed options; commands that benefit from shared loaders can accept
//! [`CommandContext`](crate::command_context::CommandContext). Commands are stateless:
//! they load the graph or workspace from disk when needed, compute results, and dispatch
//! output via [`OutputContext`](crate::output::OutputContext).

pub mod check;
pub mod extract;
pub mod fix;
pub mod get;
pub mod info;
pub mod new;
pub mod open;
pub mod orphans;
pub mod path;
pub mod query;
pub mod resolve;
#[cfg(feature = "web")]
pub mod serve;
pub mod show;
pub mod stats;
pub mod suggest;
pub mod task;
pub mod task_common;
pub mod task_index;
pub mod validate;
