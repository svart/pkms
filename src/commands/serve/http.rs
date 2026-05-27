use super::{assets, percent_decode, render_note_html, render_preview_html};
use crate::commands::open;
use crate::config::ResolvedConfig;
use crate::graph::{Graph, resolve_file_link_path};
use anyhow::{Context, Result};
use std::io::{self, BufRead, BufReader, Write};
use std::net::TcpStream;

pub(super) struct ServeState<'a> {
    pub(super) config: &'a ResolvedConfig,
    pub(super) graph: Graph,
    pub(super) initial_uuid: String,
}

pub(super) fn log_request_error(err: &anyhow::Error) {
    if is_client_disconnect(err) {
        tracing::debug!(error = %err, "serve client disconnected before response completed");
    } else {
        tracing::warn!(error = %err, "serve request failed");
    }
}

pub(super) fn log_connection_error(err: &io::Error) {
    if is_client_disconnect_kind(err.kind()) {
        tracing::debug!(error = %err, "serve client disconnected before request handling");
    } else {
        tracing::warn!(error = %err, "serve connection failed");
    }
}

pub(super) fn is_client_disconnect(err: &anyhow::Error) -> bool {
    err.chain().any(|cause| {
        cause
            .downcast_ref::<io::Error>()
            .is_some_and(|err| is_client_disconnect_kind(err.kind()))
    })
}

fn is_client_disconnect_kind(kind: io::ErrorKind) -> bool {
    matches!(
        kind,
        io::ErrorKind::BrokenPipe
            | io::ErrorKind::ConnectionReset
            | io::ErrorKind::ConnectionAborted
    )
}

pub(super) fn handle_connection(mut stream: TcpStream, state: &ServeState<'_>) -> Result<()> {
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut request_line = String::new();
    reader.read_line(&mut request_line)?;
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or_default();
    let target = parts.next().unwrap_or("/");
    if method != "GET" && method != "HEAD" && method != "POST" {
        return write_response(
            &mut stream,
            405,
            "text/plain; charset=utf-8",
            b"Method not allowed",
        );
    }

    let (path, query) = split_target(target);
    let response = if method == "POST" {
        match path {
            "/open" => open_response(state, query, open::DEFAULT_EDITOR),
            _ => Ok(HttpResponse::method_not_allowed("Method not allowed")),
        }
    } else if let Some(font_name) = path.strip_prefix("/font/") {
        Ok(assets::font_response(font_name))
    } else {
        match path {
            "/" => render_response(state, query),
            "/preview" => preview_response(state, query),
            "/asset" => asset_response(state, query),
            "/open" => Ok(HttpResponse::method_not_allowed("Method not allowed")),
            _ => Ok(HttpResponse::not_found("Not found")),
        }
    }?;

    if method == "HEAD" {
        write_headers(&mut stream, response.status, response.content_type, 0)
    } else {
        write_response(
            &mut stream,
            response.status,
            response.content_type,
            &response.body,
        )
    }
}

pub(super) struct HttpResponse {
    pub(super) status: u16,
    pub(super) content_type: &'static str,
    pub(super) body: Vec<u8>,
}

impl HttpResponse {
    fn html(body: String) -> Self {
        Self {
            status: 200,
            content_type: "text/html; charset=utf-8",
            body: body.into_bytes(),
        }
    }

    pub(super) fn not_found(message: &str) -> Self {
        Self {
            status: 404,
            content_type: "text/plain; charset=utf-8",
            body: message.as_bytes().to_vec(),
        }
    }

    fn method_not_allowed(message: &str) -> Self {
        Self {
            status: 405,
            content_type: "text/plain; charset=utf-8",
            body: message.as_bytes().to_vec(),
        }
    }

    fn text(message: String) -> Self {
        Self {
            status: 200,
            content_type: "text/plain; charset=utf-8",
            body: message.into_bytes(),
        }
    }
}

