use anyhow::{Result, bail};
use pkms_org::graph::{Graph, Node};
use pkms_org::mentions::{MentionHit, MentionKind, MentionMatcher, linked_ids};
use pkms_org::{NoteId, OrgConfig, ScopeFilter};
use rayon::prelude::*;
use serde::Serialize;
use std::borrow::Cow;
use std::collections::HashMap;
use std::fmt::Write;
use std::path::Path;

pub enum MentionsSource {
    /// A note or heading target resolved through the graph.
    Target(String),
    /// Org text that does not have to exist in the database (stdin).
    Text(String),
}

pub struct MentionsOptions {
    pub source: MentionsSource,
    pub incoming: bool,
    /// Also match heading nodes with `:ID:` as mentioned notes (outgoing only).
    pub include_headings: bool,
    /// Ignore titles and aliases shorter than this many chars (outgoing only).
    pub min_length: usize,
    pub scope_filter: ScopeFilter,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum MentionsMode {
    Outgoing,
    Incoming,
}

#[derive(Debug, Serialize)]
pub struct MentionsOutput {
    pub target: Option<String>,
    pub target_uuid: Option<String>,
    pub mode: MentionsMode,
    pub count: usize,
    pub mentions: Vec<Mention>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Mention {
    pub source_uuid: Option<String>,
    pub source_title: Option<String>,
    pub path: Option<String>,
    pub line: usize,
    pub col: usize,
    pub phrase: String,
    pub uuid: String,
    pub title: String,
    #[serde(rename = "match")]
    pub match_kind: MentionKind,
    pub already_linked: bool,
}

/// The text being scanned and where it came from.
struct ScanRegion<'a> {
    source: Option<&'a Node>,
    text: Cow<'a, str>,
    /// Added to matcher line numbers so they refer to the source file.
    line_offset: usize,
}

pub fn execute(config: &OrgConfig, opts: &MentionsOptions) -> Result<MentionsOutput> {
    let graph = crate::load_graph(config)?;
    match (&opts.source, opts.incoming) {
        (MentionsSource::Text(_), true) => {
            bail!("--incoming needs a note target; it cannot read text from stdin")
        }
        (MentionsSource::Text(text), false) => {
            let region = ScanRegion {
                source: None,
                text: Cow::Borrowed(text),
                line_offset: 0,
            };
            Ok(outgoing(&graph, &region, opts))
        }
        (MentionsSource::Target(target), false) => {
            let node = graph.resolve_target(target)?;
            let region = region_for_node(&graph, node)?;
            Ok(outgoing(&graph, &region, opts))
        }
        (MentionsSource::Target(target), true) => {
            let node = graph.resolve_target(target)?;
            Ok(incoming(&graph, node, &opts.scope_filter))
        }
    }
}

fn is_heading_node(graph: &Graph, node: &Node) -> bool {
    graph.primary_uuid_for_heading(&node.uuid).is_some()
}

fn region_for_node<'g>(graph: &'g Graph, node: &'g Node) -> Result<ScanRegion<'g>> {
    let Some(content) = graph.raw_content_for_path(&node.path) else {
        bail!("Content not available for {}", node.path.display());
    };
    let Some(location) = graph.heading_location(&node.uuid) else {
        return Ok(ScanRegion {
            source: Some(node),
            text: Cow::Borrowed(content),
            line_offset: 0,
        });
    };

    // A heading target scans only its subtree: from the heading line up to the
    // next heading at the same or a higher level.
    let headings = graph
        .file(&node.path)
        .map(|file| file.parsed.headings.as_slice())
        .unwrap_or_default();
    let level = headings
        .iter()
        .find(|heading| heading.line_number == location.line_number)
        .map_or(1, |heading| heading.level);
    let end = headings
        .iter()
        .find(|heading| heading.line_number > location.line_number && heading.level <= level)
        .map(|heading| heading.line_number - 1);
    let start = location.line_number - 1;
    let lines: Vec<&str> = content.lines().collect();
    let end = end.unwrap_or(lines.len()).min(lines.len());

    Ok(ScanRegion {
        source: Some(node),
        text: Cow::Owned(lines[start..end].join("\n")),
        line_offset: start,
    })
}

fn outgoing(graph: &Graph, region: &ScanRegion<'_>, opts: &MentionsOptions) -> MentionsOutput {
    let source_path = region.source.map(|node| node.path.as_path());
    let is_same_file = |node: &Node| source_path.is_some_and(|path| node.path == path);

    // Names from the source file are matched too, so that the note's own title
    // consumes its text instead of leaving shorter names to match inside it.
    // Those hits are dropped below.
    let entries = graph
        .nodes()
        .filter(|node| {
            is_same_file(node)
                || ((opts.include_headings || !is_heading_node(graph, node))
                    && opts.scope_filter.matches(&node.path, &node.filetags, false))
        })
        .flat_map(|node| names_for(graph, node))
        .filter(|(name, _, _)| name.trim().chars().count() >= opts.min_length);
    let matcher = MentionMatcher::new(entries);

    let linked = linked_ids(&region.text);
    let mut mentions: Vec<Mention> = matcher
        .find_mentions(&region.text)
        .iter()
        .flat_map(|hit| {
            hit.targets.iter().filter_map(|target| {
                let node = graph.node(&target.uuid)?;
                (!is_same_file(node))
                    .then(|| mention(region, hit, node, target.kind, linked.contains(&*node.uuid)))
            })
        })
        .collect();
    sort_mentions(&mut mentions);

    MentionsOutput {
        target: region.source.map(|node| node.title.clone()),
        target_uuid: region.source.map(|node| node.uuid.to_string()),
        mode: MentionsMode::Outgoing,
        count: mentions.len(),
        mentions,
    }
}

fn incoming(graph: &Graph, target: &Node, scope_filter: &ScopeFilter) -> MentionsOutput {
    // The user named this note explicitly, so --min-length does not apply.
    let matcher = MentionMatcher::new(names_for(graph, target));
    let contents: HashMap<&Path, &str> = graph
        .files()
        .iter()
        .filter_map(|file| Some((file.path.as_path(), file.raw_content.as_deref()?)))
        .collect();
    let sources: Vec<&Node> = graph
        .nodes()
        .filter(|node| {
            !is_heading_node(graph, node)
                && node.path != target.path
                && scope_filter.matches(&node.path, &node.filetags, true)
        })
        .collect();

    let mut mentions: Vec<Mention> = sources
        .par_iter()
        .flat_map_iter(|source| {
            let region = ScanRegion {
                source: Some(source),
                text: Cow::Borrowed(
                    contents
                        .get(source.path.as_path())
                        .copied()
                        .unwrap_or_default(),
                ),
                line_offset: 0,
            };
            let already_linked = linked_ids(&region.text).contains(&*target.uuid);
            let hits = matcher.find_mentions(&region.text);
            hits.iter()
                .flat_map(|hit| {
                    hit.targets
                        .iter()
                        .map(|t| mention(&region, hit, target, t.kind, already_linked))
                })
                .collect::<Vec<_>>()
        })
        .collect();
    sort_mentions(&mut mentions);

    MentionsOutput {
        target: Some(target.title.clone()),
        target_uuid: Some(target.uuid.to_string()),
        mode: MentionsMode::Incoming,
        count: mentions.len(),
        mentions,
    }
}

/// Names a node can be mentioned by. Heading nodes inherit the file's aliases
/// in the graph, so only their own title identifies them.
fn names_for<'g>(graph: &Graph, node: &'g Node) -> Vec<(&'g str, NoteId, MentionKind)> {
    let mut names = vec![(node.title.as_str(), node.uuid.clone(), MentionKind::Title)];
    if !is_heading_node(graph, node) {
        names.extend(
            node.aliases
                .iter()
                .map(|alias| (alias.as_str(), node.uuid.clone(), MentionKind::Alias)),
        );
    }
    names
}

