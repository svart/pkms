use super::blocks::{OrgBlock, read_org_block, render_org_block, render_table};
use super::lists::{ListFrame, append_list_continuation, close_lists, list_item, render_list_item};
use super::{OrgRenderContext, escape_html};
use pkms_org::parser::HEADING_RE;

pub(super) fn result_after(lines: &[&str], start: usize) -> Option<usize> {
    let mut i = start;
    while lines.get(i).is_some_and(|line| line.trim().is_empty()) {
        i += 1;
    }
    result_label(lines.get(i)?).map(|_| i)
}

pub(super) fn render_org_result(
    context: &OrgRenderContext<'_>,
    lines: &[&str],
    start: usize,
) -> Option<(String, usize)> {
    let label = result_label(lines.get(start)?)?;
    let (body, next_i) = render_result_body(context, lines, start + 1);
    let mut html = String::from("<figure class=\"org-block org-block-result\"><figcaption>Result");
    if let Some(label) = label {
        html.push_str(": ");
        html.push_str(&escape_html(label));
    }
    html.push_str("</figcaption>");
    html.push_str(&body);
    html.push_str("</figure>\n");
    Some((html, next_i))
}

fn result_label(line: &str) -> Option<Option<&str>> {
    let trimmed = line.trim();
    let colon = trimmed.find(':')?;
    let keyword = &trimmed[..colon];
    let (prefix, suffix) = keyword.split_at_checked("#+results".len())?;
    if !prefix.eq_ignore_ascii_case("#+results") {
        return None;
    }
    if !suffix.is_empty() && !suffix.starts_with('[') && !suffix.starts_with('(') {
        return None;
    }
    let label = trimmed[colon + 1..].trim();
    Some((!label.is_empty()).then_some(label))
}

fn render_result_body(
    context: &OrgRenderContext<'_>,
    lines: &[&str],
    start: usize,
) -> (String, usize) {
    let Some(first) = lines.get(start) else {
        return (render_empty_result(), start);
    };
    if first.trim().is_empty() || is_result_boundary(first) {
        return (render_empty_result(), start);
    }
    if first.trim().eq_ignore_ascii_case(":results:") {
        return render_result_drawer(context, lines, start);
    }
    if let Some((block, next_i)) = read_org_block(lines, start) {
        return (render_result_block(context, &block), next_i);
    }
    if fixed_width_text(first).is_some() {
        let mut body = Vec::new();
        let mut i = start;
        while let Some(text) = lines.get(i).and_then(|line| fixed_width_text(line)) {
            body.push(text);
            i += 1;
        }
        return (render_fixed_width_result(&body), i);
    }
    if first.trim().starts_with('|') {
        let mut table = Vec::new();
        let mut i = start;
        while lines
            .get(i)
            .is_some_and(|line| line.trim().starts_with('|'))
        {
            table.push(lines[i].trim());
            i += 1;
        }
        return (render_result_table(context, &table), i);
    }
    if list_item(first).is_some() {
        let mut i = start + 1;
        while let Some(line) = lines.get(i) {
            if line.trim().is_empty()
                || (list_item(line).is_none() && line.trim_start().len() == line.len())
            {
                break;
            }
            i += 1;
        }
        return (render_result_list(context, &lines[start..i]), i);
    }

    let mut i = start + 1;
    while let Some(line) = lines.get(i) {
        if line.trim().is_empty() || is_result_boundary(line) || line.trim().starts_with("#+") {
            break;
        }
        i += 1;
    }
    (render_result_text(context, &lines[start..i]), i)
}

fn render_result_drawer(
    context: &OrgRenderContext<'_>,
    lines: &[&str],
    start: usize,
) -> (String, usize) {
    let mut end = start + 1;
    while lines
        .get(end)
        .is_some_and(|line| !line.trim().eq_ignore_ascii_case(":end:"))
    {
        end += 1;
    }
    let next_i = if end < lines.len() { end + 1 } else { end };
    (render_result_lines(context, &lines[start + 1..end]), next_i)
}

