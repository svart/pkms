use crate::cli::{OutputFormat, QueryArgs};
use crate::command_context::CommandContext;
use crate::output::OutputContext;
use anyhow::Result;
use pkms_db::commands::query::{
    self, QueryOptions, QueryOutput, QuerySearchScope, QueryTodoFilter,
};

pub fn options_from_args(args: &QueryArgs) -> Result<QueryOptions> {
    Ok(QueryOptions {
        terms: args
            .terms
            .clone()
            .ok_or_else(|| anyhow::anyhow!("No search terms specified. Provide terms"))?,
        limit: args.limit,
        scope: QuerySearchScope::from_flags(args.title, args.tags, args.content),
        todo_filter: if args.todos {
            QueryTodoFilter::WithTodos
        } else {
            QueryTodoFilter::All
        },
    })
}

pub fn run(ctx: &CommandContext<'_>, opts: &QueryOptions) -> Result<()> {
    let config = ctx.config().db_command_config();
    let output = query::execute(&config.org, opts)?;
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
