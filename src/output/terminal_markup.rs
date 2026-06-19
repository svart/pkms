use std::io::IsTerminal;

const RESET: &str = "\x1b[0m";
const CODE_PREFIX: &str = "\x1b[2m";
const ORANGE_PREFIX: &str = "\x1b[38;5;166m";
const MENTION_PREFIX: &str = "\x1b[38;5;39m";

pub fn format_if_terminal_supported(text: &str) -> String {
    if terminal_markup_enabled() {
        format_terminal_markup(text)
    } else {
        text.to_string()
    }
}

pub fn terminal_markup_enabled() -> bool {
    terminal_markup_enabled_for_stream(std::io::stdout().is_terminal())
}

pub fn format_stderr_warning(text: &str) -> String {
    if terminal_markup_enabled_for_stream(std::io::stderr().is_terminal()) {
        format_orange(text)
    } else {
        text.to_string()
    }
}

fn terminal_markup_enabled_for_stream(stream_is_terminal: bool) -> bool {
    let term = std::env::var("TERM").ok();
    let clicolor = std::env::var("CLICOLOR").ok();
    let clicolor_force = std::env::var("CLICOLOR_FORCE").ok();
    terminal_markup_enabled_from(
        stream_is_terminal,
        term.as_deref(),
        std::env::var_os("NO_COLOR").is_some(),
        clicolor.as_deref(),
        clicolor_force.as_deref(),
    )
}

fn terminal_markup_enabled_from(
    stream_is_terminal: bool,
    term: Option<&str>,
    no_color: bool,
    clicolor: Option<&str>,
    clicolor_force: Option<&str>,
) -> bool {
    if env_flag_enabled(clicolor_force) {
        return true;
    }
    if no_color || clicolor == Some("0") {
        return false;
    }
    stream_is_terminal && term != Some("dumb")
}

fn env_flag_enabled(value: Option<&str>) -> bool {
    value.is_some_and(|value| !value.is_empty() && value != "0")
}

fn format_orange(text: &str) -> String {
    format!("{ORANGE_PREFIX}{text}{RESET}")
}

pub fn format_terminal_markup(text: &str) -> String {
    let Some((start, end, style)) = find_code_markup(text) else {
        return format_mentions(text);
    };

    let mut formatted = String::new();
    formatted.push_str(&format_mentions(&text[..start]));

    let delimiter_len = style.delimiter().len_utf8();
    let inner = &text[start + delimiter_len..end];
    formatted.push_str(style.prefix());
    formatted.push_str(inner);
    formatted.push_str(RESET);
    formatted.push_str(&format_terminal_markup(&text[end + delimiter_len..]));
    formatted
}

fn format_mentions(text: &str) -> String {
    let mut formatted = String::new();
    let mut last = 0;
    for (start, _) in text.match_indices('@') {
        if !is_valid_mention_start(text, start) {
            continue;
        }
        let end = mention_end(text, start + '@'.len_utf8());
        if end == start + '@'.len_utf8() {
            continue;
        }
        formatted.push_str(&text[last..start]);
        formatted.push_str(MENTION_PREFIX);
        formatted.push_str(&text[start..end]);
        formatted.push_str(RESET);
        last = end;
    }
    formatted.push_str(&text[last..]);
    formatted
}

fn is_valid_mention_start(text: &str, start: usize) -> bool {
    let before = text[..start].chars().next_back();
    let after = text[start + '@'.len_utf8()..].chars().next();
    before.is_none_or(is_mention_boundary) && after.is_some_and(is_mention_char)
}

fn mention_end(text: &str, start: usize) -> usize {
    let mut end = start;
    for (offset, c) in text[start..].char_indices() {
        if !is_mention_char(c) {
            break;
        }
        end = start + offset + c.len_utf8();
    }
    end
}

fn is_mention_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '-'
}

