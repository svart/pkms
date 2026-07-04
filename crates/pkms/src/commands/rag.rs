use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

use crate::{
    cli::{
        RagCommand, RagIndexArgs, RagIngestArgs, RagRetrieveArgs, RagRetrieveMode, RagSearchArgs,
        RagServeArgs, RagStatusArgs,
    },
    command_context::CommandContext,
    output::OutputContext,
};

const RAG_DB_ENV: &str = "PKMS_RAG_DB";
const RAG_NOTES_ROOT_ENV: &str = "PKMS_RAG_NOTES_ROOT";
const RAG_INDEX_SOURCE_ENV: &str = "PKMS_RAG_INDEX_SOURCE";
const RAG_HOST_ENV: &str = "PKMS_RAG_HOST";
const RAG_PORT_ENV: &str = "PKMS_RAG_PORT";
const DEFAULT_RAG_HOST: &str = "127.0.0.1";
const DEFAULT_RAG_PORT: u16 = 7337;

pub fn run(command_ctx: &CommandContext<'_>, command: &RagCommand) -> Result<()> {
    match command {
        RagCommand::Status(args) => run_status(command_ctx.output(), args),
        RagCommand::Ingest(args) => run_ingest(command_ctx.output(), args),
        RagCommand::Index(args) => run_index(command_ctx, args),
        RagCommand::Search(args) => run_search(command_ctx.output(), args),
        RagCommand::Retrieve(args) => run_retrieve(command_ctx.output(), args),
        RagCommand::Serve(args) => run_serve(command_ctx, args),
    }
}

fn run_status(ctx: &OutputContext, args: &RagStatusArgs) -> Result<()> {
    let db_path = resolve_rag_db(args.rag_db.as_ref());
    let conn = pkms_rag::connect(&db_path)?;
    let status = pkms_rag::status(&conn, &db_path)?;
    render_status(ctx, &status)
}

fn run_ingest(ctx: &OutputContext, args: &RagIngestArgs) -> Result<()> {
    let db_path = resolve_rag_db(args.rag_db.as_ref());
    let records = pkms_rag::ndjson::load_ndjson(&args.path)?;
    let provider = pkms_rag::provider_from_env()?;
    let mut conn = pkms_rag::connect(&db_path)?;
    let summary = pkms_rag::ingest_records(&mut conn, &records, provider.as_ref(), false)?;
    render_ingest_summary(ctx, &summary)
}

fn run_index(command_ctx: &CommandContext<'_>, args: &RagIndexArgs) -> Result<()> {
    let db_path = resolve_rag_db(args.rag_db.as_ref());
    let (notes_root, index_source) = resolve_index_sources(
        args.notes_root.as_ref(),
        args.index_source.as_ref(),
        Some(command_ctx.config().resolved_db_root()),
    );
    let provider_config = pkms_rag::embedding_provider_config_from_env()?;
    let indexer = pkms_rag::BackgroundIndexer::new(db_path, index_source, notes_root);
    let progress = indexer.run_sync_with_provider_config(&provider_config);
    if progress.phase == "error" {
        bail!(
            "{}",
            progress
                .error
                .clone()
                .unwrap_or_else(|| progress.message.clone())
        );
    }
    render_index_progress(command_ctx.output(), &progress)
}

fn run_search(ctx: &OutputContext, args: &RagSearchArgs) -> Result<()> {
    let db_path = resolve_rag_db(args.rag_db.as_ref());
    let conn = pkms_rag::connect(&db_path)?;
    let results = pkms_rag::search(&conn, &args.query, args.limit)?;
    let response = pkms_rag::SearchResponse {
        query: args.query.clone(),
        results,
    };
    render_search_response(ctx, &response)
}

fn run_retrieve(ctx: &OutputContext, args: &RagRetrieveArgs) -> Result<()> {
    let db_path = resolve_rag_db(args.rag_db.as_ref());
    let conn = pkms_rag::connect(&db_path)?;
    let provider = pkms_rag::provider_from_env()?;
    let request = pkms_rag::RetrieveRequest {
        query: args.query.clone(),
        limit: args.limit,
        mode: retrieve_mode(args.mode),
        max_token_budget: args.max_token_budget,
        weights: pkms_rag::RetrieveWeights::default(),
    };
    let response = pkms_rag::retrieve(&conn, &request, provider.as_ref())?;
    render_retrieve_response(ctx, &response)
}

