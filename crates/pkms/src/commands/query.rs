use crate::cli::{OutputFormat, QueryArgs};
use crate::command_context::CommandContext;
use crate::config::ResolvedConfig;
use crate::output::OutputContext;
use anyhow::Result;
use pkms_db::commands::query::{
    self, QueryOptions, QueryOutput, QuerySearchScope, QueryTodoFilter,
};

pub fn options_from_args(args: &QueryArgs, config: &ResolvedConfig) -> Result<QueryOptions> {
    Ok(QueryOptions {
        terms: args
            .terms
            .clone()
            .ok_or_else(|| anyhow::anyhow!("No search terms specified. Provide terms"))?,
        limit: args.limit,
        max_matches_per_note: if args.all_matches {
            None
        } else {
            Some(args.max_matches_per_note.unwrap_or(3))
        },
        scope: QuerySearchScope::from_flags(args.title, args.tags, args.content),
        todo_filter: if args.todos {
            QueryTodoFilter::WithTodos
        } else {
            QueryTodoFilter::All
        },
        scope_filter: super::scope::filter_from_args(&args.scope, config),
    })
}

pub fn run(ctx: &CommandContext<'_>, opts: &QueryOptions) -> Result<()> {
    let org_config = ctx.config().org_config();
    let output = query::execute(&org_config, opts)?;
    render(ctx.output(), &output)
}

pub fn render(ctx: &OutputContext, output: &QueryOutput) -> Result<()> {
    match ctx.format {
        OutputFormat::Text => {
            print!("{}", query::render_text(output));
        }
        OutputFormat::Json => {
            ctx.print_json(output)?;
        }
        OutputFormat::Ndjson => {
            ctx.print_ndjson(&output.results)?;
        }
    }

    Ok(())
}
