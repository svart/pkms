use std::{
    path::{Path, PathBuf},
    str,
};

use anyhow::{Context, Result};
use axum::{
    Json, Router,
    body::Bytes,
    extract::{State, rejection::JsonRejection},
    http::{StatusCode, header},
    response::{Html, IntoResponse, Response},
    routing::{get, post},
};
use serde::Serialize;

use crate::{
    BackgroundIndexer, EmbeddingProviderConfig, IngestSummary, RetrieveRequest, RetrieveResponse,
    SearchRequest, SearchResponse, StatusResponse,
    db::{connect, ingest_records, search, status as db_status},
    embeddings::{embedding_provider_config_from_env, provider_from_config},
    models::IndexProgress,
    ndjson::parse_ndjson,
    retrieve::retrieve,
};

pub const DEFAULT_RAG_DB: &str = ".data/pkms-rag.sqlite3";

const RAG_DB_ENV: &str = "PKMS_RAG_DB";
const RAG_INDEX_SOURCE_ENV: &str = "PKMS_RAG_INDEX_SOURCE";
const RAG_NOTES_ROOT_ENV: &str = "PKMS_RAG_NOTES_ROOT";

const INDEX_HTML: &str = r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>PKMS Search</title>
</head>
<body>
  <main>
    <h1>PKMS Search</h1>
    <section aria-live="polite">
      <strong id="phase">Loading status</strong>
      <p id="details"></p>
    </section>
  </main>
  <script src="/ui.js"></script>
</body>
</html>"#;

const UI_JS: &str = r#"async function refreshIndexStatus() {
  const response = await fetch("/index/status");
  const body = await response.json();
  document.getElementById("phase").textContent = `${body.phase}: ${body.current_step || "idle"}`;
  document.getElementById("details").textContent = `${body.message || ""} records: ${body.processed_records}/${body.total_records || "?"}`;
}
refreshIndexStatus().catch((error) => {
  document.getElementById("phase").textContent = "error";
  document.getElementById("details").textContent = error.message;
});"#;

#[derive(Debug, Clone)]
pub struct AppState {
    db_path: PathBuf,
    indexer: BackgroundIndexer,
    embedding_provider_config: EmbeddingProviderConfig,
    synchronous_indexing: bool,
}

impl AppState {
    pub fn from_env() -> Result<Self> {
        let db_path = std::env::var(RAG_DB_ENV)
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from(DEFAULT_RAG_DB));
        let index_source = env_path(RAG_INDEX_SOURCE_ENV);
        let notes_root = env_path(RAG_NOTES_ROOT_ENV);
        let embedding_provider_config = embedding_provider_config_from_env()
            .context("failed to read RAG embedding provider configuration")?;
        Ok(Self::with_embedding_provider_config(
            db_path,
            index_source,
            notes_root,
            embedding_provider_config,
        ))
    }

    pub fn with_embedding_provider_config(
        db_path: impl Into<PathBuf>,
        index_source: Option<PathBuf>,
        notes_root: Option<PathBuf>,
        embedding_provider_config: EmbeddingProviderConfig,
    ) -> Self {
        let db_path = db_path.into();
        let indexer = BackgroundIndexer::new(db_path.clone(), index_source, notes_root);
        Self {
            db_path,
            indexer,
            embedding_provider_config,
            synchronous_indexing: false,
        }
    }

    pub fn with_synchronous_indexing(mut self, synchronous_indexing: bool) -> Self {
        self.synchronous_indexing = synchronous_indexing;
        self
    }

    fn provider(&self) -> Result<Box<dyn crate::EmbeddingProvider>> {
        provider_from_config(&self.embedding_provider_config)
    }

    fn start_indexing(&self) -> IndexProgress {
        if self.synchronous_indexing {
            self.indexer
                .run_sync_with_provider_config(&self.embedding_provider_config)
        } else {
            self.indexer
                .start_with_provider_config(self.embedding_provider_config.clone())
        }
    }
}

pub fn router(state: AppState) -> Router {
    tracing::info!(
        event = "rag_api_router_init",
        db_path = %display_path(&state.db_path),
        "initialized RAG API router"
    );
    Router::new()
        .route("/", get(index))
        .route("/ui.js", get(ui_js))
        .route("/health", get(health))
        .route("/status", get(get_status))
        .route("/index/status", get(index_status))
        .route("/index/start", post(start_index))
        .route("/ingest", post(post_ingest))
        .route("/search", post(post_search))
        .route("/retrieve", post(post_retrieve))
        .with_state(state)
}

async fn index() -> Html<&'static str> {
    Html(INDEX_HTML)
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

