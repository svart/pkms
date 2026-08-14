//! Internal HTTP viewer implementation modules.

pub(super) mod assets;
pub(super) mod highlight;
pub(super) mod http;
pub(super) mod inline;
pub(super) mod markdown_html;
pub(super) mod org_html;
pub(super) mod page;

pub(super) use http::HttpResponse;
pub(super) use page::{
    render_markdown_html, render_note_html, render_preview_html, render_standalone_org_html,
};

pub(super) fn page_css() -> String {
    let mut css = include_str!("../serve.css").to_string();
    css.push('\n');
    css.push_str(&highlight::syntect_css());
    css
}

pub(super) fn page_js() -> &'static str {
    include_str!("page.js")
}