fn run_serve(command_ctx: &CommandContext<'_>, args: &RagServeArgs) -> Result<()> {
    let _options = RagServeOptions {
        rag_db: resolve_rag_db(args.rag_db.as_ref()),
        notes_root: resolve_notes_root_for_serve(args.notes_root.as_ref(), command_ctx),
        index_source: args
            .index_source
            .clone()
            .or_else(|| env_path(RAG_INDEX_SOURCE_ENV)),
        host: args
            .host
            .clone()
            .or_else(|| non_empty_env(RAG_HOST_ENV))
            .unwrap_or_else(|| DEFAULT_RAG_HOST.to_string()),
        port: args
            .port
            .or_else(|| env_u16(RAG_PORT_ENV))
            .unwrap_or(DEFAULT_RAG_PORT),
    };
    bail!("pkms rag serve is not implemented yet")
}

struct RagServeOptions {
    #[allow(dead_code)]
    rag_db: PathBuf,
    #[allow(dead_code)]
    notes_root: Option<PathBuf>,
    #[allow(dead_code)]
    index_source: Option<PathBuf>,
    #[allow(dead_code)]
    host: String,
    #[allow(dead_code)]
    port: u16,
}

fn render_status(ctx: &OutputContext, status: &pkms_rag::StatusResponse) -> Result<()> {
    if ctx.is_structured() {
        return ctx.print_structured(status);
    }
    println!("RAG database: {}", status.db_path);
    println!("Schema version: {}", status.schema_version);
    println!("Notes: {}", status.notes);
    println!("Chunks: {}", status.chunks);
    println!("Links: {}", status.links);
    println!("Stale chunks: {}", status.stale_chunks);
    println!("FTS rows: {}", status.fts_rows);
    println!("Embeddings: {}", status.embeddings);
    if !status.embedding_models.is_empty() {
        println!("Embedding models: {}", status.embedding_models.join(", "));
    }
    Ok(())
}

fn render_ingest_summary(ctx: &OutputContext, summary: &pkms_rag::IngestSummary) -> Result<()> {
    if ctx.is_structured() {
        return ctx.print_structured(summary);
    }
    println!(
        "Ingested {} notes, {} chunks, {} links",
        summary.notes_seen, summary.chunks_seen, summary.links_seen
    );
    println!(
        "Upserted {} notes, {} chunks; computed {} embeddings",
        summary.notes_upserted, summary.chunks_upserted, summary.embeddings_computed
    );
    if summary.notes_deleted > 0 || summary.chunks_deleted > 0 {
        println!(
            "Deleted {} notes, {} chunks",
            summary.notes_deleted, summary.chunks_deleted
        );
    }
    Ok(())
}

fn render_index_progress(ctx: &OutputContext, progress: &pkms_rag::IndexProgress) -> Result<()> {
    if ctx.is_structured() {
        return ctx.print_structured(progress);
    }
    println!("Index phase: {}", progress.phase);
    println!("Step: {}", progress.current_step);
    if !progress.message.is_empty() {
        println!("{}", progress.message);
    }
    println!(
        "Processed records: {}/{}",
        progress.processed_records, progress.total_records
    );
    println!(
        "Notes: {} seen, {} upserted, {} deleted",
        progress.notes_seen, progress.notes_upserted, progress.notes_deleted
    );
    println!(
        "Chunks: {} seen, {} upserted, {} unchanged, {} deleted",
        progress.chunks_seen,
        progress.chunks_upserted,
        progress.chunks_unchanged,
        progress.chunks_deleted
    );
    println!(
        "Embeddings: {} computed, {} skipped",
        progress.embeddings_computed, progress.embeddings_skipped
    );
    Ok(())
}

