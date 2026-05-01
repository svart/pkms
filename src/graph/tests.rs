use super::*;
use crate::parser::ParsedNote;

fn make_note(uuid: &str, title: &str, outgoing: Vec<Link>) -> FileScanResult {
    FileScanResult {
        path: PathBuf::from(format!("{}.org", uuid)),
        parsed: ParsedNote {
            uuid: Some(uuid.to_string()),
            title: Some(title.to_string()),
            filetags: vec![],
            roam_aliases: vec![],
            roam_refs: vec![],
            outgoing,
            headings: vec![],
        },
        parse_error: None,
    }
}

fn make_note_full(
    uuid: &str,
    title: &str,
    outgoing: Vec<Link>,
    filetags: Vec<String>,
    aliases: Vec<String>,
) -> FileScanResult {
    FileScanResult {
        path: PathBuf::from(format!("{}.org", uuid)),
        parsed: ParsedNote {
            uuid: Some(uuid.to_string()),
            title: Some(title.to_string()),
            filetags,
            roam_aliases: aliases,
            roam_refs: vec![],
            outgoing,
            headings: vec![],
        },
        parse_error: None,
    }
}

fn make_parse_error(_uuid: &str, path: &str) -> FileScanResult {
    FileScanResult {
        path: PathBuf::from(path),
        parsed: ParsedNote::empty(),
        parse_error: Some("mock error".to_string()),
    }
}

#[test]
fn test_graph_build() {
    let results = vec![
        make_note("a", "Note A", vec![Link::Internal("b".to_string())]),
        make_note("b", "Note B", vec![]),
    ];
    let graph = Graph::build(results);
    assert_eq!(graph.nodes.len(), 2);
    assert_eq!(graph.broken_links.len(), 0);
    assert_eq!(graph.backlinks.get("b").unwrap().len(), 1);
}

#[test]
fn test_broken_links() {
    let results = vec![make_note(
        "a",
        "Note A",
        vec![Link::Internal("nonexistent".to_string())],
    )];
    let graph = Graph::build(results);
    assert_eq!(graph.broken_links.len(), 1);
}

#[test]
fn test_orphan_detection() {
    let results = vec![
        make_note("a", "Orphan", vec![]),
        make_note("b", "Connected", vec![Link::Internal("c".to_string())]),
        make_note("c", "Target", vec![]),
    ];
    let graph = Graph::build(results);
    let stats = graph.stats();
    assert_eq!(stats.orphan_notes, 1);
}

#[test]
fn test_shortest_path() {
    let results = vec![
        make_note("a", "A", vec![Link::Internal("b".to_string())]),
        make_note("b", "B", vec![Link::Internal("c".to_string())]),
        make_note("c", "C", vec![]),
    ];
    let graph = Graph::build(results);
    let path = graph.find_shortest_path("a", "c", None);
    assert!(path.is_some());
    assert_eq!(path.unwrap(), vec!["a", "b", "c"]);
}

#[test]
fn test_duplicate_uuid_detection() {
    let results = vec![
        make_note("dup-uuid", "First", vec![]),
        make_note("dup-uuid", "Second", vec![]),
    ];
    let graph = Graph::build(results);
    assert_eq!(
        graph.nodes.len(),
        1,
        "only one node should exist for unique UUID"
    );
    assert_eq!(graph.duplicates.duplicate_uuids.len(), 1);
    assert_eq!(graph.duplicates.duplicate_uuids[0].value, "dup-uuid");
}

#[test]
fn test_duplicate_title_detection() {
    let results = vec![
        make_note("uuid-1", "Same Title", vec![]),
        make_note("uuid-2", "Same Title", vec![]),
    ];
    let graph = Graph::build(results);
    assert_eq!(graph.nodes.len(), 2);
    assert_eq!(graph.duplicates.duplicate_titles.len(), 1);
    assert_eq!(graph.duplicates.duplicate_titles[0].value, "Same Title");
}

