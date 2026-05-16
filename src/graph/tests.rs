use super::*;
use crate::graph::search::SearchFields;
use crate::parser::ParsedNote;

fn make_note(uuid: &str, title: &str, outgoing: Vec<Link>) -> FileScanResult {
    FileScanResult {
        path: PathBuf::from(format!("{}.org", uuid)),
        parsed: ParsedNote {
            uuids: vec![uuid.to_string()],
            title: Some(title.to_string()),
            filetags: vec![],
            categories: vec![],
            aliases: vec![],
            roam_refs: vec![],
            outgoing,
            headings: vec![],
        },
        raw_content: None,
        parse_error: None,
    }
}

fn make_note_with_headings(
    uuid: &str,
    title: &str,
    outgoing: Vec<Link>,
    heading_uuids: Vec<&str>,
) -> FileScanResult {
    let headings: Vec<crate::parser::Heading> = heading_uuids
        .into_iter()
        .map(|huid| crate::parser::Heading {
            level: 1,
            title: format!("Heading {}", huid),
            todo_state: None,
            tags: vec![],
            uuid: Some(huid.to_string()),
            scheduled: None,
            deadline: None,
            priority: None,
            line_number: 1,
            outgoing: vec![],
            raw: format!("* Heading {}", huid),
        })
        .collect();
    FileScanResult {
        path: PathBuf::from(format!("{}.org", uuid)),
        parsed: ParsedNote {
            uuids: vec![uuid.to_string()],
            title: Some(title.to_string()),
            filetags: vec![],
            categories: vec![],
            aliases: vec![],
            roam_refs: vec![],
            outgoing,
            headings,
        },
        raw_content: None,
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
            uuids: vec![uuid.to_string()],
            title: Some(title.to_string()),
            filetags,
            categories: vec![],
            aliases: aliases,
            roam_refs: vec![],
            outgoing,
            headings: vec![],
        },
        raw_content: None,
        parse_error: None,
    }
}

