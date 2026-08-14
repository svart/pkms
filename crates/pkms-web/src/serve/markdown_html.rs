use super::highlight::highlight_code;
use super::inline::percent_encode;
use pulldown_cmark::{
    CodeBlockKind, CowStr, Event, HeadingLevel, Options, Parser, Tag, TagEnd, html,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct MarkdownHeading {
    pub(super) level: usize,
    pub(super) line_number: usize,
    pub(super) title: String,
}

pub(super) struct RenderedMarkdown {
    pub(super) body: String,
    pub(super) headings: Vec<MarkdownHeading>,
}

pub(super) fn render_markdown(content: &str, request_path: &str) -> RenderedMarkdown {
    let options = markdown_options();

    let mut headings = Vec::new();
    let mut current_heading: Option<MarkdownHeading> = None;
    let mut events = Vec::new();
    let mut last_heading_offset = 0;
    let mut heading_line_number = 1;
    let mut fenced_language = None;

    for (event, range) in Parser::new_ext(content, options).into_offset_iter() {
        let event = match event {
            Event::Start(Tag::Heading {
                level,
                classes,
                attrs,
                ..
            }) => {
                heading_line_number += content.as_bytes()
                    [last_heading_offset..range.start.min(content.len())]
                    .iter()
                    .filter(|byte| **byte == b'\n')
                    .count();
                last_heading_offset = range.start;
                current_heading = Some(MarkdownHeading {
                    level: heading_level(level),
                    line_number: heading_line_number,
                    title: String::new(),
                });
                Event::Start(Tag::Heading {
                    level,
                    id: Some(CowStr::from(format!("h-{heading_line_number}"))),
                    classes,
                    attrs,
                })
            }
            Event::Start(Tag::CodeBlock(kind)) => {
                fenced_language = match &kind {
                    CodeBlockKind::Fenced(info) => info
                        .split_whitespace()
                        .next()
                        .filter(|language| !language.is_empty())
                        .map(str::to_string),
                    CodeBlockKind::Indented => None,
                };
                Event::Start(Tag::CodeBlock(kind))
            }
            Event::Start(Tag::Link {
                link_type,
                dest_url,
                title,
                id,
            }) => Event::Start(Tag::Link {
                link_type,
                dest_url: safe_destination(dest_url, request_path),
                title,
                id,
            }),
            Event::Start(Tag::Image {
                link_type,
                dest_url,
                title,
                id,
            }) => Event::Start(Tag::Image {
                link_type,
                dest_url: safe_destination(dest_url, request_path),
                title,
                id,
            }),
            Event::End(TagEnd::Heading(level)) => {
                if let Some(heading) = current_heading.take() {
                    headings.push(heading);
                }
                Event::End(TagEnd::Heading(level))
            }
            Event::End(TagEnd::CodeBlock) => {
                fenced_language = None;
                Event::End(TagEnd::CodeBlock)
            }
            Event::Text(text) => {
                if let Some(language) = fenced_language.as_deref() {
                    Event::Html(CowStr::from(highlight_code(language, &text)))
                } else {
                    if let Some(heading) = current_heading.as_mut() {
                        heading.title.push_str(&text);
                    }
                    Event::Text(text)
                }
            }
            Event::Code(code) => {
                if let Some(heading) = current_heading.as_mut() {
                    heading.title.push_str(&code);
                }
                Event::Code(code)
            }
            Event::Html(raw) | Event::InlineHtml(raw) => Event::Text(raw),
            event => event,
        };
        events.push(event);
    }

    let mut body = String::new();
    html::push_html(&mut body, events.into_iter());
    RenderedMarkdown { body, headings }
}

fn safe_destination<'a>(destination: CowStr<'a>, request_path: &str) -> CowStr<'a> {
    let trimmed = destination.trim();
    if is_document_destination(trimmed) {
        return destination;
    }
    let scheme = trimmed
        .split_once(':')
        .map(|(scheme, _)| scheme.to_ascii_lowercase());
    match scheme.as_deref() {
        None => CowStr::from(format!(
            "/markdown-asset?file={}&target={}",
            percent_encode(request_path),
            percent_encode(trimmed)
        )),
        Some("http" | "https" | "mailto") => destination,
        Some(_) => CowStr::Borrowed("#"),
    }
}