fn is_mention_boundary(c: char) -> bool {
    c.is_whitespace()
        || matches!(
            c,
            '(' | '[' | '{' | '<' | '\'' | '"' | ',' | ';' | ':' | '!' | '?'
        )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TerminalStyle {
    Code,
    OrangeCode,
}

impl TerminalStyle {
    fn delimiter(self) -> char {
        match self {
            TerminalStyle::Code => '=',
            TerminalStyle::OrangeCode => '~',
        }
    }

    fn prefix(self) -> &'static str {
        match self {
            TerminalStyle::Code => CODE_PREFIX,
            TerminalStyle::OrangeCode => ORANGE_PREFIX,
        }
    }

    fn from_delimiter(delimiter: char) -> Option<Self> {
        match delimiter {
            '=' => Some(TerminalStyle::Code),
            '~' => Some(TerminalStyle::OrangeCode),
            _ => None,
        }
    }
}

fn find_code_markup(text: &str) -> Option<(usize, usize, TerminalStyle)> {
    for (start, delimiter) in text.char_indices() {
        let Some(style) = TerminalStyle::from_delimiter(delimiter) else {
            continue;
        };
        if !is_valid_emphasis_start(text, start, delimiter) {
            continue;
        }
        let search_start = start + delimiter.len_utf8();
        for (offset, candidate) in text[search_start..].char_indices() {
            if candidate != delimiter {
                continue;
            }
            let end = search_start + offset;
            if is_valid_emphasis_end(text, end, delimiter) {
                return Some((start, end, style));
            }
        }
    }
    None
}

fn is_valid_emphasis_start(text: &str, start: usize, delimiter: char) -> bool {
    let before = text[..start].chars().next_back();
    let after = text[start + delimiter.len_utf8()..].chars().next();
    let starts_after_boundary = before.is_none_or(is_emphasis_boundary);
    let has_content = after.is_some_and(|c| !c.is_whitespace() && c != delimiter);
    starts_after_boundary && has_content
}

fn is_valid_emphasis_end(text: &str, end: usize, delimiter: char) -> bool {
    let before = text[..end].chars().next_back();
    let after = text[end + delimiter.len_utf8()..].chars().next();
    let ends_before_boundary = after.is_none_or(is_emphasis_boundary);
    let has_content = before.is_some_and(|c| !c.is_whitespace() && c != delimiter);
    ends_before_boundary && has_content
}

fn is_emphasis_boundary(c: char) -> bool {
    c.is_whitespace()
        || matches!(
            c,
            '(' | ')'
                | '['
                | ']'
                | '{'
                | '}'
                | '<'
                | '>'
                | '\''
                | '"'
                | ','
                | ';'
                | ':'
                | '.'
                | '!'
                | '?'
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_code_tilde_and_mentions_like_web_inline_rendering() {
        let text = "=literal @skip <tag>= ~orange @skip~ @alice @bob-dev email@example.com @";

        let formatted = format_terminal_markup(text);

        assert!(formatted.contains("\x1b[2mliteral @skip <tag>\x1b[0m"));
        assert!(formatted.contains("\x1b[38;5;166morange @skip\x1b[0m"));
        assert!(formatted.contains("\x1b[38;5;39m@alice\x1b[0m"));
        assert!(formatted.contains("\x1b[38;5;39m@bob-dev\x1b[0m"));
        assert!(formatted.contains("email@example.com @"));
        assert!(!formatted.contains("=literal"));
        assert!(!formatted.contains("~orange"));
    }

    #[test]
    fn leaves_plain_text_unchanged_when_markup_is_not_valid() {
        let text = "mid=word= unmatched ~ spaced ~ and email@example.com";

        assert_eq!(format_terminal_markup(text), text);
    }

    #[test]
    fn formats_warning_text_in_orange() {
        assert_eq!(
            format_orange("WARN: Task IDs changed"),
            "\x1b[38;5;166mWARN: Task IDs changed\x1b[0m"
        );
    }

    #[test]
    fn detects_terminal_formatting_support_from_environment() {
        assert!(terminal_markup_enabled_from(
            false,
            None,
            false,
            None,
            Some("1")
        ));
        assert!(!terminal_markup_enabled_from(
            true,
            Some("xterm-256color"),
            true,
            None,
            None
        ));
        assert!(!terminal_markup_enabled_from(
            true,
            Some("xterm-256color"),
            false,
            Some("0"),
            None
        ));
        assert!(!terminal_markup_enabled_from(
            true,
            Some("dumb"),
            false,
            None,
            None
        ));
        assert!(terminal_markup_enabled_from(
            true,
            Some("xterm-256color"),
            false,
            None,
            None
        ));
    }
}
