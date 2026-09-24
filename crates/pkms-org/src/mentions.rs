//! Unlinked-mention scanning.
//!
//! [`MentionMatcher`] finds prose phrases that name a note by title or alias.
//! Matching is case-insensitive, whole-word, and leftmost-longest without
//! overlaps. Org syntax that is not prose is never matched: links, URLs,
//! keyword lines, drawers, planning lines, src/example/export blocks, inline
//! code/verbatim, timestamps, and heading keywords/tags.

use crate::domain::NoteId;
use crate::parser::{HEADING_RE, LINK_RE};
use regex::Regex;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Placeholder for masked characters. It is neither a word character nor
/// whitespace, and never appears in a name, so no match can span it.
const MASK: char = '\0';

static URL_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"[A-Za-z][A-Za-z0-9+.-]*://\S+|\b(?:file|id|mailto):\S+").unwrap()
});

static VERBATIM_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?:^|[\s({'"])(?:~[^~\s](?:[^~]*?[^~\s])?~|=[^=\s](?:[^=]*?[^=\s])?=)"#).unwrap()
});

static TIMESTAMP_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"<\d{4}-\d{2}-\d{2}[^>]*>|\[\d{4}-\d{2}-\d{2}[^\]]*\]").unwrap());

static DRAWER_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^:[A-Za-z0-9_-]+:$").unwrap());

/// Block types whose content is code or literal text rather than prose.
const LITERAL_BLOCKS: &[&str] = &["src", "example", "export"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum MentionKind {
    Title,
    Alias,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MentionTarget {
    pub uuid: NoteId,
    pub kind: MentionKind,
}

/// One matched phrase. `line` and `col` are 1-based; `col` counts chars.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MentionHit<'m> {
    pub line: usize,
    pub col: usize,
    pub phrase: String,
    pub targets: &'m [MentionTarget],
}

#[derive(Debug, Default)]
pub struct MentionMatcher {
    names: HashMap<String, Vec<MentionTarget>>,
    first_chars: HashSet<char>,
    /// Distinct name lengths in chars, longest first.
    lengths: Vec<usize>,
}

impl MentionMatcher {
    /// Build a matcher from `(name, uuid, kind)` entries. Names are trimmed;
    /// empty names are ignored. When one note has the same name as title and
    /// alias, it is reported as a title match.
    pub fn new<I, S>(entries: I) -> Self
    where
        I: IntoIterator<Item = (S, NoteId, MentionKind)>,
        S: AsRef<str>,
    {
        let mut matcher = Self::default();
        let mut lengths = HashSet::new();
        for (name, uuid, kind) in entries {
            let name = name.as_ref().trim();
            let Some(first) = name.chars().next() else {
                continue;
            };
            matcher.first_chars.extend(first.to_lowercase());
            lengths.insert(name.chars().count());

            let targets = matcher.names.entry(name.to_lowercase()).or_default();
            match targets.iter_mut().find(|target| target.uuid == uuid) {
                Some(existing) if kind == MentionKind::Title => existing.kind = kind,
                Some(_) => {}
                None => targets.push(MentionTarget { uuid, kind }),
            }
        }
        matcher.lengths = lengths.into_iter().collect();
        matcher.lengths.sort_unstable_by(|a, b| b.cmp(a));
        matcher
    }

    pub fn is_empty(&self) -> bool {
        self.names.is_empty()
    }

    /// Scan org text and return non-overlapping mentions in document order.
    pub fn find_mentions(&self, content: &str) -> Vec<MentionHit<'_>> {
        let mut hits = Vec::new();
        if self.is_empty() {
            return hits;
        }
        let mut state = BlockState::default();
        for (idx, line) in content.lines().enumerate() {
            if let Some(chars) = prose_chars(line, &mut state) {
                self.scan_line(idx + 1, &chars, &mut hits);
            }
        }
        hits
    }

    fn scan_line<'m>(&'m self, line: usize, chars: &[char], hits: &mut Vec<MentionHit<'m>>) {
        let mut start = 0;
        while start < chars.len() {
            if is_candidate_start(chars, start)
                && let Some((len, targets)) = self.longest_at(chars, start)
            {
                hits.push(MentionHit {
                    line,
                    col: start + 1,
                    phrase: chars[start..start + len].iter().collect(),
                    targets,
                });
                start += len;
            } else {
                start += 1;
            }
        }
    }

    fn longest_at(&self, chars: &[char], start: usize) -> Option<(usize, &[MentionTarget])> {
        if !chars[start]
            .to_lowercase()
            .all(|c| self.first_chars.contains(&c))
        {
            return None;
        }
        self.lengths.iter().find_map(|&len| {
            let end = start + len;
            if end > chars.len() || chars.get(end).copied().is_some_and(is_word_char) {
                return None;
            }
            let candidate: String = chars[start..end].iter().collect::<String>().to_lowercase();
            self.names
                .get(&candidate)
                .map(|targets| (len, targets.as_slice()))
        })
    }
}