#[derive(Debug, Serialize)]
struct HealthResponse {
    status: &'static str,
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse { status: "ok" })
}

async fn get_status(State(state): State<AppState>) -> Result<Json<StatusResponse>, ApiError> {
    let conn = connect(&state.db_path).map_err(|err| ApiError::internal("status", err))?;
    let status =
        db_status(&conn, &state.db_path).map_err(|err| ApiError::internal("status", err))?;
    Ok(Json(status))
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

#[derive(Debug)]
pub struct ApiError {
    status: StatusCode,
    detail: String,
}

#[derive(Debug, Serialize)]
struct ErrorResponse {
    detail: String,
}

impl ApiError {
    fn bad_request(detail: String) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            detail,
        }
    }

    fn json_rejection(rejection: JsonRejection) -> Self {
        Self::bad_request(rejection.body_text())
    }

    fn internal(operation: &'static str, err: anyhow::Error) -> Self {
        tracing::error!(
            event = "rag_api_request_failed",
            operation,
            error = %err,
            "RAG API request failed"
        );
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            detail: err.to_string(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(ErrorResponse {
                detail: self.detail,
            }),
        )
            .into_response()
    }
}

fn env_path(key: &str) -> Option<PathBuf> {
    std::env::var(key)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .map(PathBuf::from)
}

