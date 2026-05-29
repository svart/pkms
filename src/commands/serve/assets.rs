use crate::util;
use std::path::{Path, PathBuf};

struct ServedFont {
    name: &'static str,
    bytes: &'static [u8],
    content_type: &'static str,
}

static SERVED_FONTS: &[ServedFont] = &[
    ServedFont {
        name: "Alegreya.ttf",
        bytes: include_bytes!("../serve_fonts/Alegreya.ttf"),
        content_type: "font/ttf",
    },
    ServedFont {
        name: "Alegreya-Italic.ttf",
        bytes: include_bytes!("../serve_fonts/Alegreya-Italic.ttf"),
        content_type: "font/ttf",
    },
    ServedFont {
        name: "AlegreyaSans-Regular.ttf",
        bytes: include_bytes!("../serve_fonts/AlegreyaSans-Regular.ttf"),
        content_type: "font/ttf",
    },
    ServedFont {
        name: "AlegreyaSans-Italic.ttf",
        bytes: include_bytes!("../serve_fonts/AlegreyaSans-Italic.ttf"),
        content_type: "font/ttf",
    },
    ServedFont {
        name: "AlegreyaSans-Bold.ttf",
        bytes: include_bytes!("../serve_fonts/AlegreyaSans-Bold.ttf"),
        content_type: "font/ttf",
    },
    ServedFont {
        name: "AlegreyaSans-BoldItalic.ttf",
        bytes: include_bytes!("../serve_fonts/AlegreyaSans-BoldItalic.ttf"),
        content_type: "font/ttf",
    },
    ServedFont {
        name: "FiraCode.ttf",
        bytes: include_bytes!("../serve_fonts/FiraCode.ttf"),
        content_type: "font/ttf",
    },
];

pub(super) fn favicon_response() -> super::HttpResponse {
    super::HttpResponse {
        status: 200,
        content_type: "image/svg+xml",
        body: include_str!("favicon.svg").as_bytes().to_vec(),
    }
}

pub(super) fn font_response(name: &str) -> super::HttpResponse {
    SERVED_FONTS
        .iter()
        .find(|font| font.name == name)
        .map(|font| super::HttpResponse {
            status: 200,
            content_type: font.content_type,
            body: font.bytes.to_vec(),
        })
        .unwrap_or_else(|| super::HttpResponse::not_found("Font not found"))
}

pub(super) fn resolve_existing_attachment(db_root: &Path, uuid: &str, target: &str) -> PathBuf {
    let bucketed = util::resolve_attachment_path(db_root, uuid, target);
    if bucketed.exists() {
        bucketed
    } else {
        db_root.join(".attach").join(uuid).join(target)
    }
}

pub(super) fn is_asset_allowed(path: &Path, db_root: &Path) -> bool {
    let Ok(canonical_path) = std::fs::canonicalize(path) else {
        return false;
    };
    if let Ok(canonical_root) = std::fs::canonicalize(db_root)
        && canonical_path.starts_with(canonical_root)
    {
        return true;
    }
    dirs::home_dir().is_some_and(|home| canonical_path.starts_with(home))
}

pub(super) fn is_image_path(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|ext| ext.to_str())
            .map(str::to_ascii_lowercase)
            .as_deref(),
        Some("png" | "jpg" | "jpeg" | "gif" | "webp" | "svg")
    )
}

pub(super) fn mime_type(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "pdf" => "application/pdf",
        "txt" => "text/plain; charset=utf-8",
        "html" => "text/html; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        _ => "application/octet-stream",
    }
}