/// Return the `id:` link targets present in org text.
pub fn linked_ids(content: &str) -> HashSet<String> {
    LINK_RE
        .captures_iter(content)
        .filter_map(|cap| cap[1].trim().strip_prefix("id:").map(str::to_string))
        .collect()
}

#[derive(Debug, Default)]
struct BlockState {
    /// Lowercased `#+end_<kind>` marker of the open literal block.
    literal_block_end: Option<String>,
    in_drawer: bool,
}

/// Return the line as chars with non-prose regions replaced by [`MASK`], or
/// `None` when the whole line is not prose.
fn prose_chars(line: &str, state: &mut BlockState) -> Option<Vec<char>> {
    let trimmed = line.trim();
    let lower = trimmed.to_ascii_lowercase();

    if let Some(end) = &state.literal_block_end {
        if lower.starts_with(end.as_str()) {
            state.literal_block_end = None;
        }
        return None;
    }
    if state.in_drawer {
        if lower == ":end:" {
            state.in_drawer = false;
        }
        return None;
    }
    if let Some(rest) = lower.strip_prefix("#+begin_") {
        let kind = rest.split_whitespace().next().unwrap_or_default();
        if LITERAL_BLOCKS.contains(&kind) {
            state.literal_block_end = Some(format!("#+end_{kind}"));
        }
        return None;
    }
    if lower.starts_with("#+") || lower == "#" || lower.starts_with("# ") {
        return None;
    }
    if DRAWER_RE.is_match(trimmed) && lower != ":end:" {
        state.in_drawer = true;
        return None;
    }
    if ["scheduled:", "deadline:", "closed:"]
        .iter()
        .any(|keyword| lower.starts_with(keyword))
    {
        return None;
    }

    let mut masked = vec![false; line.len()];
    let mut mask = |range: std::ops::Range<usize>| masked[range].fill(true);
    if let Some(cap) = HEADING_RE.captures(line) {
        match cap.get(4) {
            Some(title) => {
                mask(0..title.start());
                mask(title.end()..line.len());
            }
            None => mask(0..line.len()),
        }
    }
    for re in [&*LINK_RE, &*URL_RE, &*VERBATIM_RE, &*TIMESTAMP_RE] {
        for found in re.find_iter(line) {
            mask(found.range());
        }
    }

    Some(
        line.char_indices()
            .map(|(idx, c)| if masked[idx] { MASK } else { c })
            .collect(),
    )
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

fn is_candidate_start(chars: &[char], idx: usize) -> bool {
    let c = chars[idx];
    c != MASK && !c.is_whitespace() && (idx == 0 || !is_word_char(chars[idx - 1]))
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: &str = "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa";
    const B: &str = "bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb";

    fn matcher(names: &[(&str, &str, MentionKind)]) -> MentionMatcher {
        MentionMatcher::new(
            names
                .iter()
                .map(|(name, uuid, kind)| (*name, NoteId::new(*uuid), *kind)),
        )
    }

    fn phrases(matcher: &MentionMatcher, content: &str) -> Vec<(usize, usize, String)> {
        matcher
            .find_mentions(content)
            .into_iter()
            .map(|hit| (hit.line, hit.col, hit.phrase))
            .collect()
    }

    #[test]
    fn matches_case_insensitively_on_whole_words() {
        let m = matcher(&[("Rust", A, MentionKind::Title)]);

        assert_eq!(
            phrases(&m, "I like rust, and RUST.\nNot rusty or trust."),
            vec![(1, 8, "rust".to_string()), (1, 18, "RUST".to_string())]
        );
    }

    #[test]
    fn matches_names_with_punctuation_edges() {
        let m = matcher(&[
            ("C++", A, MentionKind::Title),
            (".NET", B, MentionKind::Title),
        ]);

        assert_eq!(
            phrases(&m, "Use C++ or .NET, not ASP.NET or C++x."),
            vec![(1, 5, "C++".to_string()), (1, 12, ".NET".to_string())]
        );
    }

    #[test]
    fn folds_unicode_case_and_counts_columns_in_chars() {
        let m = matcher(&[("Граф знаний", A, MentionKind::Title)]);

        assert_eq!(
            phrases(&m, "Про ГРАФ ЗНАНИЙ и графы знаний."),
            vec![(1, 5, "ГРАФ ЗНАНИЙ".to_string())]
        );
    }

    #[test]
    fn prefers_longest_name_without_overlaps() {
        let m = matcher(&[
            ("Learning", A, MentionKind::Title),
            ("Machine Learning", B, MentionKind::Title),
        ]);

        let hits = m.find_mentions("machine learning and learning");

        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].phrase, "machine learning");
        assert_eq!(hits[0].targets[0].uuid, B);
        assert_eq!(hits[1].col, 22);
        assert_eq!(hits[1].targets[0].uuid, A);
    }

    #[test]
    fn falls_back_to_shorter_name_when_longer_fails_word_boundary() {
        let m = matcher(&[
            ("Rust", A, MentionKind::Title),
            ("Rust lang", B, MentionKind::Title),
        ]);

        let hits = m.find_mentions("Rust language");

        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].phrase, "Rust");
        assert_eq!(hits[0].targets[0].uuid, A);
    }

    #[test]
    fn skips_links_urls_code_and_timestamps() {
        let m = matcher(&[("Rust", A, MentionKind::Title)]);
        let content = "[[id:x][Rust]] [[https://rust.org]] https://example.com/Rust \
                       file:Rust.org ~Rust~ =Rust= <2026-01-01 Rust> [2026-01-01 Rust]";

        assert!(phrases(&m, content).is_empty());
    }

    #[test]
    fn skips_keyword_lines_drawers_planning_and_literal_blocks() {
        let m = matcher(&[("Rust", A, MentionKind::Title)]);
        let content = "\
:PROPERTIES:
:ID: Rust
:END:
#+title: Rust
#+filetags: :Rust:
# Rust comment
SCHEDULED: <2026-01-01> Rust
#+begin_src rust
fn rust() {} // Rust
#+end_src
#+BEGIN_EXAMPLE
Rust
#+END_EXAMPLE
#+begin_quote
Rust in a quote
#+end_quote
";

        assert_eq!(phrases(&m, content), vec![(15, 1, "Rust".to_string())]);
    }

    #[test]
    fn matches_heading_title_but_not_keyword_or_tags() {
        let m = matcher(&[
            ("Rust", A, MentionKind::Title),
            ("TODO", B, MentionKind::Title),
        ]);

        assert_eq!(
            phrases(&m, "** TODO [#A] Learn Rust :rust:todo:"),
            vec![(1, 20, "Rust".to_string())]
        );
    }

    #[test]
    fn reports_every_note_sharing_a_name_and_prefers_title_kind() {
        let m = matcher(&[
            ("Graph", A, MentionKind::Alias),
            ("graph", A, MentionKind::Title),
            ("Graph", B, MentionKind::Alias),
        ]);

        let hits = m.find_mentions("a graph");

        assert_eq!(
            hits[0].targets,
            &[
                MentionTarget {
                    uuid: NoteId::new(A),
                    kind: MentionKind::Title
                },
                MentionTarget {
                    uuid: NoteId::new(B),
                    kind: MentionKind::Alias
                },
            ]
        );
    }

    #[test]
    fn collects_id_link_targets() {
        let ids = linked_ids(&format!("[[id:{A}][a]] [[id:{B}]] [[https://x.org][x]]"));

        assert_eq!(ids, HashSet::from([A.to_string(), B.to_string()]));
    }
}