fn make_parse_error(_uuid: &str, path: &str) -> FileScanResult {
    FileScanResult {
        path: PathBuf::from(path),
        parsed: ParsedNote::empty(),
        raw_content: None,
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
        FileScanResult {
            path: PathBuf::from("first.org"),
            parsed: ParsedNote {
                uuids: vec!["dup-uuid".to_string()],
                title: Some("First".to_string()),
                filetags: vec![],
                categories: vec![],
                aliases: vec![],
                roam_refs: vec![],
                outgoing: vec![],
                headings: vec![],
            },
            raw_content: None,
            parse_error: None,
        },
        FileScanResult {
            path: PathBuf::from("second.org"),
            parsed: ParsedNote {
                uuids: vec!["dup-uuid".to_string()],
                title: Some("Second".to_string()),
                filetags: vec![],
                categories: vec![],
                aliases: vec![],
                roam_refs: vec![],
                outgoing: vec![],
                headings: vec![],
            },
            raw_content: None,
            parse_error: None,
        },
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
            uuids: vec!["uuid-no-title".to_string()],
            title: None,
            filetags: vec![],
            categories: vec![],
            aliases: vec![],
            roam_refs: vec![],
            outgoing: vec![],
            headings: vec![],
        },
        raw_content: None,
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
fn test_heading_uuid_resolves_to_own_node() {
    let results = vec![
        make_note_with_headings("parent-uuid", "Parent Note", vec![], vec!["heading-uuid-1"]),
        make_note(
            "other",
            "Other Note",
            vec![Link::Internal("heading-uuid-1".to_string())],
        ),
    ];
    let graph = Graph::build(results);
    // heading UUID should resolve to its own node, not the parent
    let node = graph.find_node("heading-uuid-1");
    assert!(node.is_some());
    assert_eq!(node.unwrap().uuid, "heading-uuid-1");
    assert_eq!(node.unwrap().title, "Heading heading-uuid-1");
    // heading UUID should be in heading_uuid_to_primary
    assert_eq!(
        graph.heading_uuid_to_primary.get("heading-uuid-1"),
        Some(&"parent-uuid".to_string())
    );
    // Link to heading UUID should not be broken
    assert_eq!(graph.broken_links.len(), 0);
    // Heading node should have parent-child edge back to parent
    let heading_node = graph.nodes.get("heading-uuid-1").unwrap();
    assert!(
        heading_node
            .outgoing
            .iter()
            .any(|l| matches!(l, Link::Internal(u) if u == "parent-uuid"))
    );
}

#[test]
fn test_heading_uuid_duplicate_detection() {
    let results = vec![
        make_note_with_headings("a", "Note A", vec![], vec!["dup-heading-uuid"]),
        make_note_with_headings("b", "Note B", vec![], vec!["dup-heading-uuid"]),
    ];
    let graph = Graph::build(results);
    assert_eq!(graph.path_to_uuid.len(), 2, "both notes should be indexed");
    assert!(
        graph
            .duplicates
            .duplicate_uuids
            .iter()
            .any(|d| d.value == "dup-heading-uuid"),
        "heading UUID duplicate should be reported"
    );
}

#[test]
fn test_intra_file_heading_heading_duplicate() {
    let results = vec![make_note_with_headings(
        "a",
        "Note A",
        vec![],
        vec!["same-heading-uuid", "same-heading-uuid"],
    )];
    let graph = Graph::build(results);
    assert_eq!(graph.path_to_uuid.len(), 1, "note should be indexed");
    let dups = &graph.duplicates.duplicate_uuids;
    assert!(
        dups.iter().any(|d| d.value == "same-heading-uuid"),
        "two headings sharing a UUID should be reported, got: {:?}",
        dups.iter().map(|d| &d.value).collect::<Vec<_>>()
    );
}

#[test]
fn test_heading_uuid_clashes_with_primary_uuid() {
    let results = vec![
        make_note("primary-uuid-a", "Note A", vec![]),
        make_note_with_headings("note-b", "Note B", vec![], vec!["primary-uuid-a"]),
    ];
    let graph = Graph::build(results);
    assert!(graph.nodes.contains_key("primary-uuid-a"));
    let dups = &graph.duplicates.duplicate_uuids;
    assert!(
        dups.iter().any(|d| d.value == "primary-uuid-a"),
        "heading UUID clashing with primary UUID should be reported, got: {:?}",
        dups.iter().map(|d| &d.value).collect::<Vec<_>>()
    );
}

#[test]
fn test_intra_file_heading_uuid_duplicate() {
    let results = vec![
        // Note where heading UUID equals its own primary UUID
        make_note_with_headings("same-uuid", "Note", vec![], vec!["same-uuid"]),
    ];
    let graph = Graph::build(results);
    let dups = &graph.duplicates.duplicate_uuids;
    assert!(
        dups.iter().any(|d| d.value == "same-uuid"),
        "intra-file heading UUID duplicate should be reported, got: {:?}",
        dups.iter().map(|d| &d.value).collect::<Vec<_>>()
    );
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

#[test]
fn test_search_title_alias_equal_score() {
    let results = vec![
        make_note_full("a", "Quantum Physics", vec![], vec![], vec![]),
        make_note_full(
            "b",
            "Unrelated",
            vec![],
            vec![],
            vec!["Quantum Physics".to_string()],
        ),
    ];
    let graph = Graph::build(results);
    let title_results = graph.search("Quantum Physics", &Default::default());
    assert_eq!(title_results.len(), 2, "both notes should match");
    let note_a = title_results
        .iter()
        .find(|(n, _, _)| n.uuid == "a")
        .unwrap();
    let note_b = title_results
        .iter()
        .find(|(n, _, _)| n.uuid == "b")
        .unwrap();
    assert_eq!(
        note_a.1, note_b.1,
        "title match and alias match should score identically"
    );
}

#[test]
fn test_search_title_outranks_ref() {
    let results = vec![
        make_note_full("a", "Quantum Theory", vec![], vec![], vec![]),
        make_note("b", "Other", vec![]),
    ];
    let mut graph = Graph::build(results);
    if let Some(node) = graph.nodes.get_mut("b") {
        node.refs.push("quantum".to_string());
    }
    let title_results = graph.search("Quantum", &Default::default());
    let note_a = title_results
        .iter()
        .find(|(n, _, _)| n.uuid == "a")
        .unwrap();
    let note_b = title_results
        .iter()
        .find(|(n, _, _)| n.uuid == "b")
        .unwrap();
    assert!(
        note_a.1 > note_b.1,
        "title score should be higher than ref score"
    );
}

#[test]
fn test_search_title_outranks_tag() {
    let results = vec![
        make_note_full("a", "Quantum Theory", vec![], vec![], vec![]),
        make_note_full("b", "Other", vec![], vec!["quantum".to_string()], vec![]),
    ];
    let graph = Graph::build(results);
    let title_results = graph.search("Quantum", &Default::default());
    let note_a = title_results
        .iter()
        .find(|(n, _, _)| n.uuid == "a")
        .unwrap();
    let note_b = title_results
        .iter()
        .find(|(n, _, _)| n.uuid == "b")
        .unwrap();
    assert!(
        note_a.1 > note_b.1,
        "title score should be higher than tag score"
    );
}

#[test]
fn test_search_ref_outranks_tag() {
    let results = vec![
        make_note("a", "Other", vec![]),
        make_note_full(
            "b",
            "Unrelated",
            vec![],
            vec!["quantum".to_string()],
            vec![],
        ),
    ];
    let mut graph = Graph::build(results);
    if let Some(node) = graph.nodes.get_mut("a") {
        node.refs.push("quantum".to_string());
    }
    let title_results = graph.search("quantum", &Default::default());
    let note_a = title_results
        .iter()
        .find(|(n, _, _)| n.uuid == "a")
        .unwrap();
    let note_b = title_results
        .iter()
        .find(|(n, _, _)| n.uuid == "b")
        .unwrap();
    assert!(
        note_a.1 > note_b.1,
        "ref score (6) should be higher than tag score (5)"
    );
}

#[test]
fn test_shortest_path_same_node() {
    let results = vec![make_note("a", "A", vec![])];
    let graph = Graph::build(results);
    let path = graph.find_shortest_path("a", "a", None);
    assert!(path.is_some());
    assert_eq!(path.unwrap(), vec!["a"]);
}

#[test]
fn test_shortest_path_max_depth() {
    let results = vec![
        make_note("a", "A", vec![Link::Internal("b".to_string())]),
        make_note("b", "B", vec![Link::Internal("c".to_string())]),
        make_note("c", "C", vec![]),
    ];
    let graph = Graph::build(results);
    // depth limit 1 should prevent reaching c
    let path = graph.find_shortest_path("a", "c", Some(1));
    assert!(path.is_none(), "should not find path with depth limit 1");
    // depth limit 2 should work
    let path2 = graph.find_shortest_path("a", "c", Some(2));
    assert!(path2.is_some());
    assert_eq!(path2.unwrap(), vec!["a", "b", "c"]);
}

#[test]
fn test_shortest_path_via_backlinks() {
    let results = vec![
        make_note("a", "A", vec![Link::Internal("c".to_string())]),
        make_note("b", "B", vec![]),
        make_note("c", "C", vec![]),
    ];
    let graph = Graph::build(results);
    // path from b to c: b has no outgoing, but a links to c and b has no link to a
    // b -> backlinks of b are empty, so no path
    let path = graph.find_shortest_path("b", "c", None);
    assert!(path.is_none());

    // path from c to a: c has no outgoing, but a links to c (backlink)
    let path2 = graph.find_shortest_path("c", "a", None);
    assert!(
        path2.is_some(),
        "should find path from c to a via backlinks"
    );
    let p2 = path2.unwrap();
    assert_eq!(p2[0], "c");
    assert_eq!(*p2.last().unwrap(), "a".to_string());
}

#[test]
fn test_shortest_path_no_path() {
    let results = vec![make_note("a", "A", vec![]), make_note("b", "B", vec![])];
    let graph = Graph::build(results);
    let path = graph.find_shortest_path("a", "b", None);
    assert!(path.is_none());
}

#[test]
fn test_get_neighbors_no_outgoing() {
    let results = vec![make_note("a", "A", vec![])];
    let graph = Graph::build(results);
    let neighbors = graph.get_neighbors("a", 1);
    assert!(neighbors.is_empty() || neighbors.get(&1).is_some());
}

#[test]
fn test_get_neighbors_broken_outgoing() {
    let results = vec![make_note(
        "a",
        "A",
        vec![Link::Internal("nonexistent".to_string())],
    )];
    let graph = Graph::build(results);
    let neighbors = graph.get_neighbors("a", 1);
    assert_eq!(neighbors.len(), 1);
    let n1 = neighbors.get(&1).unwrap();
    assert_eq!(n1.outgoing.len(), 0, "broken target is not in nodes");
    assert_eq!(
        n1.broken_outgoing.len(),
        1,
        "broken link should be reported"
    );
}

#[test]
fn test_shortest_path_backlink_traversal() {
    let results = vec![
        make_note("a", "A", vec![Link::Internal("b".to_string())]),
        make_note("c", "C", vec![Link::Internal("b".to_string())]),
        make_note("d", "D", vec![Link::Internal("c".to_string())]),
        make_note("b", "B", vec![]),
    ];
    let graph = Graph::build(results);
    // Path from a to d: a -> b (backlink finds c) -> c (forward) -> d
    // At b, backlinks include a (visited) and c (not visited, not target) -> traverse through c -> d
    let path = graph.find_shortest_path("a", "d", None);
    assert!(
        path.is_some(),
        "should find path from a to d via backlink traversal"
    );
    let p = path.unwrap();
    assert_eq!(p[0], "a");
    assert_eq!(*p.last().unwrap(), "d".to_string());
}

#[test]
fn test_get_neighbors_with_depth() {
    let results = vec![
        make_note("a", "A", vec![Link::Internal("b".to_string())]),
        make_note("b", "B", vec![Link::Internal("c".to_string())]),
        make_note("c", "C", vec![]),
    ];
    let graph = Graph::build(results);
    let neighbors = graph.get_neighbors("a", 1);
    assert_eq!(neighbors.len(), 1);
    let n1 = neighbors.get(&1).unwrap();
    assert_eq!(n1.outgoing.len(), 1);
    assert_eq!(n1.outgoing[0].uuid, "b");

    let neighbors2 = graph.get_neighbors("a", 2);
    assert_eq!(neighbors2.len(), 2);
    let n2 = neighbors2.get(&2).unwrap();
    assert_eq!(n2.outgoing.len(), 1);
    assert_eq!(n2.outgoing[0].uuid, "c");
}

#[test]
fn test_search_by_tag_only() {
    let results = vec![
        make_note_full("a", "Note A", vec![], vec!["quantum".to_string()], vec![]),
        make_note_full("b", "Note B", vec![], vec![], vec![]),
    ];
    let graph = Graph::build(results);
    let fields = SearchFields {
        title: false,
        alias: false,
        ref_: false,
        tag: true,
        category: false,
    };
    let results = graph.search("quantum", &fields);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].0.uuid, "a");
}