fn render_search_response(ctx: &OutputContext, response: &pkms_rag::SearchResponse) -> Result<()> {
    if ctx.is_ndjson() {
        return ctx.print_ndjson(&response.results);
    }
    if ctx.is_json() {
        return ctx.print_json(response);
    }
    if response.results.is_empty() {
        println!("No RAG search results for '{}'.", response.query);
        return Ok(());
    }
    for result in &response.results {
        println!(
            "{} [{}:{}-{}] score {:.4}",
            result.title,
            result.path,
            result.start_line,
            result.end_line,
            result.scores.final_score
        );
        if !result.heading_path.is_empty() {
            println!("  {}", result.heading_path.join(" / "));
        }
        println!("  {}", text_snippet(&result.text, 220));
    }
    Ok(())
}

fn render_retrieve_response(
    ctx: &OutputContext,
    response: &pkms_rag::RetrieveResponse,
) -> Result<()> {
    if ctx.is_ndjson() {
        return ctx.print_ndjson(&response.results);
    }
    if ctx.is_json() {
        return ctx.print_json(response);
    }
    if response.results.is_empty() {
        println!("No RAG retrieval results for '{}'.", response.query);
        return Ok(());
    }
    for result in &response.results {
        let item = &result.result;
        println!(
            "{} [{}:{}-{}] score {:.4} ({})",
            item.title,
            item.path,
            item.start_line,
            item.end_line,
            item.scores.final_score,
            result.reason
        );
        if !item.heading_path.is_empty() {
            println!("  {}", item.heading_path.join(" / "));
        }
        println!("  {}", text_snippet(&item.text, 220));
    }
    Ok(())
}

fn retrieve_mode(mode: RagRetrieveMode) -> pkms_rag::RetrieveMode {
    match mode {
        RagRetrieveMode::Hybrid => pkms_rag::RetrieveMode::Hybrid,
        RagRetrieveMode::Bm25 => pkms_rag::RetrieveMode::Bm25,
        RagRetrieveMode::Dense => pkms_rag::RetrieveMode::Dense,
    }
}

fn resolve_rag_db(rag_db: Option<&PathBuf>) -> PathBuf {
    rag_db
        .cloned()
        .or_else(|| env_path(RAG_DB_ENV))
        .unwrap_or_else(|| PathBuf::from(pkms_rag::DEFAULT_RAG_DB))
}

fn resolve_index_sources(
    notes_root_arg: Option<&PathBuf>,
    index_source_arg: Option<&PathBuf>,
    fallback_notes_root: Option<&Path>,
) -> (Option<PathBuf>, Option<PathBuf>) {
    let notes_root = notes_root_arg
        .cloned()
        .or_else(|| env_path(RAG_NOTES_ROOT_ENV));
    let index_source = index_source_arg
        .cloned()
        .or_else(|| env_path(RAG_INDEX_SOURCE_ENV));
    let notes_root = if notes_root.is_none() && index_source.is_none() {
        fallback_notes_root.map(Path::to_path_buf)
    } else {
        notes_root
    };
    (notes_root, index_source)
}

fn resolve_notes_root_for_serve(
    notes_root_arg: Option<&PathBuf>,
    command_ctx: &CommandContext<'_>,
) -> Option<PathBuf> {
    notes_root_arg
        .cloned()
        .or_else(|| env_path(RAG_NOTES_ROOT_ENV))
        .or_else(|| Some(command_ctx.config().resolved_db_root().to_path_buf()))
}

fn env_path(key: &str) -> Option<PathBuf> {
    non_empty_env(key).map(PathBuf::from)
}

fn non_empty_env(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn env_u16(key: &str) -> Option<u16> {
    non_empty_env(key).and_then(|value| {
        value
            .parse::<u16>()
            .with_context(|| format!("invalid {key} value '{value}'"))
            .ok()
    })
}

fn text_snippet(text: &str, max_chars: usize) -> String {
    let mut snippet = text.chars().take(max_chars).collect::<String>();
    if text.chars().count() > max_chars {
        snippet.push_str("...");
    }
    snippet.replace('\n', " ")
}
