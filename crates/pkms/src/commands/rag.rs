#[cfg(feature = "web")]
use std::sync::Arc;
use std::{cell::RefCell, io::Write, path::PathBuf};

use anyhow::{Context, Result, bail};

#[cfg(feature = "web")]
use crate::commands::open;
use crate::{
    cli::{
        RagCommand, RagIndexArgs, RagIngestArgs, RagRetrieveArgs, RagRetrieveMode, RagSearchArgs,
        RagServeArgs, RagStatusArgs,
    },
    command_context::CommandContext,
    config::ResolvedConfig,
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
        RagCommand::Status(args) => run_status(command_ctx, args),
        RagCommand::Ingest(args) => run_ingest(command_ctx, args),
        RagCommand::Index(args) => run_index(command_ctx, args),
        RagCommand::Search(args) => run_search(command_ctx, args),
        RagCommand::Retrieve(args) => run_retrieve(command_ctx, args),
        RagCommand::Serve(args) => run_serve(command_ctx, args),
    }
}

fn run_status(command_ctx: &CommandContext<'_>, args: &RagStatusArgs) -> Result<()> {
    let db_path = resolve_rag_db(args.rag_db.as_ref(), command_ctx.config());
    let conn = pkms_rag::connect(&db_path)?;
    let status = pkms_rag::status(&conn, &db_path)?;
    render_status(command_ctx.output(), &status)
}

fn run_ingest(command_ctx: &CommandContext<'_>, args: &RagIngestArgs) -> Result<()> {
    let db_path = resolve_rag_db(args.rag_db.as_ref(), command_ctx.config());
    let records = pkms_rag::load_ndjson(&args.path)?;
    let provider_config = resolve_embedding_provider_config(command_ctx.config())?;
    let provider = pkms_rag::provider_from_config(&provider_config)?;
    let mut conn = pkms_rag::connect(&db_path)?;
    let summary = pkms_rag::ingest_records(&mut conn, &records, provider.as_ref(), false)?;
    render_ingest_summary(command_ctx.output(), &summary)
}