#[test]
fn test_search_by_ref() {
    let results = vec![make_note_full("a", "A", vec![], vec![], vec![])];
    let mut graph = Graph::build(results);
    if let Some(node) = graph.nodes.get_mut("a") {
        node.refs.push("reference-keyword".to_string());
    }
    let fields = SearchFields {
        title: false,
        alias: false,
        ref_: true,
        tag: false,
        category: false,
    };
    let results = graph.search("reference-keyword", &fields);
    assert_eq!(results.len(), 1);
}

#[test]
fn test_search_by_category() {
    let results = vec![make_note_full("a", "Category Note", vec![], vec![], vec![])];
    let mut graph = Graph::build(results);
    if let Some(node) = graph.nodes.get_mut("a") {
        node.categories.push("example-category".to_string());
    }
    let fields = SearchFields {
        title: false,
        alias: false,
        ref_: false,
        tag: false,
        category: true,
    };
    let results = graph.search("example-category", &fields);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].0.uuid, "a");
}

#[test]
fn test_search_content_found() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test_note.org");
    std::fs::write(
        &path,
        r#":PROPERTIES:
:ID:       aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa
:END:
#+title: Content Test

This is the content with a unique-searchable-keyword here.
"#,
    )
    .unwrap();

    let results = vec![FileScanResult {
        path: path.clone(),
        parsed: crate::parser::ParsedNote {
            uuids: vec!["aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa".to_string()],
            title: Some("Content Test".to_string()),
            filetags: vec![],
            categories: vec![],
            aliases: vec![],
            roam_refs: vec![],
            outgoing: vec![],
            headings: vec![],
        },
        raw_content: None,
        parse_error: None,
    }];
    let graph = Graph::build(results);
    let content_results = graph.search_content("unique-searchable-keyword");
    assert_eq!(content_results.len(), 1);
    assert_eq!(content_results[0].1, "Content Test");
    assert!(!content_results[0].2.is_empty());
}

