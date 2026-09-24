use anyhow::Result;
use pkms_org::discovery;
use pkms_org::parser::{ParsedNoteSummary, parse_note_summary_with_todo_states};
use pkms_org::{OrgConfig, ScopeFilter};
use serde::Serialize;
use std::collections::HashSet;
use std::fmt::Write;
use std::path::Path;

#[derive(Debug, Clone, Serialize)]
pub struct ResolvedNote {
    pub uuid: String,
    pub title: String,
    pub path: String,
    pub filetags: Vec<String>,
    pub categories: Vec<String>,
    pub aliases: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub matched_heading_uuid: Option<String>,
    pub has_todos: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub match_kind: Option<MatchKind>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub matched_query: Option<String>,
}

/// How a `--title` query matched a note, ordered from strongest to weakest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MatchKind {
    /// The whole title equals the query.
    Exact,
    /// One alias equals the query.
    Alias,
    /// Every query word appears as a whole word in the title or one alias.
    Word,
    /// Every query word appears as a substring of the title or one alias.
    Substring,
}

/// Weakest match kind accepted for `--title` queries.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum TitleMatchMode {
    /// Exact title or alias equality.
    Exact,
    /// Exact, alias, or whole-word matches.
    Word,
    /// Any of the above plus substring matches.
    #[default]
    Substring,
}

impl TitleMatchMode {
    fn accepts(self, kind: MatchKind) -> bool {
        let weakest = match self {
            Self::Exact => MatchKind::Alias,
            Self::Word => MatchKind::Word,
            Self::Substring => MatchKind::Substring,
        };
        kind <= weakest
    }
}

/// Returns the strongest way `query` matches the note title or aliases.
pub fn title_match_kind(title: &str, aliases: &[String], query: &str) -> Option<MatchKind> {
    let query = normalize(query);
    let title = normalize(title);
    let aliases: Vec<String> = aliases.iter().map(|a| normalize(a)).collect();
    if title == query {
        return Some(MatchKind::Exact);
    }
    if aliases.contains(&query) {
        return Some(MatchKind::Alias);
    }
    let words: Vec<&str> = query.split(' ').filter(|w| !w.is_empty()).collect();
    let candidates = || std::iter::once(&title).chain(aliases.iter());
    if candidates().any(|c| words.iter().all(|w| contains_word(c, w))) {
        return Some(MatchKind::Word);
    }
    if candidates().any(|c| words.iter().all(|w| c.contains(w))) {
        return Some(MatchKind::Substring);
    }
    None
}