#[test]
fn test_parse_error_handling() {
    let results = vec![
        make_note("a", "Good", vec![]),
        make_parse_error("bad", "broken.org"),
        make_note("b", "Also Good", vec![]),
    ];
    let graph = Graph::build(results);
    assert_eq!(graph.nodes.len(), 2);
    assert_eq!(graph.parse_errors.len(), 1);
    assert_eq!(graph.skipped_files.len(), 1);
}

#[test]
fn test_missing_title() {
    let results = vec![FileScanResult {
        path: PathBuf::from("no-title.org"),
        parsed: ParsedNote {
            uuid: Some("uuid-no-title".to_string()),
            title: None,
            filetags: vec![],
            roam_aliases: vec![],
            roam_refs: vec![],
            outgoing: vec![],
            headings: vec![],
        },
        parse_error: None,
    }];
    let graph = Graph::build(results);
    assert_eq!(graph.nodes.len(), 1);
    assert_eq!(graph.duplicates.missing_titles.len(), 1);
    let node = graph.nodes.get("uuid-no-title").unwrap();
    assert_eq!(node.title, "no-title");
}

#[test]
fn test_empty_results() {
    let graph = Graph::build(vec![]);
    assert_eq!(graph.nodes.len(), 0);
    assert_eq!(graph.broken_links.len(), 0);
    assert_eq!(graph.backlinks.len(), 0);
    assert!(graph.duplicates.duplicate_uuids.is_empty());
    assert!(graph.duplicates.duplicate_titles.is_empty());
}

#[test]
fn test_self_link_not_broken() {
    let results = vec![make_note(
        "self",
        "Self Link",
        vec![Link::Internal("self".to_string())],
    )];
    let graph = Graph::build(results);
    assert_eq!(graph.broken_links.len(), 0);
    assert_eq!(graph.backlinks.get("self").unwrap().len(), 1);
}

#[test]
fn test_find_node_by_uuid() {
    let results = vec![
        make_note("a", "Note A", vec![]),
        make_note("b", "Note B", vec![]),
    ];
    let graph = Graph::build(results);
    assert!(graph.find_node("a").is_some());
    assert_eq!(graph.find_node("a").unwrap().title, "Note A");
    assert!(graph.find_node("nonexistent").is_none());
}

#[test]
fn test_find_node_by_title() {
    let results = vec![make_note("a", "Unique Title", vec![])];
    let graph = Graph::build(results);
    let node = graph.find_node("Unique Title");
    assert!(node.is_some());
    assert_eq!(node.unwrap().uuid, "a");
}

#[test]
fn test_find_node_by_alias() {
    let results = vec![make_note_full(
        "a",
        "Note A",
        vec![],
        vec![],
        vec!["MyAlias".to_string()],
    )];
    let graph = Graph::build(results);
    let node = graph.find_node("MyAlias");
    assert!(node.is_some());
    assert_eq!(node.unwrap().uuid, "a");
}

#[test]
fn test_resolve_target_ok() {
    let results = vec![make_note("a", "Note A", vec![])];
    let graph = Graph::build(results);
    let node = graph.resolve_target("a");
    assert!(node.is_ok());
    assert_eq!(node.unwrap().title, "Note A");
}

#[test]
fn test_resolve_target_not_found() {
    let results = vec![make_note("a", "Note A", vec![])];
    let graph = Graph::build(results);
    let node = graph.resolve_target("nonexistent");
    assert!(node.is_err());
    assert!(node.unwrap_err().to_string().contains("not found"));
}

#[test]
fn test_broken_links_deduplication() {
    let results = vec![make_note(
        "a",
        "Note A",
        vec![
            Link::Internal("missing".to_string()),
            Link::Internal("missing".to_string()),
        ],
    )];
    let graph = Graph::build(results);
    assert_eq!(graph.broken_links.len(), 1);
}

