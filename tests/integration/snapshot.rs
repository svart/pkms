use super::*;

#[test]
fn test_snapshot_path() {
    let (_dir, root) = setup_db();
    let (stdout, _stderr, _status) =
        run(&["--db", root.to_str().unwrap(), "path", "Note A", "Note C"]);
    insta::assert_snapshot!("path_human", normalize_snapshot(&stdout, &root));
}

#[test]
fn test_snapshot_resolve() {
    let (_dir, root) = setup_db();
    let (stdout, _stderr, _status) =
        run(&["--db", root.to_str().unwrap(), "resolve", "--title", "Note"]);
    insta::assert_snapshot!("resolve_query_human", normalize_snapshot(&stdout, &root));
}
