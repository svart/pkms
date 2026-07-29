use pkms_org::attachments::resolve_attachment_path;
use std::fmt;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ContentType {
    Html,
    PlainText,
    Css,
    Svg,
    FontTtf,
    FontWoff2,
    ImagePng,
    ImageJpeg,
    ImageGif,
    ImageWebp,
    Pdf,
    OctetStream,
}

impl ContentType {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            ContentType::Html => "text/html; charset=utf-8",
            ContentType::PlainText => "text/plain; charset=utf-8",
            ContentType::Css => "text/css; charset=utf-8",
            ContentType::Svg => "image/svg+xml",
            ContentType::FontTtf => "font/ttf",
            ContentType::FontWoff2 => "font/woff2",
            ContentType::ImagePng => "image/png",
            ContentType::ImageJpeg => "image/jpeg",
            ContentType::ImageGif => "image/gif",
            ContentType::ImageWebp => "image/webp",
            ContentType::Pdf => "application/pdf",
            ContentType::OctetStream => "application/octet-stream",
        }
    }
}

impl fmt::Display for ContentType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum AssetKind {
    File,
    Attachment,
}

impl AssetKind {
    pub(super) fn parse(value: &str) -> Option<Self> {
        match value {
            "file" => Some(AssetKind::File),
            "attachment" => Some(AssetKind::Attachment),
            _ => None,
        }
    }

    pub(super) fn as_str(self) -> &'static str {
        match self {
            AssetKind::File => "file",
            AssetKind::Attachment => "attachment",
        }
    }
}

struct ServedFont {
    name: &'static str,
    bytes: &'static [u8],
    content_type: ContentType,
}

static SERVED_FONTS: &[ServedFont] = &[
    ServedFont {
        name: "Commissioner.ttf",
        bytes: include_bytes!("../serve_fonts/Commissioner.ttf"),
        content_type: ContentType::FontTtf,
    },
    ServedFont {
        name: "Outfit.ttf",
        bytes: include_bytes!("../serve_fonts/Outfit.ttf"),
        content_type: ContentType::FontTtf,
    },
    ServedFont {
        name: "Iosevka-Regular.woff2",
        bytes: include_bytes!("../serve_fonts/Iosevka-Regular.woff2"),
        content_type: ContentType::FontWoff2,
    },
    ServedFont {
        name: "Iosevka-Bold.woff2",
        bytes: include_bytes!("../serve_fonts/Iosevka-Bold.woff2"),
        content_type: ContentType::FontWoff2,
    },
    ServedFont {
        name: "SymbolsNerdFontMono-Regular.woff2",
        bytes: include_bytes!("../serve_fonts/SymbolsNerdFontMono-Regular.woff2"),
        content_type: ContentType::FontWoff2,
    },
];

pub(crate) fn favicon_response() -> super::HttpResponse {
    super::HttpResponse {
        status: 200,
        content_type: ContentType::Svg,
        body: include_str!("favicon.svg").as_bytes().to_vec(),
    }
}

pub(crate) fn font_response(name: &str) -> super::HttpResponse {
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
    let bucketed = resolve_attachment_path(db_root, uuid, target);
    if bucketed.exists() {
        bucketed
    } else {
        db_root.join(".attach").join(uuid).join(target)
    }
}

pub(super) fn is_db_asset_allowed(path: &Path, db_root: &Path) -> bool {
    let Ok(canonical_path) = std::fs::canonicalize(path) else {
        return false;
    };
    std::fs::canonicalize(db_root)
        .is_ok_and(|canonical_root| canonical_path.starts_with(canonical_root))
}

pub(super) fn is_attachment_asset_allowed(path: &Path, db_root: &Path, uuid: &str) -> bool {
    let Ok(canonical_path) = std::fs::canonicalize(path) else {
        return false;
    };
    attachment_roots(db_root, uuid)
        .into_iter()
        .filter_map(|root| std::fs::canonicalize(root).ok())
        .any(|root| canonical_path.starts_with(root))
}

fn attachment_roots(db_root: &Path, uuid: &str) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if uuid.len() > 2 {
        roots.push(db_root.join(".attach").join(&uuid[..2]).join(&uuid[2..]));
    }
    roots.push(db_root.join(".attach").join(uuid));
    roots
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

pub(super) fn mime_type(path: &Path) -> ContentType {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "png" => ContentType::ImagePng,
        "jpg" | "jpeg" => ContentType::ImageJpeg,
        "gif" => ContentType::ImageGif,
        "webp" => ContentType::ImageWebp,
        "svg" => ContentType::Svg,
        "pdf" => ContentType::Pdf,
        "txt" => ContentType::PlainText,
        "html" => ContentType::Html,
        "css" => ContentType::Css,
        _ => ContentType::OctetStream,
    }
}