fn render_result_lines(context: &OrgRenderContext<'_>, lines: &[&str]) -> String {
    let start = lines
        .iter()
        .position(|line| !line.trim().is_empty())
        .unwrap_or(lines.len());
    let end = lines
        .iter()
        .rposition(|line| !line.trim().is_empty())
        .map_or(start, |index| index + 1);
    let lines = &lines[start..end];
    let Some(first) = lines.first() else {
        return render_empty_result();
    };
    if lines.iter().all(|line| fixed_width_text(line).is_some()) {
        let body = lines
            .iter()
            .filter_map(|line| fixed_width_text(line))
            .collect::<Vec<_>>();
        return render_fixed_width_result(&body);
    }
    if lines.iter().all(|line| line.trim().starts_with('|')) {
        let table = lines.iter().map(|line| line.trim()).collect::<Vec<_>>();
        return render_result_table(context, &table);
    }
    if list_item(first).is_some() {
        return render_result_list(context, lines);
    }
    if let Some((block, next_i)) = read_org_block(lines, 0)
        && next_i == lines.len()
    {
        return render_result_block(context, &block);
    }
    render_result_text(context, lines)
}

fn render_result_block(context: &OrgRenderContext<'_>, block: &OrgBlock<'_>) -> String {
    if block.is_results_wrapper() {
        return render_result_lines(context, block.body());
    }
    if block.is_example() {
        return render_fixed_width_result(block.body());
    }
    format!(
        "<div class=\"org-result-content\">{}</div>",
        render_org_block(context, block, None)
    )
}

fn render_fixed_width_result(lines: &[&str]) -> String {
    let mut text = lines.join("\n");
    if !text.is_empty() {
        text.push('\n');
    }
    format!("<pre><samp>{}</samp></pre>", escape_html(&text))
}

fn render_result_table(context: &OrgRenderContext<'_>, lines: &[&str]) -> String {
    format!(
        "<div class=\"org-result-content\">{}</div>",
        render_table(lines, context)
    )
}

fn render_result_list(context: &OrgRenderContext<'_>, lines: &[&str]) -> String {
    let mut body = String::from("<div class=\"org-result-content\">");
    let mut stack: Vec<ListFrame> = Vec::new();
    for line in lines {
        if let Some(item) = list_item(line) {
            render_list_item(&mut body, &mut stack, item, context);
        } else {
            let _ = append_list_continuation(&mut body, &stack, line, context);
        }
    }
    close_lists(&mut body, &mut stack);
    body.push_str("</div>");
    body
}

fn render_result_text(context: &OrgRenderContext<'_>, lines: &[&str]) -> String {
    format!(
        "<div class=\"org-result-content\"><p>{}</p></div>",
        context.render_inline(&lines.join("\n"))
    )
}

fn render_empty_result() -> String {
    String::from("<div class=\"org-result-content org-result-empty\">No output</div>")
}

fn fixed_width_text(line: &str) -> Option<&str> {
    let trimmed = line.trim_start();
    let rest = trimmed.strip_prefix(':')?;
    match rest.as_bytes().first() {
        None => Some(""),
        Some(b' ' | b'\t') => Some(&rest[1..]),
        _ => None,
    }
}

fn is_result_boundary(line: &str) -> bool {
    HEADING_RE.is_match(line) || result_label(line).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_plain_named_and_cached_result_markers() {
        assert_eq!(result_label("#+RESULTS:"), Some(None));
        assert_eq!(
            result_label("#+results: named block"),
            Some(Some("named block"))
        );
        assert_eq!(
            result_label("#+RESULTS[abc123]: cached"),
            Some(Some("cached"))
        );
        assert_eq!(result_label("#+resultset: no"), None);
    }

    #[test]
    fn removes_only_the_fixed_width_marker() {
        assert_eq!(fixed_width_text(": output"), Some("output"));
        assert_eq!(fixed_width_text(":   indented"), Some("  indented"));
        assert_eq!(fixed_width_text(":not fixed width"), None);
    }

    #[test]
    fn distinguishes_headings_from_emphasized_result_text() {
        assert!(is_result_boundary("* Heading"));
        assert!(!is_result_boundary("*emphasized*"));
    }
}