fn display_path(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::{Body, to_bytes},
        http::{Method, Request},
    };
    use serde::de::DeserializeOwned;
    use serde_json::{Value, json};
    use tower::ServiceExt;

    #[tokio::test]
    async fn api_serves_health_ui_and_idle_index_status() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let app = test_router(tempdir.path().join("api.sqlite3"), None, None);

        let health = json_request(app.clone(), Method::GET, "/health", Body::empty(), None).await;
        assert_eq!(health.0, StatusCode::OK);
        assert_eq!(health.1, json!({"status": "ok"}));

        let page = text_request(app.clone(), Method::GET, "/", Body::empty()).await;
        assert_eq!(page.0, StatusCode::OK);
        assert!(page.1.contains("PKMS Search"));

        let script = text_request(app.clone(), Method::GET, "/ui.js", Body::empty()).await;
        assert_eq!(script.0, StatusCode::OK);
        assert!(script.1.contains("fetch(\"/index/status\")"));

        let index_status =
            json_request(app, Method::GET, "/index/status", Body::empty(), None).await;
        assert_eq!(index_status.0, StatusCode::OK);
        assert_eq!(index_status.1["phase"], "idle");
    }

    #[tokio::test]
    async fn api_ingest_status_search_and_retrieve() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let db_path = tempdir.path().join("api.sqlite3");
        let app = test_router(db_path, None, None);

        let ingest = json_request(
            app.clone(),
            Method::POST,
            "/ingest",
            Body::from(fixture_text()),
            Some("application/x-ndjson"),
        )
        .await;
        assert_eq!(ingest.0, StatusCode::OK);
        assert_eq!(ingest.1["chunks_upserted"], 2);

        let status = json_request(app.clone(), Method::GET, "/status", Body::empty(), None).await;
        assert_eq!(status.0, StatusCode::OK);
        assert_eq!(status.1["chunks"], 2);
        assert_eq!(status.1["embeddings"], 2);

        let search = json_request(
            app.clone(),
            Method::POST,
            "/search",
            json_body(json!({"query": "UrlBase externalHostname", "limit": 5})),
            Some("application/json"),
        )
        .await;
        assert_eq!(search.0, StatusCode::OK);
        assert_eq!(search.1["query"], "UrlBase externalHostname");
        assert_eq!(
            search.1["results"][0]["title"],
            "Media Library Migration to Jellyfin"
        );

        let retrieve = json_request(
            app,
            Method::POST,
            "/retrieve",
            json_body(json!({"query": "agenda inspect tasks", "limit": 5, "mode": "hybrid"})),
            Some("application/json"),
        )
        .await;
        assert_eq!(retrieve.0, StatusCode::OK);
        assert_eq!(retrieve.1["query"], "agenda inspect tasks");
        assert_eq!(retrieve.1["results"][0]["title"], "PKMS Task Backend");
        assert!(
            retrieve.1["results"][0]["scores"]["final"]
                .as_f64()
                .unwrap()
                > 0.0
        );
    }

    #[tokio::test]
    async fn api_rejects_invalid_ndjson_with_detail() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let app = test_router(tempdir.path().join("api.sqlite3"), None, None);

        let response = json_request(
            app,
            Method::POST,
            "/ingest",
            Body::from("not json\n"),
            Some("application/x-ndjson"),
        )
        .await;

        assert_eq!(response.0, StatusCode::BAD_REQUEST);
        assert!(
            response.1["detail"]
                .as_str()
                .unwrap()
                .contains("Invalid NDJSON")
        );
    }

    #[tokio::test]
    async fn api_index_start_rebuilds_from_notes_root_and_removes_deleted_notes() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let notes_root = tempdir.path().join("notes");
        std::fs::create_dir(&notes_root).expect("notes root creates");
        let note_path = notes_root.join("api-semantic-search.org");
        std::fs::write(
            &note_path,
            "\
#+title: API Semantic Search
:PROPERTIES:
:ID: eeeeeeee-eeee-4eee-eeee-eeeeeeeeeeee
:END:
* API usage
Agents call /retrieve to search mounted PKMS notes.
",
        )
        .expect("note writes");
        let db_path = tempdir.path().join("api.sqlite3");
        let state = test_state(db_path, None, Some(notes_root)).with_synchronous_indexing(true);
        let app = router(state);

        let first = json_request(
            app.clone(),
            Method::POST,
            "/index/start",
            Body::empty(),
            None,
        )
        .await;
        assert_eq!(first.0, StatusCode::OK);
        assert_eq!(first.1["phase"], "complete");
        assert_eq!(first.1["chunks_seen"], 1);

        let retrieve = json_request(
            app.clone(),
            Method::POST,
            "/retrieve",
            json_body(
                json!({"query": "agents retrieve mounted notes", "limit": 5, "mode": "hybrid"}),
            ),
            Some("application/json"),
        )
        .await;
        assert_eq!(retrieve.0, StatusCode::OK);
        assert_eq!(
            retrieve.1["results"][0]["note_id"],
            "eeeeeeee-eeee-4eee-eeee-eeeeeeeeeeee"
        );

        std::fs::remove_file(note_path).expect("note removes");
        let second = json_request(
            app.clone(),
            Method::POST,
            "/index/start",
            Body::empty(),
            None,
        )
        .await;
        assert_eq!(second.0, StatusCode::OK);
        assert_eq!(second.1["phase"], "complete");
        assert_eq!(second.1["chunks_deleted"], 1);

        let status = json_request(app, Method::GET, "/status", Body::empty(), None).await;
        assert_eq!(status.0, StatusCode::OK);
        assert_eq!(status.1["notes"], 0);
        assert_eq!(status.1["chunks"], 0);
    }

    fn test_router(
        db_path: PathBuf,
        index_source: Option<PathBuf>,
        notes_root: Option<PathBuf>,
    ) -> Router {
        router(test_state(db_path, index_source, notes_root))
    }

    fn test_state(
        db_path: PathBuf,
        index_source: Option<PathBuf>,
        notes_root: Option<PathBuf>,
    ) -> AppState {
        AppState::with_embedding_provider_config(
            db_path,
            index_source,
            notes_root,
            EmbeddingProviderConfig::Hash,
        )
    }

    async fn json_request(
        app: Router,
        method: Method,
        uri: &str,
        body: Body,
        content_type: Option<&'static str>,
    ) -> (StatusCode, Value) {
        response_body(app, method, uri, body, content_type).await
    }

    async fn text_request(
        app: Router,
        method: Method,
        uri: &str,
        body: Body,
    ) -> (StatusCode, String) {
        let (status, body) = raw_response_body(app, method, uri, body, None).await;
        (
            status,
            String::from_utf8(body).expect("response body is UTF-8"),
        )
    }

    async fn response_body<T: DeserializeOwned>(
        app: Router,
        method: Method,
        uri: &str,
        body: Body,
        content_type: Option<&'static str>,
    ) -> (StatusCode, T) {
        let (status, body) = raw_response_body(app, method, uri, body, content_type).await;
        (
            status,
            serde_json::from_slice(&body).expect("response body parses as JSON"),
        )
    }

    async fn raw_response_body(
        app: Router,
        method: Method,
        uri: &str,
        body: Body,
        content_type: Option<&'static str>,
    ) -> (StatusCode, Vec<u8>) {
        let mut builder = Request::builder().method(method).uri(uri);
        if let Some(content_type) = content_type {
            builder = builder.header(header::CONTENT_TYPE, content_type);
        }
        let response = app
            .oneshot(builder.body(body).expect("request builds"))
            .await
            .expect("router responds");
        let status = response.status();
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("response body reads")
            .to_vec();
        (status, body)
    }

    fn json_body(value: Value) -> Body {
        Body::from(serde_json::to_vec(&value).expect("json body serializes"))
    }

    fn fixture_text() -> String {
        std::fs::read_to_string(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/retrieval-export.ndjson"),
        )
        .expect("fixture reads")
    }
}