#[test]
fn test_backlinks_count() {
    let results = vec![
        make_note("a", "A", vec![Link::Internal("target".to_string())]),
        make_note("b", "B", vec![Link::Internal("target".to_string())]),
        make_note("target", "Target", vec![]),
    ];
    let graph = Graph::build(results);
    assert_eq!(graph.backlinks.get("target").unwrap().len(), 2);
}

#[test]
fn test_hubs_ordering() {
    let results = vec![
        make_note(
            "a",
            "Hub A",
            vec![
                Link::Internal("x".to_string()),
                Link::Internal("y".to_string()),
            ],
        ),
        make_note("b", "Hub B", vec![Link::Internal("x".to_string())]),
        make_note("x", "X", vec![]),
        make_note("y", "Y", vec![]),
    ];
    let graph = Graph::build(results);
    let hubs = graph.hubs(10);
    assert_eq!(hubs.len(), 4);
    assert_eq!(hubs[0].1, 2, "top degree should be 2");
    assert_eq!(hubs[1].1, 2, "second degree should be 2");
    assert_eq!(hubs[2].1, 1, "third degree should be 1");
}

#[test]
fn test_url_and_file_links_not_counted_as_broken() {
    let results = vec![make_note(
        "a",
        "Note A",
        vec![
            Link::Url("https://example.com".to_string()),
            Link::File("/tmp/test".to_string()),
        ],
    )];
    let graph = Graph::build(results);
    assert_eq!(graph.broken_links.len(), 0);
}

#[test]
fn test_notes_by_tag() {
    let results = vec![
        make_note_full("a", "Tagged A", vec![], vec!["foo".to_string()], vec![]),
        make_note_full(
            "b",
            "Tagged B",
            vec![],
            vec!["foo".to_string(), "bar".to_string()],
            vec![],
        ),
        make_note_full("c", "Plain C", vec![], vec![], vec![]),
    ];
    let graph = Graph::build(results);
    let foo_notes = graph.notes_by_tag("foo");
    assert_eq!(foo_notes.len(), 2);
    let bar_notes = graph.notes_by_tag("bar");
    assert_eq!(bar_notes.len(), 1);
}

#[test]
fn test_all_tags() {
    let results = vec![
        make_note_full("a", "A", vec![], vec!["alpha".to_string()], vec![]),
        make_note_full(
            "b",
            "B",
            vec![],
            vec!["alpha".to_string(), "beta".to_string()],
            vec![],
        ),
        make_note_full("c", "C", vec![], vec!["beta".to_string()], vec![]),
    ];
    let graph = Graph::build(results);
    let tags = graph.all_tags();
    assert_eq!(tags.len(), 2);
    let alpha_count = tags
        .iter()
        .find(|(t, _)| t == "alpha")
        .map(|(_, c)| *c)
        .unwrap();
    assert_eq!(alpha_count, 2);
}

proptest::proptest! {
    #[test]
    fn test_graph_build_never_panics(
        uuids in proptest::collection::vec("[a-f0-9-]{1,36}", 0..5),
    ) {
        let results: Vec<FileScanResult> = uuids.iter().map(|uuid| {
            let parsed = ParsedNote {
                uuid: if uuid.is_empty() { None } else { Some(uuid.clone()) },
                title: Some("test".to_string()),
                filetags: vec![],
                roam_aliases: vec![],
                roam_refs: vec![],
                outgoing: vec![],
                headings: vec![],
            };
            FileScanResult {
                path: PathBuf::from(format!("{}.org", uuid)),
                parsed,
                parse_error: None,
            }
        }).collect();
        let _graph = Graph::build(results);
    }

    #[test]
    fn test_find_node_never_panics(
        uuid in "[a-f0-9-]{0,36}",
        search in ".{0,20}",
    ) {
        let results = vec![make_note("test-uuid", "Test Title", vec![])];
        let graph = Graph::build(results);
        let _ = graph.find_node(&uuid);
        let _ = graph.find_node(&search);
    }
}
