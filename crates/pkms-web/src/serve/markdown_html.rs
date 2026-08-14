use pulldown_cmark::{CowStr, Event, HeadingLevel, Options, Parser, Tag, TagEnd, html};

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

pub(super) fn render_markdown(content: &str) -> RenderedMarkdown {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_FOOTNOTES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_TASKLISTS);

    let mut headings = Vec::new();
    let mut current_heading: Option<MarkdownHeading> = None;
    let mut events = Vec::new();
    let mut last_heading_offset = 0;
    let mut heading_line_number = 1;

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
            Event::Start(Tag::Link {
                link_type,
                dest_url,
                title,
                id,
            }) => Event::Start(Tag::Link {
                link_type,
                dest_url: safe_destination(dest_url),
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
                dest_url: safe_destination(dest_url),
                title,
                id,
            }),
            Event::End(TagEnd::Heading(level)) => {
                if let Some(heading) = current_heading.take() {
                    headings.push(heading);
                }
                Event::End(TagEnd::Heading(level))
            }
            Event::Text(text) => {
                if let Some(heading) = current_heading.as_mut() {
                    heading.title.push_str(&text);
                }
                Event::Text(text)
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

fn safe_destination(destination: CowStr<'_>) -> CowStr<'_> {
    let trimmed = destination.trim();
    let scheme = trimmed
        .split_once(':')
        .map(|(scheme, _)| scheme.to_ascii_lowercase());
    match scheme.as_deref() {
        None | Some("http" | "https" | "mailto") => destination,
        Some(_) => CowStr::Borrowed("#"),
    }
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
}