fn mention(
    region: &ScanRegion<'_>,
    hit: &MentionHit<'_>,
    node: &Node,
    kind: MentionKind,
    already_linked: bool,
) -> Mention {
    Mention {
        source_uuid: region.source.map(|source| source.uuid.to_string()),
        source_title: region.source.map(|source| source.title.clone()),
        path: region
            .source
            .map(|source| source.path.display().to_string()),
        line: hit.line + region.line_offset,
        col: hit.col,
        phrase: hit.phrase.clone(),
        uuid: node.uuid.to_string(),
        title: node.title.clone(),
        match_kind: kind,
        already_linked,
    }
}

fn sort_mentions(mentions: &mut [Mention]) {
    mentions
        .sort_by(|a, b| (&a.path, a.line, a.col, &a.uuid).cmp(&(&b.path, b.line, b.col, &b.uuid)));
}

pub fn render_text(output: &MentionsOutput) -> String {
    let mut text = String::new();
    let target = output.target.as_deref().unwrap_or("stdin");
    match output.mode {
        MentionsMode::Outgoing => {
            let _ = writeln!(text, "Mentions in \"{target}\" ({}):", output.count);
            for m in &output.mentions {
                let _ = writeln!(
                    text,
                    "  {}:{}  \"{}\" -> {} ({}){}",
                    m.line,
                    m.col,
                    m.phrase,
                    m.title,
                    m.uuid,
                    flags(m)
                );
            }
        }
        MentionsMode::Incoming => {
            let _ = writeln!(text, "Mentions of \"{target}\" ({}):", output.count);
            let mut current_source = None;
            for m in &output.mentions {
                if current_source != m.source_uuid.as_deref() {
                    current_source = m.source_uuid.as_deref();
                    let _ = writeln!(
                        text,
                        "  {} ({})",
                        m.source_title.as_deref().unwrap_or_default(),
                        current_source.unwrap_or_default()
                    );
                }
                let _ = writeln!(
                    text,
                    "    {}:{}  \"{}\"{}",
                    m.line,
                    m.col,
                    m.phrase,
                    flags(m)
                );
            }
        }
    }
    text
}

