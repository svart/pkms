use super::super::highlight::highlight_code;
use super::super::inline::{escape_html, render_display_math, render_formatted_text};
use super::OrgRenderContext;

pub(super) struct OrgBlock<'a> {
    kind: OrgBlockKind,
    args: &'a str,
    body: Vec<&'a str>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum OrgBlockKind {
    Src,
    Example,
    Quote,
    Verse,
    Center,
    Comment,
    Export,
    Other(String),
}

impl OrgBlockKind {
    fn parse(value: &str) -> Self {
        match value {
            "src" => Self::Src,
            "example" => Self::Example,
            "quote" => Self::Quote,
            "verse" => Self::Verse,
            "center" => Self::Center,
            "comment" => Self::Comment,
            "export" => Self::Export,
            other => Self::Other(other.to_string()),
        }
    }
}

pub(super) fn read_org_block<'a>(lines: &[&'a str], start: usize) -> Option<(OrgBlock<'a>, usize)> {
    let trimmed = lines.get(start)?.trim();
    let lower = trimmed.to_ascii_lowercase();
    let rest = lower.strip_prefix("#+begin_")?;
    let kind_end = rest.find(char::is_whitespace).unwrap_or(rest.len());
    let kind_name = &rest[..kind_end];
    let kind = OrgBlockKind::parse(kind_name);
    let original_rest = trimmed.get("#+begin_".len()..)?;
    let args = original_rest
        .get(kind_end..)
        .map(str::trim)
        .unwrap_or_default();
    let end_marker = format!("#+end_{kind_name}");
    let mut body = Vec::new();
    let mut i = start + 1;
    while i < lines.len() && !lines[i].trim().eq_ignore_ascii_case(&end_marker) {
        body.push(lines[i]);
        i += 1;
    }
    if i < lines.len() {
        i += 1;
    }
    Some((OrgBlock { kind, args, body }, i))
}

pub(super) fn render_org_block(
    context: &OrgRenderContext<'_>,
    block: &OrgBlock<'_>,
    caption: Option<&str>,
) -> String {
    match &block.kind {
        OrgBlockKind::Src => render_src_block(block, caption),
        OrgBlockKind::Example => {
            render_pre_block("example", "Example", &block.body_text(), caption)
        }
        OrgBlockKind::Quote => render_text_block(context, "quote", "Quote", &block.body, caption),
        OrgBlockKind::Verse => render_pre_block("verse", "Verse", &block.body_text(), caption),
        OrgBlockKind::Center => {
            render_text_block(context, "center", "Center", &block.body, caption)
        }
        OrgBlockKind::Comment => {
            render_text_block(context, "comment", "Comment", &block.body, caption)
        }
        OrgBlockKind::Export => render_export_block(block, caption),
        OrgBlockKind::Other(kind) => {
            let label = format!("Block: {kind}");
            render_pre_block("special", &label, &block.body_text(), caption)
        }
    }
}

impl OrgBlock<'_> {
    pub(super) fn is_src(&self) -> bool {
        matches!(self.kind, OrgBlockKind::Src)
    }

    pub(super) fn is_example(&self) -> bool {
        matches!(self.kind, OrgBlockKind::Example)
    }

    pub(super) fn is_results_wrapper(&self) -> bool {
        matches!(&self.kind, OrgBlockKind::Other(kind) if kind == "results")
    }

    pub(super) fn body(&self) -> &[&str] {
        &self.body
    }

    fn body_text(&self) -> String {
        let mut text = self.body.join("\n");
        if !text.is_empty() {
            text.push('\n');
        }
        text
    }
}

fn render_src_block(block: &OrgBlock<'_>, caption: Option<&str>) -> String {
    let lang = block.args.split_whitespace().next().unwrap_or_default();
    let label = if lang.is_empty() {
        "Source".to_string()
    } else {
        format!("Source: {lang}")
    };
    let code = block.body_text();
    let rendered_code = if lang.is_empty() {
        escape_html(&code)
    } else {
        highlight_code(lang, &code)
    };
    let mut html = format!(
        "<figure class=\"org-block org-block-src code\"><figcaption>{}</figcaption><pre><code class=\"syn-code\">{}</code></pre>",
        escape_html(&label),
        rendered_code
    );
    push_block_caption(&mut html, caption);
    html.push_str("</figure>\n");
    html
}

fn render_export_block(block: &OrgBlock<'_>, caption: Option<&str>) -> String {
    let backend = block.args.split_whitespace().next().unwrap_or_default();
    let text = block.body_text();
    if backend.eq_ignore_ascii_case("latex") {
        let mut html = format!(
            "<figure class=\"org-block org-block-export org-block-export-latex\"><figcaption>Export: latex</figcaption>{}",
            render_display_math(text.trim())
        );
        push_block_caption(&mut html, caption);
        html.push_str("</figure>\n");
        return html;
    }
    let label = if backend.is_empty() {
        "Export".to_string()
    } else {
        format!("Export: {backend}")
    };
    render_pre_block("export", &label, &text, caption)
}

fn render_pre_block(kind: &str, label: &str, text: &str, caption: Option<&str>) -> String {
    let mut html = format!(
        "<figure class=\"org-block org-block-{kind}\"><figcaption>{}</figcaption><pre>{}</pre>",
        escape_html(label),
        escape_html(text)
    );
    push_block_caption(&mut html, caption);
    html.push_str("</figure>\n");
    html
}

fn render_text_block(
    context: &OrgRenderContext<'_>,
    kind: &str,
    label: &str,
    lines: &[&str],
    caption: Option<&str>,
) -> String {
    let text = lines.join("\n");
    let mut html = format!(
        "<figure class=\"org-block org-block-{kind}\"><figcaption>{}</figcaption><div class=\"org-block-content\">{}</div>",
        escape_html(label),
        context.render_inline(&text)
    );
    push_block_caption(&mut html, caption);
    html.push_str("</figure>\n");
    html
}

fn push_block_caption(html: &mut String, caption: Option<&str>) {
    if let Some(caption) = caption {
        html.push_str("<div class=\"org-block-caption\">");
        html.push_str(&render_formatted_text(caption));
        html.push_str("</div>");
    }
}

pub(super) fn render_table(lines: &[&str], context: &OrgRenderContext<'_>) -> String {
    let mut html = String::from("<table>\n<tbody>\n");
    let mut is_header = true;
    for line in lines {
        let cells: Vec<&str> = line.trim_matches('|').split('|').map(str::trim).collect();
        if cells
            .iter()
            .all(|cell| !cell.is_empty() && cell.chars().all(|c| c == '-' || c == '+'))
        {
            continue;
        }
        html.push_str("<tr>");
        for cell in cells {
            if is_header {
                html.push_str("<th scope=\"col\">");
            } else {
                html.push_str("<td>");
            }
            html.push_str(&context.render_inline(cell));
            if is_header {
                html.push_str("</th>");
            } else {
                html.push_str("</td>");
            }
        }
        html.push_str("</tr>\n");
        is_header = false;
    }
    html.push_str("</tbody>\n</table>\n");
    html
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_org_block_kind_as_domain_value() {
        let (block, next) =
            read_org_block(&["#+begin_src rust", "fn main() {}", "#+end_src"], 0).unwrap();
        assert!(matches!(block.kind, OrgBlockKind::Src));
        assert_eq!(block.args, "rust");
        assert_eq!(next, 3);

        let (block, _) = read_org_block(&["#+begin_unknown", "body", "#+end_unknown"], 0).unwrap();
        assert!(matches!(block.kind, OrgBlockKind::Other(ref kind) if kind == "unknown"));
    }
}