fn run_index(command_ctx: &CommandContext<'_>, args: &RagIndexArgs) -> Result<()> {
    let db_path = resolve_rag_db(args.rag_db.as_ref(), command_ctx.config());
    let (notes_root, index_source) = resolve_index_sources(
        args.notes_root.as_ref(),
        args.index_source.as_ref(),
        command_ctx.config(),
    );
    let provider_config = resolve_embedding_provider_config(command_ctx.config())?;
    let indexer = pkms_rag::BackgroundIndexer::new(db_path, index_source, notes_root);
    let progress = if command_ctx.output().is_structured() {
        indexer.run_sync_with_provider_config(&provider_config)
    } else {
        let progress_printer = RefCell::new(RagIndexProgressPrinter::default());
        indexer.run_sync_with_provider_config_and_progress(&provider_config, |progress| {
            progress_printer.borrow_mut().render(progress);
        })
    };
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

fn run_search(command_ctx: &CommandContext<'_>, args: &RagSearchArgs) -> Result<()> {
    let db_path = resolve_rag_db(args.rag_db.as_ref(), command_ctx.config());
    let conn = pkms_rag::connect(&db_path)?;
    let results = pkms_rag::search(&conn, &args.query, args.limit)?;
    let response = pkms_rag::SearchResponse {
        query: args.query.clone(),
        results,
    };
    render_search_response(command_ctx.output(), &response)
}

fn run_retrieve(command_ctx: &CommandContext<'_>, args: &RagRetrieveArgs) -> Result<()> {
    let db_path = resolve_rag_db(args.rag_db.as_ref(), command_ctx.config());
    let conn = pkms_rag::connect(&db_path)?;
    let provider_config = resolve_embedding_provider_config(command_ctx.config())?;
    let provider = pkms_rag::provider_from_config(&provider_config)?;
    let request = pkms_rag::RetrieveRequest {
        query: args.query.clone(),
        limit: args.limit,
        mode: retrieve_mode(args.mode),
        max_token_budget: args.max_token_budget,
        weights: pkms_rag::RetrieveWeights::default(),
    };
    let response = pkms_rag::retrieve(&conn, &request, provider.as_ref())?;
    render_retrieve_response(command_ctx.output(), &response)
}

fn run_serve(command_ctx: &CommandContext<'_>, args: &RagServeArgs) -> Result<()> {
    let (notes_root, index_source) = resolve_index_sources(
        args.notes_root.as_ref(),
        args.index_source.as_ref(),
        command_ctx.config(),
    );
    let options = pkms_rag::RagServeOptions {
        db_path: resolve_rag_db(args.rag_db.as_ref(), command_ctx.config()),
        notes_root,
        index_source,
        embedding_provider_config: Some(resolve_embedding_provider_config(command_ctx.config())?),
        host: args
            .host
            .clone()
            .or_else(|| non_empty_env(RAG_HOST_ENV))
            .unwrap_or_else(|| DEFAULT_RAG_HOST.to_string()),
        port: resolve_rag_port(args.port)?,
    };
    let output = command_ctx.output();
    #[cfg(feature = "web")]
    {
        let note_viewer = rag_note_viewer(command_ctx, options.notes_root.as_ref())?;
        pkms_rag::serve_with_note_viewer(options, note_viewer, |started| {
            render_serve_started(output, started)
        })
    }
    #[cfg(not(feature = "web"))]
    pkms_rag::serve(options, |started| render_serve_started(output, started))
}

fn render_serve_started(output: &OutputContext, started: &pkms_rag::RagServeStarted) -> Result<()> {
    if output.is_structured() {
        output.print_structured(started)?;
    } else {
        println!("Serving {}", started.url);
        println!("RAG database: {}", started.db_path);
    }
    std::io::stdout().flush()?;
    Ok(())
}

#[cfg(feature = "web")]
fn rag_note_viewer(
    command_ctx: &CommandContext<'_>,
    notes_root: Option<&PathBuf>,
) -> Result<Arc<dyn pkms_rag::NoteViewer>> {
    let mut config = command_ctx.config().web_command_config();
    if let Some(notes_root) = notes_root {
        config.org.db_root = notes_root.clone();
        config.org.new_notes_dir = Some(notes_root.join("roam"));
        config.org.daily_notes_dir = Some(notes_root.join("roam"));
    }
    let viewer = pkms_web::NoteViewer::new(config, open::open_target, open::DEFAULT_EDITOR)?;
    Ok(Arc::new(RagWebNoteViewer { viewer }))
}

#[cfg(feature = "web")]
struct RagWebNoteViewer {
    viewer: pkms_web::NoteViewer,
}

#[cfg(feature = "web")]
impl pkms_rag::NoteViewer for RagWebNoteViewer {
    fn respond(
        &self,
        request: pkms_rag::NoteViewerRequest,
    ) -> Result<pkms_rag::NoteViewerResponse> {
        let response = self.viewer.respond(pkms_web::ViewerRequest {
            method: web_viewer_method(request.method),
            path: request.path,
            query: request.query,
        })?;
        Ok(pkms_rag::NoteViewerResponse {
            status: response.status,
            content_type: response.content_type,
            body: response.body,
        })
    }
}

#[cfg(feature = "web")]
fn web_viewer_method(method: pkms_rag::NoteViewerMethod) -> pkms_web::ViewerMethod {
    match method {
        pkms_rag::NoteViewerMethod::Get => pkms_web::ViewerMethod::Get,
        pkms_rag::NoteViewerMethod::Head => pkms_web::ViewerMethod::Head,
        pkms_rag::NoteViewerMethod::Post => pkms_web::ViewerMethod::Post,
    }
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

#[derive(Default)]
struct RagIndexProgressPrinter {
    last_step: Option<String>,
    last_bucket: Option<u64>,
}

impl RagIndexProgressPrinter {
    fn render(&mut self, progress: &pkms_rag::IndexProgress) {
        let bucket = progress_bucket(progress);
        let step_changed = self.last_step.as_deref() != Some(progress.current_step.as_str());
        let bucket_changed = self.last_bucket != bucket;
        let terminal = matches!(progress.phase.as_str(), "complete" | "error" | "idle");
        if !step_changed && !bucket_changed && !terminal {
            return;
        }
        eprintln!("{}", format_index_progress_line(progress));
        self.last_step = Some(progress.current_step.clone());
        self.last_bucket = bucket;
    }
}

fn progress_bucket(progress: &pkms_rag::IndexProgress) -> Option<u64> {
    if progress.current_step == "embed-chunks" && progress.total_embeddings > 0 {
        return progress
            .processed_embeddings
            .saturating_mul(20)
            .checked_div(progress.total_embeddings);
    }
    progress
        .processed_records
        .saturating_mul(20)
        .checked_div(progress.total_records)
}

fn format_index_progress_line(progress: &pkms_rag::IndexProgress) -> String {
    match progress.current_step.as_str() {
        "ingest-records" if progress.total_records == 0 => {
            "RAG index: Processing records: none found.".to_string()
        }
        "ingest-records" => format!(
            "RAG index: Processing records {}/{} ({})",
            progress.processed_records,
            progress.total_records,
            percent(progress.processed_records, progress.total_records)
        ),
        "embed-chunks" if progress.total_embeddings == 0 => {
            "RAG index: Embedding chunks: none needed.".to_string()
        }
        "embed-chunks" => {
            format!(
                "RAG index: Embedding chunks {}/{} ({})",
                progress.processed_embeddings,
                progress.total_embeddings,
                percent(progress.processed_embeddings, progress.total_embeddings)
            )
        }
        "cleanup-stale" => "RAG index: Removing stale index rows.".to_string(),
        "complete" => format!(
            "RAG index: Complete ({} records, {} chunks, {} embeddings computed).",
            progress.processed_records, progress.chunks_seen, progress.embeddings_computed
        ),
        "error" => format!("RAG index: Error: {}", progress.message),
        _ if !progress.message.is_empty() => format!("RAG index: {}", progress.message),
        _ => format!("RAG index: {}", progress.current_step),
    }
}

fn percent(processed: u64, total: u64) -> String {
    let percent = processed
        .saturating_mul(100)
        .checked_div(total)
        .unwrap_or(0);
    format!("{percent}%")
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

fn resolve_rag_db(rag_db: Option<&PathBuf>, config: &ResolvedConfig) -> PathBuf {
    rag_db
        .cloned()
        .or_else(|| env_path(RAG_DB_ENV))
        .or_else(|| config.resolve_rag_db())
        .unwrap_or_else(|| PathBuf::from(pkms_rag::DEFAULT_RAG_DB))
}

fn resolve_index_sources(
    notes_root_arg: Option<&PathBuf>,
    index_source_arg: Option<&PathBuf>,
    config: &ResolvedConfig,
) -> (Option<PathBuf>, Option<PathBuf>) {
    let notes_root = notes_root_arg
        .cloned()
        .or_else(|| env_path(RAG_NOTES_ROOT_ENV));
    let index_source = index_source_arg
        .cloned()
        .or_else(|| env_path(RAG_INDEX_SOURCE_ENV));
    if notes_root.is_some() || index_source.is_some() {
        return (notes_root, index_source);
    }

    let index_source = config.resolve_rag_index_source();
    if index_source.is_some() {
        return (None, index_source);
    }

    (Some(config.resolved_db_root().to_path_buf()), None)
}

fn resolve_embedding_provider_config(
    config: &ResolvedConfig,
) -> Result<pkms_rag::EmbeddingProviderConfig> {
    pkms_rag::embedding_provider_config_from_env_with_model(config.rag_embedding_model())
        .context("failed to read RAG embedding provider configuration")
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

fn resolve_rag_port(port: Option<u16>) -> Result<u16> {
    if let Some(port) = port {
        return Ok(port);
    }
    if let Some(value) = non_empty_env(RAG_PORT_ENV) {
        return value
            .parse::<u16>()
            .with_context(|| format!("invalid {RAG_PORT_ENV} value '{value}'"));
    }
    Ok(DEFAULT_RAG_PORT)
}

fn text_snippet(text: &str, max_chars: usize) -> String {
    let mut snippet = text.chars().take(max_chars).collect::<String>();
    if text.chars().count() > max_chars {
        snippet.push_str("...");
    }
    snippet.replace('\n', " ")
}
