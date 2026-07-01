use super::{assets, inline::percent_decode, render_note_html, render_preview_html};
use crate::commands::open;
use crate::config::ResolvedConfig;
use crate::domain::NoteId;
use crate::graph::{Graph, resolve_file_link_path};
use crate::parser::{Link, parse_note};
use anyhow::{Context, Result};
use std::io::{self, BufRead, BufReader, Write};
use std::net::TcpStream;
use std::path::Path;

pub(super) struct ServeState<'a> {
    pub(super) config: &'a ResolvedConfig,
    pub(super) graph: Graph,
    pub(super) initial_uuid: NoteId,
}

enum Route<'a> {
    Root,
    Preview,
    Asset,
    Favicon,
    Open,
    Font(&'a str),
    NotFound,
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
    drain_headers(&mut reader)?;
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or_default();
    let target = parts.next().unwrap_or("/");
    if method != "GET" && method != "HEAD" && method != "POST" {
        return write_response(
            &mut stream,
            405,
            assets::ContentType::PlainText,
            b"Method not allowed",
        );
    }

    let (path, query) = split_target(target);
    let route = route_for_path(path);
    let response = if method == "POST" {
        match route {
            Route::Open => open_response(state, query, open::DEFAULT_EDITOR),
            _ => Ok(HttpResponse::method_not_allowed("Method not allowed")),
        }
    } else {
        match route {
            Route::Root => render_response(state, query),
            Route::Preview => preview_response(state, query),
            Route::Asset => asset_response(state, query),
            Route::Favicon => Ok(assets::favicon_response()),
            Route::Open => Ok(HttpResponse::method_not_allowed("Method not allowed")),
            Route::Font(font_name) => Ok(assets::font_response(font_name)),
            Route::NotFound => Ok(HttpResponse::not_found("Not found")),
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

fn drain_headers(reader: &mut BufReader<TcpStream>) -> Result<()> {
    let mut line = String::new();
    loop {
        line.clear();
        let bytes_read = reader.read_line(&mut line)?;
        if bytes_read == 0 || line == "\r\n" || line == "\n" {
            break;
        }
    }
    Ok(())
}

pub(super) struct HttpResponse {
    pub(super) status: u16,
    pub(super) content_type: assets::ContentType,
    pub(super) body: Vec<u8>,
}

impl HttpResponse {
    fn html(body: String) -> Self {
        Self {
            status: 200,
            content_type: assets::ContentType::Html,
            body: body.into_bytes(),
        }
    }

    pub(super) fn not_found(message: &str) -> Self {
        Self {
            status: 404,
            content_type: assets::ContentType::PlainText,
            body: message.as_bytes().to_vec(),
        }
    }

    fn method_not_allowed(message: &str) -> Self {
        Self {
            status: 405,
            content_type: assets::ContentType::PlainText,
            body: message.as_bytes().to_vec(),
        }
    }

    fn text(message: String) -> Self {
        Self {
            status: 200,
            content_type: assets::ContentType::PlainText,
            body: message.into_bytes(),
        }
    }
}

fn render_response(state: &ServeState<'_>, query: Option<&str>) -> Result<HttpResponse> {
    let requested = query_param(query, "id").unwrap_or_else(|| state.initial_uuid.to_string());
    let node = state.graph.resolve_target(&requested)?;
    let node = if let Some(primary_uuid) = state.graph.primary_uuid_for_heading(&node.uuid) {
        state.graph.resolve_target(primary_uuid)?
    } else {
        node
    };
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
    let Some(kind) = assets::AssetKind::parse(&kind) else {
        return Ok(HttpResponse::not_found("Unknown asset kind"));
    };
    let note = state.graph.resolve_target(&note_uuid)?;
    if !note_declares_asset_link(&note.path, kind, &target)? {
        return Ok(HttpResponse::not_found("Asset not found"));
    }
    let (path, allowed) = match kind {
        assets::AssetKind::File => {
            let path = resolve_file_link_path(&target, &note.path, state.config.resolved_db_root());
            let allowed = assets::is_db_asset_allowed(&path, state.config.resolved_db_root());
            (path, allowed)
        }
        assets::AssetKind::Attachment => {
            let path = assets::resolve_existing_attachment(
                state.config.resolved_db_root(),
                &note.uuid,
                &target,
            );
            let allowed = assets::is_attachment_asset_allowed(
                &path,
                state.config.resolved_db_root(),
                &note.uuid,
            );
            (path, allowed)
        }
    };
    if !allowed || !path.is_file() {
        return Ok(HttpResponse::not_found("Asset not found"));
    }
    let body = std::fs::read(&path)?;
    Ok(HttpResponse {
        status: 200,
        content_type: assets::mime_type(&path),
        body,
    })
}

fn note_declares_asset_link(
    note_path: &Path,
    kind: assets::AssetKind,
    target: &str,
) -> Result<bool> {
    let content = std::fs::read_to_string(note_path)
        .with_context(|| format!("Failed to read {}", note_path.display()))?;
    let parsed = parse_note(&content);
    Ok(parsed
        .outgoing
        .iter()
        .chain(
            parsed
                .headings
                .iter()
                .flat_map(|heading| heading.outgoing.iter()),
        )
        .any(|link| match (kind, link) {
            (assets::AssetKind::File, Link::File(link_target)) => link_target.as_str() == target,
            (assets::AssetKind::Attachment, Link::Attachment(link_target)) => {
                link_target.as_str() == target
            }
            _ => false,
        }))
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

fn route_for_path(path: &str) -> Route<'_> {
    if let Some(font_name) = path.strip_prefix("/font/") {
        return Route::Font(font_name);
    }
    match path {
        "/" => Route::Root,
        "/preview" => Route::Preview,
        "/asset" => Route::Asset,
        "/favicon.svg" => Route::Favicon,
        "/open" => Route::Open,
        _ => Route::NotFound,
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
    content_type: assets::ContentType,
    body: &[u8],
) -> Result<()> {
    write_headers(stream, status, content_type, body.len())?;
    stream.write_all(body)?;
    Ok(())
}

fn write_headers(
    stream: &mut TcpStream,
    status: u16,
    content_type: assets::ContentType,
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
