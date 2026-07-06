use super::{ApiError, AppState, NoteViewerMethod, viewer::note_viewer_response};
use axum::{
    Json, Router,
    body::Bytes,
    extract::{Path as AxumPath, RawQuery, State, rejection::JsonRejection},
    http::header,
    response::{Html, IntoResponse, Response},
    routing::{get, post},
};
use serde::Serialize;
use std::str;

use crate::{
    IngestSummary, RetrieveRequest, RetrieveResponse, SearchRequest, SearchResponse,
    StatusResponse,
    db::{connect, ingest_records, search, status as db_status},
    models::IndexProgress,
    ndjson::parse_ndjson,
    retrieve::retrieve,
    web::{INDEX_HTML, UI_JS},
};

use super::server::display_path;

pub fn router(state: AppState) -> Router {
    tracing::info!(
        event = "rag_api_router_init",
        db_path = %display_path(&state.db_path),
        "initialized RAG API router"
    );
    Router::new()
        .route("/", get(index))
        .route("/ui.js", get(ui_js))
        .route("/preview", get(note_preview))
        .route("/asset", get(note_asset))
        .route("/open", post(note_open))
        .route("/favicon.svg", get(note_favicon))
        .route("/font/{font_name}", get(note_font))
        .route("/health", get(health))
        .route("/status", get(get_status))
        .route("/index/status", get(index_status))
        .route("/index/start", post(start_index))
        .route("/ingest", post(post_ingest))
        .route("/search", post(post_search))
        .route("/retrieve", post(post_retrieve))
        .with_state(state)
}

async fn index(
    State(state): State<AppState>,
    RawQuery(query): RawQuery,
) -> Result<Response, ApiError> {
    if query_has_param(query.as_deref(), "id") && state.note_viewer.is_some() {
        return note_viewer_response(&state, NoteViewerMethod::Get, "/", query);
    }
    Ok(Html(INDEX_HTML).into_response())
}

async fn ui_js() -> impl IntoResponse {
    (
        [(
            header::CONTENT_TYPE,
            "application/javascript; charset=utf-8",
        )],
        UI_JS,
    )
}

async fn note_preview(
    State(state): State<AppState>,
    RawQuery(query): RawQuery,
) -> Result<Response, ApiError> {
    note_viewer_response(&state, NoteViewerMethod::Get, "/preview", query)
}

async fn note_asset(
    State(state): State<AppState>,
    RawQuery(query): RawQuery,
) -> Result<Response, ApiError> {
    note_viewer_response(&state, NoteViewerMethod::Get, "/asset", query)
}

async fn note_open(
    State(state): State<AppState>,
    RawQuery(query): RawQuery,
) -> Result<Response, ApiError> {
    note_viewer_response(&state, NoteViewerMethod::Post, "/open", query)
}

async fn note_favicon(State(state): State<AppState>) -> Result<Response, ApiError> {
    note_viewer_response(&state, NoteViewerMethod::Get, "/favicon.svg", None)
}

async fn note_font(
    State(state): State<AppState>,
    AxumPath(font_name): AxumPath<String>,
) -> Result<Response, ApiError> {
    note_viewer_response(
        &state,
        NoteViewerMethod::Get,
        format!("/font/{font_name}"),
        None,
    )
}

#[derive(Debug, Serialize)]
struct HealthResponse {
    status: &'static str,
}

#[derive(Debug, Serialize)]
struct ApiStatusResponse {
    #[serde(flatten)]
    status: StatusResponse,
    note_viewer_available: bool,
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse { status: "ok" })
}

async fn get_status(State(state): State<AppState>) -> Result<Json<ApiStatusResponse>, ApiError> {
    let conn = connect(&state.db_path).map_err(|err| ApiError::internal("status", err))?;
    let status =
        db_status(&conn, &state.db_path).map_err(|err| ApiError::internal("status", err))?;
    Ok(Json(ApiStatusResponse {
        status,
        note_viewer_available: state.note_viewer.is_some(),
    }))
}

async fn index_status(State(state): State<AppState>) -> Json<IndexProgress> {
    Json(state.indexer.status())
}

async fn start_index(State(state): State<AppState>) -> Json<IndexProgress> {
    tracing::info!(event = "rag_api_index_start", "starting RAG index rebuild");
    Json(state.start_indexing())
}

async fn post_ingest(
    State(state): State<AppState>,
    body: Bytes,
) -> Result<Json<IngestSummary>, ApiError> {
    let body = str::from_utf8(&body)
        .map_err(|err| ApiError::bad_request(format!("invalid UTF-8 request body: {err}")))?;
    let records = parse_ndjson(body).map_err(|err| {
        tracing::warn!(event = "rag_api_ingest_rejected", error = %err, "invalid NDJSON ingest body");
        ApiError::bad_request(err.to_string())
    })?;
    let provider = state
        .provider()
        .map_err(|err| ApiError::internal("ingest_provider", err))?;
    let mut conn = connect(&state.db_path).map_err(|err| ApiError::internal("ingest", err))?;
    let summary = ingest_records(&mut conn, &records, provider.as_ref(), false)
        .map_err(|err| ApiError::internal("ingest", err))?;
    tracing::info!(
        event = "rag_api_ingest_complete",
        records = records.len(),
        notes_upserted = summary.notes_upserted,
        chunks_upserted = summary.chunks_upserted,
        embeddings_computed = summary.embeddings_computed,
        "RAG ingest complete"
    );
    Ok(Json(summary))
}

async fn post_search(
    State(state): State<AppState>,
    payload: Result<Json<SearchRequest>, JsonRejection>,
) -> Result<Json<SearchResponse>, ApiError> {
    let Json(request) = payload.map_err(ApiError::json_rejection)?;
    let conn = connect(&state.db_path).map_err(|err| ApiError::internal("search", err))?;
    let results = search(&conn, &request.query, request.limit)
        .map_err(|err| ApiError::internal("search", err))?;
    tracing::info!(
        event = "rag_api_search_complete",
        query_len = request.query.chars().count(),
        limit = request.limit,
        results = results.len(),
        "RAG search complete"
    );
    Ok(Json(SearchResponse {
        query: request.query,
        results,
    }))
}

async fn post_retrieve(
    State(state): State<AppState>,
    payload: Result<Json<RetrieveRequest>, JsonRejection>,
) -> Result<Json<RetrieveResponse>, ApiError> {
    let Json(request) = payload.map_err(ApiError::json_rejection)?;
    let conn = connect(&state.db_path).map_err(|err| ApiError::internal("retrieve", err))?;
    let provider = state
        .provider()
        .map_err(|err| ApiError::internal("retrieve_provider", err))?;
    let response = retrieve(&conn, &request, provider.as_ref())
        .map_err(|err| ApiError::internal("retrieve", err))?;
    tracing::info!(
        event = "rag_api_retrieve_complete",
        query_len = response.query.chars().count(),
        limit = request.limit,
        mode = ?response.mode,
        results = response.results.len(),
        "RAG retrieve complete"
    );
    Ok(Json(response))
}

fn query_has_param(query: Option<&str>, key: &str) -> bool {
    query.is_some_and(|query| {
        query.split('&').any(|part| {
            part.split_once('=')
                .is_some_and(|(candidate, _)| candidate == key)
        })
    })
}