#[test]
fn test_search_content_not_found() {
    let results = vec![make_note("a", "A", vec![])];
    let graph = Graph::build(results);
    let content_results = graph.search_content("nonexistent");
    assert!(content_results.is_empty());
}

#[test]
fn test_detect_overlinks_basic() {
    let results = vec![make_note(
        "uuid1",
        "Note A",
        vec![
            Link::Internal("target".to_string()),
            Link::Internal("target".to_string()),
        ],
    )];
    let graph = Graph::build(results);
    let overlinks = graph.detect_overlinks();
    assert_eq!(overlinks.len(), 1);
    assert_eq!(overlinks[0].source_uuid, "uuid1");
    assert_eq!(overlinks[0].target_uuid, "target");
    assert_eq!(overlinks[0].count, 2);
}

#[test]
fn test_detect_overlinks_single_not_reported() {
    let results = vec![make_note(
        "uuid1",
        "Note A",
        vec![Link::Internal("target".to_string())],
    )];
    let graph = Graph::build(results);
    let overlinks = graph.detect_overlinks();
    assert!(
        overlinks.is_empty(),
        "single link should not be overlinking"
    );
}

#[test]
fn test_detect_overlinks_heading_uuid_not_double_counted() {
    // Rule 1: Link from UUID1 to X and link from heading UUID2 to X is NOT overlinking
    // (In the current model, all outgoing links are file-level; a single link is not overlinking)
    let results = vec![make_note_with_headings(
        "uuid1",
        "Note A",
        vec![Link::Internal("target".to_string())],
        vec!["heading-uuid"],
    )];
    let graph = Graph::build(results);
    // Only 1 link to target in the file, so no overlinking
    let overlinks = graph.detect_overlinks();
    assert!(
        overlinks.is_empty(),
        "single link should not be overlinking even with heading UUID"
    );
}

