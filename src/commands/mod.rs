//! CLI subcommand implementations.
//!
//! Each subcommand in [`Command`](crate::cli::Command) has a corresponding module here
//! with a `pub fn run(config, ctx, opts) -> Result<()>` entry point. Commands are
//! stateless — they load the graph from disk on every invocation, compute results, and
//! dispatch output via [`OutputContext`](crate::output::OutputContext).

pub mod agenda;
pub mod check;
pub mod context;
pub mod fix;
pub mod get;
pub mod info;
pub mod new;
pub mod open;
pub mod orphans;
pub mod path;
pub mod query;
pub mod resolve;
pub mod show;
pub mod stats;
pub mod suggest;
pub mod task_common;
pub mod todo;
pub mod validate;