fn flags(mention: &Mention) -> String {
    let mut flags = Vec::new();
    if mention.match_kind == MentionKind::Alias {
        flags.push("alias");
    }
    if mention.already_linked {
        flags.push("already linked");
    }
    if flags.is_empty() {
        String::new()
    } else {
        format!(" [{}]", flags.join(", "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    const RUST: &str = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
    const ML: &str = "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb";
    const LEARNING: &str = "cccccccc-cccc-4ccc-8ccc-cccccccccccc";
    const GO: &str = "dddddddd-dddd-4ddd-8ddd-dddddddddddd";
    const SOURCE: &str = "eeeeeeee-eeee-4eee-8eee-eeeeeeeeeeee";
    const TOKIO: &str = "ffffffff-ffff-4fff-8fff-ffffffffffff";
    const DAILY: &str = "11111111-1111-4111-8111-111111111111";

    fn note(dir: &Path, name: &str, id: &str, title: &str, extra: &str) {
        fs::write(
            dir.join(name),
            format!(":PROPERTIES:\n:ID:       {id}\n:END:\n#+title: {title}\n{extra}"),
        )
        .unwrap();
    }

    fn fixture() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::write(
            root.join("rust.org"),
            format!(
                ":PROPERTIES:\n:ID:       {RUST}\n:ROAM_ALIASES: rustlang\n:END:\n#+title: Rust\n\
             * Tokio Runtime\n:PROPERTIES:\n:ID:       ffffffff-ffff-4fff-8fff-ffffffffffff\n:END:\n\
             Async Rust on machine learning.\n\
             * Other\nMore rustlang.\n"
            ),
        )
        .unwrap();
        note(
            root,
            "ml.org",
            ML,
            "Machine Learning",
            "Machine learning is fun.\n",
        );
        note(root, "learning.org", LEARNING, "Learning", "");
        note(root, "go.org", GO, "Go", "");
        note(
            root,
            "source.org",
            SOURCE,
            "Source",
            "I write Rust for machine learning, and Go.\n\
             Uses Tokio Runtime. Linked: [[id:aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa][it]].\n",
        );
        note(
            root,
            "2026-01-01.org",
            DAILY,
            "2026-01-01",
            "Played with rustlang.\n",
        );
        dir
    }

    fn config(dir: &tempfile::TempDir) -> OrgConfig {
        OrgConfig {
            db_root: dir.path().to_path_buf(),
            ignore_patterns: Vec::new(),
            home_dir: None,
            todo_states: vec!["TODO".to_string(), "DONE".to_string()],
        }
    }

    fn options(source: MentionsSource) -> MentionsOptions {
        MentionsOptions {
            source,
            incoming: false,
            include_headings: false,
            min_length: 3,
            scope_filter: ScopeFilter::default(),
        }
    }

    fn summary(output: &MentionsOutput) -> Vec<(usize, usize, &str, &str)> {
        output
            .mentions
            .iter()
            .map(|m| (m.line, m.col, m.phrase.as_str(), m.uuid.as_str()))
            .collect()
    }

    #[test]
    fn outgoing_lists_note_mentions_with_positions_and_link_state() {
        let dir = fixture();

        let output = execute(
            &config(&dir),
            &options(MentionsSource::Target(SOURCE.to_string())),
        )
        .unwrap();

        assert_eq!(
            summary(&output),
            vec![(5, 9, "Rust", RUST), (5, 18, "machine learning", ML)]
        );
        assert!(output.mentions[0].already_linked);
        assert!(!output.mentions[1].already_linked);
        assert_eq!(output.mentions[0].source_uuid.as_deref(), Some(SOURCE));
    }

    #[test]
    fn outgoing_includes_heading_nodes_and_short_names_on_request() {
        let dir = fixture();
        let mut opts = options(MentionsSource::Target(SOURCE.to_string()));
        opts.include_headings = true;
        opts.min_length = 2;

        let output = execute(&config(&dir), &opts).unwrap();

        let uuids: Vec<&str> = output.mentions.iter().map(|m| m.uuid.as_str()).collect();
        assert_eq!(uuids, vec![RUST, ML, GO, TOKIO]);
    }

    #[test]
    fn outgoing_never_reports_the_note_itself_or_names_inside_its_title() {
        let dir = fixture();

        let output = execute(
            &config(&dir),
            &options(MentionsSource::Target(ML.to_string())),
        )
        .unwrap();

        assert!(output.mentions.is_empty(), "{:?}", output.mentions);
    }

    #[test]
    fn heading_target_scans_only_its_subtree() {
        let dir = fixture();

        let output = execute(
            &config(&dir),
            &options(MentionsSource::Target(TOKIO.to_string())),
        )
        .unwrap();

        assert_eq!(summary(&output), vec![(10, 15, "machine learning", ML)]);
    }

    #[test]
    fn stdin_text_has_no_source_and_excludes_dailies_by_default() {
        let dir = fixture();

        let output = execute(
            &config(&dir),
            &options(MentionsSource::Text(
                "Rust on 2026-01-01.\n[[id:aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa]]".to_string(),
            )),
        )
        .unwrap();

        assert_eq!(summary(&output), vec![(1, 1, "Rust", RUST)]);
        assert!(output.mentions[0].already_linked);
        assert_eq!(output.mentions[0].path, None);
        assert_eq!(output.target, None);
    }

    #[test]
    fn incoming_lists_other_notes_naming_the_target_including_dailies() {
        let dir = fixture();
        let mut opts = options(MentionsSource::Target(RUST.to_string()));
        opts.incoming = true;

        let output = execute(&config(&dir), &opts).unwrap();

        let sources: Vec<(&str, &str, MentionKind, bool)> = output
            .mentions
            .iter()
            .map(|m| {
                (
                    m.source_uuid.as_deref().unwrap(),
                    m.phrase.as_str(),
                    m.match_kind,
                    m.already_linked,
                )
            })
            .collect();
        assert_eq!(
            sources,
            vec![
                (DAILY, "rustlang", MentionKind::Alias, false),
                (SOURCE, "Rust", MentionKind::Title, true),
            ]
        );
        assert!(output.mentions.iter().all(|m| m.uuid == RUST));
    }

    #[test]
    fn incoming_rejects_stdin_text() {
        let dir = fixture();
        let mut opts = options(MentionsSource::Text("Rust".to_string()));
        opts.incoming = true;

        let err = execute(&config(&dir), &opts).unwrap_err();

        assert!(err.to_string().contains("--incoming"), "{err}");
    }

    #[test]
    fn renders_outgoing_text() {
        let output = MentionsOutput {
            target: Some("Source".to_string()),
            target_uuid: Some(SOURCE.to_string()),
            mode: MentionsMode::Outgoing,
            count: 1,
            mentions: vec![Mention {
                source_uuid: Some(SOURCE.to_string()),
                source_title: Some("Source".to_string()),
                path: Some("/db/source.org".to_string()),
                line: 5,
                col: 9,
                phrase: "rustlang".to_string(),
                uuid: RUST.to_string(),
                title: "Rust".to_string(),
                match_kind: MentionKind::Alias,
                already_linked: true,
            }],
        };

        assert_eq!(
            render_text(&output),
            format!(
                "Mentions in \"Source\" (1):\n  5:9  \"rustlang\" -> Rust ({RUST}) [alias, already linked]\n"
            )
        );
    }
}