fn normalize(s: &str) -> String {
    s.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// True when `word` occurs in `text` without word characters on either side.
fn contains_word(text: &str, word: &str) -> bool {
    text.match_indices(word).any(|(start, m)| {
        let before = text[..start].chars().next_back();
        let after = text[start + m.len()..].chars().next();
        !before.is_some_and(is_word_char) && !after.is_some_and(is_word_char)
    })
}

#[derive(Serialize)]
pub struct ResolveOutput {
    pub query: String,
    /// Title queries in request order; used to group text output.
    #[serde(skip)]
    pub titles: Vec<String>,
    pub total: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub showed: Option<usize>,
    pub results: Vec<ResolvedNote>,
}

fn scan_files(
    root: &Path,
    ignore_patterns: &[String],
    uuid_query: Option<&str>,
    require_todos: bool,
    todo_states: &[String],
) -> Vec<ResolvedNote> {
    let Ok(files) = discovery::walk_org_files(root, ignore_patterns) else {
        return Vec::new();
    };

    let do_full_scan = uuid_query.is_some() || require_todos;
    let mut notes = Vec::new();

    for path in &files {
        let Ok(content) = std::fs::read_to_string(path) else {
            continue;
        };

        if do_full_scan {
            let summary = parse_note_summary_with_todo_states(&content, todo_states);
            let Some(primary_uuid) = summary.uuids.first().cloned() else {
                continue;
            };
            let matched_heading = uuid_query.and_then(|uq| {
                let uq = uq.to_lowercase();
                summary
                    .uuids
                    .iter()
                    .skip(1)
                    .find(|u| u.to_lowercase().contains(&uq))
                    .cloned()
            });
            notes.push(resolved_note_from_parsed(
                path,
                &summary,
                primary_uuid.to_string(),
                matched_heading.map(|uuid| uuid.to_string()),
            ));
        } else {
            let header: Vec<&str> = content.lines().take(100).collect();
            let header_str = header.join("\n");
            let summary = parse_note_summary_with_todo_states(&header_str, todo_states);
            let Some(uuid) = summary.uuids.first().cloned() else {
                continue;
            };
            notes.push(resolved_note_from_parsed(
                path,
                &summary,
                uuid.to_string(),
                None,
            ));
        }
    }

    notes
}

fn resolved_note_from_parsed(
    path: &std::path::Path,
    summary: &ParsedNoteSummary,
    uuid: String,
    matched_heading: Option<String>,
) -> ResolvedNote {
    ResolvedNote {
        uuid,
        title: summary.title.clone(),
        path: path.display().to_string(),
        filetags: summary.filetags.clone(),
        categories: summary.categories.clone(),
        aliases: summary.aliases.clone(),
        matched_heading_uuid: matched_heading,
        has_todos: summary.has_todos,
        match_kind: None,
        matched_query: None,
    }
}

pub struct ResolveOptions {
    pub uuid: Option<String>,
    /// Title queries; each one is matched and limited independently.
    pub titles: Vec<String>,
    pub title_match: TitleMatchMode,
    pub tags: Option<Vec<String>>,
    /// Maximum results, applied per title query when titles are given.
    pub limit: Option<usize>,
    pub fields: Option<Vec<String>>,
    pub todos: bool,
    pub scope_filter: ScopeFilter,
}

pub struct ResolveCommandOutput {
    pub output: ResolveOutput,
    pub fields: Option<HashSet<String>>,
}

pub fn execute(config: &OrgConfig, opts: &ResolveOptions) -> Result<ResolveCommandOutput> {
    let db_root = config.db_root.as_path();
    let ignore = config.ignore_patterns.as_slice();
    let notes = scan_files(
        db_root,
        ignore,
        opts.uuid.as_deref(),
        opts.todos,
        &config.todo_states,
    );

    let mut candidates: Vec<ResolvedNote> = notes
        .into_iter()
        .filter(|n| {
            if let Some(ref uq) = opts.uuid
                && !n.uuid.to_lowercase().contains(&uq.to_lowercase())
                && n.matched_heading_uuid
                    .as_ref()
                    .is_none_or(|h| !h.to_lowercase().contains(&uq.to_lowercase()))
            {
                return false;
            }
            if let Some(ref tags) = opts.tags {
                let matches_tag = tags
                    .iter()
                    .any(|w| n.filetags.iter().any(|ft| ft.contains(w)));
                let matches_category = tags
                    .iter()
                    .any(|w| n.categories.iter().any(|c| c.contains(w)));
                if !matches_tag && !matches_category {
                    return false;
                }
            }
            if opts.todos && !n.has_todos {
                return false;
            }
            if !opts
                .scope_filter
                .matches(Path::new(&n.path), &n.filetags, true)
            {
                return false;
            }
            true
        })
        .collect();

    candidates.sort_by(|a, b| a.title.cmp(&b.title).then_with(|| a.uuid.cmp(&b.uuid)));

    let groups: Vec<Vec<ResolvedNote>> = if opts.titles.is_empty() {
        vec![candidates]
    } else {
        opts.titles
            .iter()
            .map(|query| match_title(&candidates, query, opts.title_match))
            .collect()
    };

    let total = groups.iter().map(Vec::len).sum();
    let shown: Vec<ResolvedNote> = groups
        .into_iter()
        .flat_map(|group| group.into_iter().take(opts.limit.unwrap_or(usize::MAX)))
        .collect();
    let showed = opts.limit.map(|_| shown.len());

    let field_set: Option<HashSet<String>> =
        opts.fields.as_ref().map(|f| f.iter().cloned().collect());

    let query_str = if opts.titles.is_empty() {
        opts.uuid
            .as_deref()
            .or_else(|| opts.tags.as_ref().map(|_| "tags"))
            .unwrap_or_default()
            .to_string()
    } else {
        opts.titles.join(" | ")
    };

    Ok(ResolveCommandOutput {
        output: ResolveOutput {
            query: query_str,
            titles: opts.titles.clone(),
            total,
            showed,
            results: shown,
        },
        fields: field_set,
    })
}

/// Notes matching one title query, strongest match kind first.
fn match_title(
    candidates: &[ResolvedNote],
    query: &str,
    mode: TitleMatchMode,
) -> Vec<ResolvedNote> {
    let mut matched: Vec<ResolvedNote> = candidates
        .iter()
        .filter_map(|n| {
            let kind =
                title_match_kind(&n.title, &n.aliases, query).filter(|k| mode.accepts(*k))?;
            let mut note = n.clone();
            note.match_kind = Some(kind);
            note.matched_query = Some(query.to_string());
            Some(note)
        })
        .collect();
    // Stable sort keeps the title/uuid order within each kind.
    matched.sort_by_key(|n| n.match_kind);
    matched
}

pub fn render_text(output: &ResolveOutput, field_set: Option<&HashSet<String>>) -> String {
    let mut text = String::new();

    if output.total == output.results.len() {
        let _ = writeln!(text, "Total: {}", output.results.len());
    } else {
        let _ = writeln!(
            text,
            "Total: {}, showed: {}",
            output.total,
            output.results.len()
        );
    }
    if output.titles.len() > 1 {
        for title in &output.titles {
            let notes: Vec<&ResolvedNote> = output
                .results
                .iter()
                .filter(|n| n.matched_query.as_deref() == Some(title.as_str()))
                .collect();
            let _ = writeln!(text, "Query: {title} ({})", notes.len());
            render_notes(&mut text, notes, field_set);
        }
    } else {
        render_notes(&mut text, output.results.iter(), field_set);
    }

    text
}

fn render_notes<'a>(
    text: &mut String,
    notes: impl IntoIterator<Item = &'a ResolvedNote>,
    field_set: Option<&HashSet<String>>,
) {
    for note in notes {
        if let Some(fs) = field_set {
            if fs.contains("title") {
                let _ = writeln!(text, "  {}", note.title);
            }
            if fs.contains("uuid") {
                let _ = writeln!(text, "         UUID: {}", note.uuid);
            }
            if fs.contains("path") {
                let _ = writeln!(text, "         Path: {}", note.path);
            }
            if fs.contains("tags") && !note.filetags.is_empty() {
                let _ = writeln!(text, "         Tags: {}", note.filetags.join(", "));
            }
            if fs.contains("categories") && !note.categories.is_empty() {
                let _ = writeln!(text, "         Cats: {}", note.categories.join(", "));
            }
            if fs.contains("aliases") && !note.aliases.is_empty() {
                let _ = writeln!(text, "         Aliases: {}", note.aliases.join(", "));
            }
        } else {
            let _ = writeln!(text, "  {}", note.title);
            let _ = writeln!(text, "         UUID: {}", note.uuid);
            if !note.filetags.is_empty() {
                let _ = writeln!(text, "         Tags: {}", note.filetags.join(", "));
            }
            if !note.categories.is_empty() {
                let _ = writeln!(text, "         Cats: {}", note.categories.join(", "));
            }
        }
    }
}

