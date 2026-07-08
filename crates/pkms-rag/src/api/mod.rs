mod error;
mod routes;
mod server;
mod state;
mod viewer;

pub use error::ApiError;
pub use routes::router;
pub use server::{RagServeOptions, RagServeStarted, serve, serve_with_note_viewer};
pub use state::{AppState, DEFAULT_RAG_DB};
pub use viewer::{NoteViewer, NoteViewerMethod, NoteViewerRequest, NoteViewerResponse};

#[cfg(test)]
use server::serve_until;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::EmbeddingProviderConfig;
    use anyhow::Result;
    use axum::{
        Router,
        body::{Body, to_bytes},
        http::{Method, Request, StatusCode, header},
    };
    use serde::de::DeserializeOwned;
    use serde_json::{Value, json};
    use std::{path::PathBuf, sync::Arc};
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
        assert!(script.1.contains("fetch(\"/retrieve\""));
        assert!(script.1.contains("let noteViewerAvailable = false;"));
        assert!(
            script
                .1
                .contains("noteViewerAvailable = statusBody.note_viewer_available === true;")
        );
        assert!(script.1.contains(
            "document.createElement(noteViewerAvailable && item.uuid ? \"a\" : \"div\")"
        ));
        assert!(script.1.contains("noteHref(item.uuid)"));
        assert!(script.1.contains("title.href = noteHref(item.uuid)"));

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
        assert_eq!(status.1["note_viewer_available"], false);

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
    async fn api_renders_search_ui_for_note_url_when_viewer_unavailable() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let app = test_router(tempdir.path().join("api.sqlite3"), None, None);

        let page = text_request(
            app,
            Method::GET,
            "/?id=aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa",
            Body::empty(),
        )
        .await;

        assert_eq!(page.0, StatusCode::OK);
        assert!(page.1.contains("PKMS Search"));
        assert!(!page.1.contains("Note viewer unavailable"));
    }

    #[tokio::test]
    async fn api_delegates_note_viewer_routes_when_configured() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let state = test_state(tempdir.path().join("api.sqlite3"), None, None)
            .with_note_viewer(Arc::new(FakeNoteViewer));
        let app = router(state);

        let status = json_request(app.clone(), Method::GET, "/status", Body::empty(), None).await;
        assert_eq!(status.0, StatusCode::OK);
        assert_eq!(status.1["note_viewer_available"], true);

        let note = text_request(
            app.clone(),
            Method::GET,
            "/?id=aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa",
            Body::empty(),
        )
        .await;
        assert_eq!(note.0, StatusCode::OK);
        assert!(
            note.1
                .contains("viewer Get / id=aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa")
        );

        let preview = text_request(
            app.clone(),
            Method::GET,
            "/preview?id=bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb",
            Body::empty(),
        )
        .await;
        assert_eq!(preview.0, StatusCode::OK);
        assert!(
            preview
                .1
                .contains("viewer Get /preview id=bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb")
        );

        let open = text_request(
            app.clone(),
            Method::POST,
            "/open?id=cccccccc-cccc-4ccc-cccc-cccccccccccc",
            Body::empty(),
        )
        .await;
        assert_eq!(open.0, StatusCode::OK);
        assert!(
            open.1
                .contains("viewer Post /open id=cccccccc-cccc-4ccc-cccc-cccccccccccc")
        );

        let favicon = text_request(app.clone(), Method::GET, "/favicon.svg", Body::empty()).await;
        assert_eq!(favicon.0, StatusCode::OK);
        assert!(favicon.1.contains("viewer Get /favicon.svg"));

        let font = text_request(app, Method::GET, "/font/Alegreya.ttf", Body::empty()).await;
        assert_eq!(font.0, StatusCode::OK);
        assert!(font.1.contains("viewer Get /font/Alegreya.ttf"));
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
            retrieve.1["results"][0]["uuid"],
            "eeeeeeee-eeee-4eee-eeee-eeeeeeeeeeee"
        );
        assert!(retrieve.1["results"][0].get("note_id").is_none());

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

    #[tokio::test]
    async fn api_serve_binds_tcp_listener_and_serves_health() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let options = RagServeOptions {
            db_path: tempdir.path().join("api.sqlite3"),
            index_source: None,
            notes_root: None,
            embedding_provider_config: Some(EmbeddingProviderConfig::Hash),
            host: "127.0.0.1".to_string(),
            port: 0,
        };
        let state = test_state(options.db_path.clone(), None, None);
        let (started, shutdown_tx, handle) = spawn_test_server(options, state).await;

        let response = http_get(&started, "/health").await;
        assert!(response.starts_with("HTTP/1.1 200 OK"));
        assert!(response.contains("{\"status\":\"ok\"}"));

        shutdown_tx.send(()).expect("shutdown sends");
        handle
            .await
            .expect("server task joins")
            .expect("server exits");
    }

    #[tokio::test]
    async fn api_serve_starts_configured_indexer_on_launch() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let notes_root = tempdir.path().join("notes");
        std::fs::create_dir(&notes_root).expect("notes root creates");
        std::fs::write(
            notes_root.join("startup-rag.org"),
            "\
#+title: Startup RAG
:PROPERTIES:
:ID: ffffffff-ffff-4fff-ffff-ffffffffffff
:END:
* Startup index
Serve startup rebuilds the configured RAG index.
",
        )
        .expect("note writes");
        let options = RagServeOptions {
            db_path: tempdir.path().join("api.sqlite3"),
            index_source: None,
            notes_root: Some(notes_root.clone()),
            embedding_provider_config: Some(EmbeddingProviderConfig::Hash),
            host: "127.0.0.1".to_string(),
            port: 0,
        };
        let state = test_state(options.db_path.clone(), None, Some(notes_root))
            .with_synchronous_indexing(true);
        let (started, shutdown_tx, handle) = spawn_test_server(options, state).await;

        let index_status = http_get(&started, "/index/status").await;
        assert!(index_status.contains("\"phase\":\"complete\""));
        assert!(index_status.contains("\"chunks_seen\":1"));
        let status = http_get(&started, "/status").await;
        assert!(status.contains("\"notes\":1"));
        assert!(status.contains("\"chunks\":1"));

        shutdown_tx.send(()).expect("shutdown sends");
        handle
            .await
            .expect("server task joins")
            .expect("server exits");
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

    struct FakeNoteViewer;

    impl NoteViewer for FakeNoteViewer {
        fn respond(&self, request: NoteViewerRequest) -> Result<NoteViewerResponse> {
            Ok(NoteViewerResponse {
                status: 200,
                content_type: "text/html; charset=utf-8".to_string(),
                body: format!(
                    "viewer {:?} {} {}",
                    request.method,
                    request.path,
                    request.query.unwrap_or_default()
                )
                .into_bytes(),
            })
        }
    }

    async fn spawn_test_server(
        options: RagServeOptions,
        state: AppState,
    ) -> (
        RagServeStarted,
        tokio::sync::oneshot::Sender<()>,
        tokio::task::JoinHandle<Result<()>>,
    ) {
        let (started_tx, started_rx) = tokio::sync::oneshot::channel();
        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
        let handle = tokio::spawn(async move {
            serve_until(
                options,
                state,
                |started| {
                    started_tx
                        .send(started.clone())
                        .map_err(|_| anyhow::anyhow!("failed to send startup event"))?;
                    Ok(())
                },
                async move {
                    let _ = shutdown_rx.await;
                },
            )
            .await
        });
        let started = started_rx.await.expect("server starts");
        (started, shutdown_tx, handle)
    }

    async fn http_get(started: &RagServeStarted, path: &'static str) -> String {
        let host = started.host.clone();
        let port = started.port;
        tokio::task::spawn_blocking(move || {
            use std::io::{Read, Write};

            let mut stream = std::net::TcpStream::connect((host.as_str(), port))?;
            write!(
                stream,
                "GET {path} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n\r\n"
            )?;
            let mut response = String::new();
            stream.read_to_string(&mut response)?;
            Ok::<_, anyhow::Error>(response)
        })
        .await
        .expect("client task joins")
        .expect("request succeeds")
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