#[test]
fn test_detect_overlinks_heading_uuid_dedup() {
    // Rules 2 & 3: 2+ links from UUID1 or heading UUID2 to same target IS overlinking.
    // Heading UUID nodes share the primary node's uuid field and outgoing links,
    // so they are deduped by seen_primaries. Overlinking is reported once under the primary UUID.
    let results = vec![make_note_with_headings(
        "uuid1",
        "Note A",
        vec![
            Link::Internal("target".to_string()),
            Link::Internal("target".to_string()),
        ],
        vec!["heading-uuid"],
    )];
    let graph = Graph::build(results);
    let overlinks = graph.detect_overlinks();
    // Should report exactly 1 overlinking entry (not 2 — the heading UUID clone is deduped)
    assert_eq!(overlinks.len(), 1, "heading UUID clone should be deduped");
    assert_eq!(overlinks[0].source_uuid, "uuid1");
    assert_eq!(overlinks[0].count, 2);
}

#[test]
fn test_detect_overlinks_multiple_targets() {
    let results = vec![make_note(
        "uuid1",
        "Note A",
        vec![
            Link::Internal("x".to_string()),
            Link::Internal("x".to_string()),
            Link::Internal("y".to_string()),
            Link::Internal("y".to_string()),
            Link::Internal("y".to_string()),
        ],
    )];
    let graph = Graph::build(results);
    let overlinks = graph.detect_overlinks();
    assert_eq!(overlinks.len(), 2);
    let counts: std::collections::HashMap<&str, usize> = overlinks
        .iter()
        .map(|e| (e.target_uuid.as_str(), e.count))
        .collect();
    assert_eq!(counts.get("x"), Some(&2));
    assert_eq!(counts.get("y"), Some(&3));
}

proptest::proptest! {
    #[test]
    fn test_graph_build_never_panics(
        uuids in proptest::collection::vec("[a-f0-9-]{1,36}", 0..5),
    ) {
        let results: Vec<FileScanResult> = uuids.iter().map(|uuid| {
            let parsed = ParsedNote {
                uuids: if uuid.is_empty() { vec![] } else { vec![uuid.clone()] },
                title: Some("test".to_string()),
                filetags: vec![],
                categories: vec![],
                aliases: vec![],
                roam_refs: vec![],
                outgoing: vec![],
                headings: vec![],
            };
            FileScanResult {
                path: PathBuf::from(format!("{}.org", uuid)),
                parsed,
                raw_content: None,
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