pub(super) fn declares_local_asset(content: &str, target: &str) -> bool {
    Parser::new_ext(content, markdown_options()).any(|event| match event {
        Event::Start(Tag::Link { dest_url, .. } | Tag::Image { dest_url, .. }) => {
            dest_url.trim() == target && is_local_asset_destination(dest_url.trim())
        }
        _ => false,
    })
}

fn markdown_options() -> Options {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_FOOTNOTES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_TASKLISTS);
    options
}

fn is_local_asset_destination(destination: &str) -> bool {
    !is_document_destination(destination) && !destination.contains(':')
}

fn is_document_destination(destination: &str) -> bool {
    destination.starts_with('#') || destination.starts_with('?') || destination.starts_with("//")
}

fn heading_level(level: HeadingLevel) -> usize {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_anchored_headings_and_escapes_raw_html() {
        let rendered = render_markdown(
            "# Title\n\n## Details\n\n<script>bad()</script>\n\n[bad](javascript:alert(1))\n",
            "/tmp/guide.md",
        );

        assert!(rendered.body.contains("<h1 id=\"h-1\">Title</h1>"));
        assert!(rendered.body.contains("<h2 id=\"h-3\">Details</h2>"));
        assert!(rendered.body.contains("&lt;script&gt;bad()&lt;/script&gt;"));
        assert!(rendered.body.contains("<a href=\"#\">bad</a>"));
        assert!(!rendered.body.contains("javascript:"));
        assert_eq!(
            rendered.headings,
            [
                MarkdownHeading {
                    level: 1,
                    line_number: 1,
                    title: "Title".to_string(),
                },
                MarkdownHeading {
                    level: 2,
                    line_number: 3,
                    title: "Details".to_string(),
                },
            ]
        );
    }

    #[test]
    fn rewrites_local_assets_and_preserves_document_and_web_destinations() {
        let rendered = render_markdown(
            "![Relative](<images/pic one.png>)\n\n[Absolute](/tmp/report.pdf)\n\n[Section](#part)\n\n[Web](https://example.com)\n",
            "/tmp/guide.md",
        );

        assert!(
            rendered
                .body
                .contains("src=\"/markdown-asset?file=%2Ftmp%2Fguide.md&amp;target=images%2Fpic%20one.png\" alt=\"Relative\""),
            "{}",
            rendered.body
        );
        assert!(rendered.body.contains(
            "href=\"/markdown-asset?file=%2Ftmp%2Fguide.md&amp;target=%2Ftmp%2Freport.pdf\""
        ));
        assert!(rendered.body.contains("href=\"#part\""));
        assert!(rendered.body.contains("href=\"https://example.com\""));
    }

    #[test]
    fn recognizes_only_declared_local_assets() {
        let content = "![Image](pic.png)\n\n[Web](https://example.com)\n";

        assert!(declares_local_asset(content, "pic.png"));
        assert!(!declares_local_asset(content, "missing.png"));
        assert!(!declares_local_asset(content, "https://example.com"));
    }

    #[test]
    fn highlights_fenced_cpp_source_code() {
        let rendered = render_markdown(
            "```c++\n#include <vector>\nclass ThisIsClass {\npublic:\n    int value;\n};\n```\n",
            "/tmp/guide.md",
        );

        assert!(rendered.body.contains("<code class=\"language-c++\">"));
        assert!(rendered.body.contains("<span class=\"syn-"));
        assert!(rendered.body.contains("ThisIsClass"));
        assert!(rendered.body.contains("&lt;"));
        assert!(rendered.body.contains("&gt;"));
        assert!(!rendered.body.contains("<vector>"));
    }
}