fn render_response(state: &ServeState<'_>, query: Option<&str>) -> Result<HttpResponse> {
    let requested = query_param(query, "id").unwrap_or_else(|| state.initial_uuid.clone());
    let node = state.graph.resolve_target(&requested)?;
    let content = std::fs::read_to_string(&node.path)
        .with_context(|| format!("Failed to read {}", node.path.display()))?;
    Ok(HttpResponse::html(render_note_html(
        &state.graph,
        state.config,
        node,
        &content,
    )))
}

fn preview_response(state: &ServeState<'_>, query: Option<&str>) -> Result<HttpResponse> {
    let Some(requested) = query_param(query, "id") else {
        return Ok(HttpResponse::not_found("Missing id"));
    };
    let node = state.graph.resolve_target(&requested)?;
    let content = std::fs::read_to_string(&node.path)
        .with_context(|| format!("Failed to read {}", node.path.display()))?;
    Ok(HttpResponse::html(render_preview_html(
        &state.graph,
        state.config,
        node,
        &content,
    )))
}

fn asset_response(state: &ServeState<'_>, query: Option<&str>) -> Result<HttpResponse> {
    let Some(note_uuid) = query_param(query, "note") else {
        return Ok(HttpResponse::not_found("Missing note"));
    };
    let Some(kind) = query_param(query, "kind") else {
        return Ok(HttpResponse::not_found("Missing kind"));
    };
    let Some(target) = query_param(query, "target") else {
        return Ok(HttpResponse::not_found("Missing target"));
    };
    let note = state.graph.resolve_target(&note_uuid)?;
    let path = match kind.as_str() {
        "file" => resolve_file_link_path(&target, &note.path, state.config.resolved_db_root()),
        "attachment" => assets::resolve_existing_attachment(
            state.config.resolved_db_root(),
            &note.uuid,
            &target,
        ),
        _ => return Ok(HttpResponse::not_found("Unknown asset kind")),
    };
    if !assets::is_asset_allowed(&path, state.config.resolved_db_root()) || !path.is_file() {
        return Ok(HttpResponse::not_found("Asset not found"));
    }
    let body = std::fs::read(&path)?;
    Ok(HttpResponse {
        status: 200,
        content_type: assets::mime_type(&path),
        body,
    })
}

pub(super) fn open_response(
    state: &ServeState<'_>,
    query: Option<&str>,
    editor: &str,
) -> Result<HttpResponse> {
    let Some(note_uuid) = query_param(query, "id") else {
        return Ok(HttpResponse::not_found("Missing id"));
    };
    let node = state.graph.resolve_target(&note_uuid)?;
    open::open_target(&state.graph, state.config, &node.uuid, editor, Some(1))?;
    Ok(HttpResponse::text(format!("Opened {}", node.title)))
}

fn split_target(target: &str) -> (&str, Option<&str>) {
    match target.split_once('?') {
        Some((path, query)) => (path, Some(query)),
        None => (target, None),
    }
}

fn query_param(query: Option<&str>, key: &str) -> Option<String> {
    query?.split('&').find_map(|part| {
        let (k, v) = part.split_once('=')?;
        (k == key).then(|| percent_decode(v))
    })
}

fn write_response(
    stream: &mut TcpStream,
    status: u16,
    content_type: &'static str,
    body: &[u8],
) -> Result<()> {
    write_headers(stream, status, content_type, body.len())?;
    stream.write_all(body)?;
    Ok(())
}

fn write_headers(
    stream: &mut TcpStream,
    status: u16,
    content_type: &'static str,
    content_len: usize,
) -> Result<()> {
    let reason = match status {
        200 => "OK",
        404 => "Not Found",
        405 => "Method Not Allowed",
        _ => "OK",
    };
    write!(
        stream,
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {content_type}\r\nContent-Length: {content_len}\r\nConnection: close\r\nX-Content-Type-Options: nosniff\r\n\r\n"
    )?;
    Ok(())
}