pub fn filter_fields(
    value: &serde_json::Value,
    fields: Option<&HashSet<String>>,
) -> serde_json::Value {
    let Some(fs) = fields else {
        return value.clone();
    };
    match value {
        serde_json::Value::Object(map) => {
            let mut filtered = serde_json::Map::new();
            for (k, v) in map {
                if fs.contains(k) || k == "score" || k == "matches" || k == "content_matches" {
                    filtered.insert(k.clone(), v.clone());
                }
            }
            serde_json::Value::Object(filtered)
        }
        _ => value.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resolved_note() -> ResolvedNote {
        ResolvedNote {
            uuid: "11111111-1111-4111-8111-111111111111".to_string(),
            title: "Note A".to_string(),
            path: "/notes/a.org".to_string(),
            filetags: vec!["tag".to_string()],
            categories: vec!["cat".to_string()],
            aliases: vec!["Alias A".to_string()],
            matched_heading_uuid: None,
            has_todos: false,
            match_kind: None,
            matched_query: None,
        }
    }

    #[test]
    fn renders_default_text_from_typed_output() {
        let output = ResolveOutput {
            query: "Note".to_string(),
            titles: vec!["Note".to_string()],
            total: 1,
            showed: None,
            results: vec![resolved_note()],
        };

        let text = render_text(&output, None);

        assert!(text.contains("Total: 1"));
        assert!(text.contains("  Note A"));
        assert!(text.contains("         UUID: 11111111-1111-4111-8111-111111111111"));
        assert!(text.contains("         Tags: tag"));
        assert!(text.contains("         Cats: cat"));
    }

    #[test]
    fn renders_selected_text_fields_and_limit_metadata() {
        let output = ResolveOutput {
            query: "Note".to_string(),
            titles: vec!["Note".to_string()],
            total: 3,
            showed: Some(1),
            results: vec![resolved_note()],
        };
        let fields = HashSet::from(["title".to_string(), "path".to_string()]);

        let text = render_text(&output, Some(&fields));

        assert!(text.contains("Total: 3, showed: 1"));
        assert!(text.contains("  Note A"));
        assert!(text.contains("         Path: /notes/a.org"));
        assert!(!text.contains("UUID:"));
        assert!(!text.contains("Tags:"));
    }

    #[test]
    fn test_filter_fields_none() {
        let v = serde_json::json!({"uuid": "abc", "title": "Test", "path": "/a.org", "filetags": ["tag1"]});
        let result = filter_fields(&v, None::<&HashSet<String>>);
        assert_eq!(result, v);
    }

    #[test]
    fn test_filter_fields_subset() {
        let v = serde_json::json!({"uuid": "abc", "title": "Test", "path": "/a.org", "filetags": ["tag1"]});
        let fields = Some(HashSet::from(["uuid".to_string(), "title".to_string()]));
        let result = filter_fields(&v, fields.as_ref());
        let obj = result.as_object().unwrap();
        assert!(obj.contains_key("uuid"));
        assert!(obj.contains_key("title"));
        assert!(!obj.contains_key("path"));
        assert!(!obj.contains_key("filetags"));
    }

    #[test]
    fn test_filter_fields_preserves_score() {
        let v = serde_json::json!({"uuid": "abc", "title": "Test", "score": 42.0, "filetags": []});
        let fields = Some(HashSet::from(["uuid".to_string()]));
        let result = filter_fields(&v, fields.as_ref());
        let obj = result.as_object().unwrap();
        assert!(obj.contains_key("uuid"));
        assert!(obj.contains_key("score"), "score should be preserved");
    }

    #[test]
    fn test_filter_fields_non_object() {
        let v = serde_json::json!("just a string");
        let fields = Some(HashSet::from(["key".to_string()]));
        let result = filter_fields(&v, fields.as_ref());
        assert_eq!(result, v);
    }

    #[test]
    fn resolve_todos_finds_tasks_after_header_scan_window() {
        let dir = tempfile::tempdir().unwrap();
        let mut content = String::from(
            r#":PROPERTIES:
:ID:       aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa
:END:
#+title: Alpha

"#,
        );
        for i in 0..110 {
            let _ = writeln!(content, "Line {i}");
        }
        content.push_str("* TODO Late task\n");
        std::fs::write(dir.path().join("alpha.org"), content).unwrap();
        let config = OrgConfig {
            db_root: dir.path().to_path_buf(),
            ignore_patterns: Vec::new(),
            home_dir: None,
            todo_states: vec!["TODO".to_string(), "DONE".to_string()],
        };
        let output = execute(
            &config,
            &ResolveOptions {
                uuid: None,
                titles: vec!["Alpha".to_string()],
                title_match: TitleMatchMode::Word,
                tags: None,
                limit: None,
                fields: None,
                todos: true,
                scope_filter: ScopeFilter::default(),
            },
        )
        .unwrap();

        assert_eq!(output.output.total, 1);
        assert!(output.output.results[0].has_todos);
    }

    fn aliases(values: &[&str]) -> Vec<String> {
        values.iter().map(|v| (*v).to_string()).collect()
    }

    #[test]
    fn title_match_kind_classifies_strongest_match() {
        let none: Vec<String> = Vec::new();
        assert_eq!(
            title_match_kind("Graph  Theory", &none, "graph theory"),
            Some(MatchKind::Exact)
        );
        assert_eq!(
            title_match_kind("Graph Theory", &aliases(&["GT"]), "gt"),
            Some(MatchKind::Alias)
        );
        assert_eq!(
            title_match_kind("Theory of Graph", &none, "graph theory"),
            Some(MatchKind::Word)
        );
        assert_eq!(
            title_match_kind("Graphs", &aliases(&["graph db"]), "graph"),
            Some(MatchKind::Word)
        );
        assert_eq!(
            title_match_kind("Paragraph style", &none, "graph"),
            Some(MatchKind::Substring)
        );
        assert_eq!(title_match_kind("Tree", &none, "graph"), None);
    }

    #[test]
    fn word_boundaries_handle_punctuation_and_unicode() {
        let none: Vec<String> = Vec::new();
        assert_eq!(
            title_match_kind("Using org-roam daily", &none, "org-roam"),
            Some(MatchKind::Word)
        );
        assert_eq!(
            title_match_kind("C++ templates", &none, "c++"),
            Some(MatchKind::Word)
        );
        assert_eq!(
            title_match_kind("Теория графов", &none, "граф"),
            Some(MatchKind::Substring)
        );
        assert_eq!(
            title_match_kind("snake_case names", &none, "snake"),
            Some(MatchKind::Substring)
        );
    }

    #[test]
    fn match_mode_limits_accepted_kinds() {
        assert!(TitleMatchMode::Exact.accepts(MatchKind::Alias));
        assert!(!TitleMatchMode::Exact.accepts(MatchKind::Word));
        assert!(TitleMatchMode::Word.accepts(MatchKind::Word));
        assert!(!TitleMatchMode::Word.accepts(MatchKind::Substring));
        assert!(TitleMatchMode::Substring.accepts(MatchKind::Substring));
    }

    fn write_note(dir: &Path, name: &str, uuid: &str, title: &str, alias: Option<&str>) {
        let alias = alias
            .map(|a| format!(":ROAM_ALIASES: \"{a}\"\n"))
            .unwrap_or_default();
        std::fs::write(
            dir.join(name),
            format!(":PROPERTIES:\n:ID:       {uuid}\n{alias}:END:\n#+title: {title}\n"),
        )
        .unwrap();
    }

    fn test_config(dir: &Path) -> OrgConfig {
        OrgConfig {
            db_root: dir.to_path_buf(),
            ignore_patterns: Vec::new(),
            home_dir: None,
            todo_states: vec!["TODO".to_string(), "DONE".to_string()],
        }
    }

    fn title_options(
        titles: &[&str],
        mode: TitleMatchMode,
        limit: Option<usize>,
    ) -> ResolveOptions {
        ResolveOptions {
            uuid: None,
            titles: titles.iter().map(|t| (*t).to_string()).collect(),
            title_match: mode,
            tags: None,
            limit,
            fields: None,
            todos: false,
            scope_filter: ScopeFilter::default(),
        }
    }

    fn graph_db() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let d = dir.path();
        write_note(
            d,
            "a.org",
            "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
            "Paragraph",
            None,
        );
        write_note(
            d,
            "b.org",
            "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb",
            "Graph theory",
            None,
        );
        write_note(
            d,
            "c.org",
            "cccccccc-cccc-4ccc-8ccc-cccccccccccc",
            "Graph",
            None,
        );
        write_note(
            d,
            "d.org",
            "dddddddd-dddd-4ddd-8ddd-dddddddddddd",
            "Networks",
            Some("graph"),
        );
        write_note(
            d,
            "e.org",
            "eeeeeeee-eeee-4eee-8eee-eeeeeeeeeeee",
            "Emacs",
            None,
        );
        dir
    }

    fn kinds(output: &ResolveOutput) -> Vec<(String, MatchKind)> {
        output
            .results
            .iter()
            .map(|n| (n.title.clone(), n.match_kind.unwrap()))
            .collect()
    }

    #[test]
    fn default_title_match_includes_substring_ranked_last() {
        let dir = graph_db();
        let out = execute(
            &test_config(dir.path()),
            &title_options(&["graph"], TitleMatchMode::default(), None),
        )
        .unwrap();

        assert_eq!(
            kinds(&out.output),
            vec![
                ("Graph".to_string(), MatchKind::Exact),
                ("Networks".to_string(), MatchKind::Alias),
                ("Graph theory".to_string(), MatchKind::Word),
                ("Paragraph".to_string(), MatchKind::Substring),
            ]
        );
    }

    #[test]
    fn exact_and_word_modes_bound_results() {
        let dir = graph_db();
        let config = test_config(dir.path());
        let exact = execute(
            &config,
            &title_options(&["graph"], TitleMatchMode::Exact, None),
        )
        .unwrap();
        assert_eq!(exact.output.total, 2);

        let word = execute(
            &config,
            &title_options(&["graph"], TitleMatchMode::Word, None),
        )
        .unwrap();
        assert_eq!(word.output.total, 3);
        assert!(
            word.output
                .results
                .iter()
                .all(|n| n.match_kind != Some(MatchKind::Substring))
        );
    }

    #[test]
    fn repeated_titles_group_results_and_limit_per_query() {
        let dir = graph_db();
        let out = execute(
            &test_config(dir.path()),
            &title_options(
                &["emacs", "graph", "missing"],
                TitleMatchMode::default(),
                Some(1),
            ),
        )
        .unwrap();

        let rows: Vec<_> = out
            .output
            .results
            .iter()
            .map(|n| (n.matched_query.as_deref().unwrap(), n.title.as_str()))
            .collect();
        assert_eq!(rows, vec![("emacs", "Emacs"), ("graph", "Graph")]);
        assert_eq!(out.output.total, 5);
        assert_eq!(out.output.showed, Some(2));
        assert_eq!(out.output.query, "emacs | graph | missing");

        let text = render_text(&out.output, None);
        assert!(text.contains("Query: emacs (1)"));
        assert!(text.contains("Query: missing (0)"));
    }
}
