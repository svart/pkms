use super::{ApiError, AppState};
use anyhow::Result;
use axum::{
    http::{HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoteViewerMethod {
    Get,
    Head,
    Post,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoteViewerRequest {
    pub method: NoteViewerMethod,
    pub path: String,
    pub query: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoteViewerResponse {
    pub status: u16,
    pub content_type: String,
    pub body: Vec<u8>,
}

pub trait NoteViewer: Send + Sync {
    fn respond(&self, request: NoteViewerRequest) -> Result<NoteViewerResponse>;
}

pub(super) fn note_viewer_response(
    state: &AppState,
    method: NoteViewerMethod,
    path: impl Into<String>,
    query: Option<String>,
) -> Result<Response, ApiError> {
    let Some(note_viewer) = &state.note_viewer else {
        return Err(ApiError::not_found("Note viewer unavailable"));
    };
    let viewer_response = note_viewer
        .respond(NoteViewerRequest {
            method,
            path: path.into(),
            query,
        })
        .map_err(|err| ApiError::internal("note_viewer", &err))?;
    let status = StatusCode::from_u16(viewer_response.status)
        .map_err(|err| ApiError::internal("note_viewer_status", &anyhow::Error::new(err)))?;
    let content_type = HeaderValue::from_str(&viewer_response.content_type)
        .map_err(|err| ApiError::internal("note_viewer_content_type", &anyhow::Error::new(err)))?;
    let mut response = (status, viewer_response.body).into_response();
    response
        .headers_mut()
        .insert(header::CONTENT_TYPE, content_type);
    Ok(response)
}
