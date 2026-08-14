use super::*;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::process::{Command, Stdio};

#[test]
fn test_serve_renders_initial_note_and_linked_note() {
    let _server_guard = lock_server_test();
    let (_dir, root) = setup_db();
    let config_home = setup_test_config_home();
    let mut child = Command::new(pkms_binary())
        .args([
            "--db",
            root.to_str().unwrap(),
            "serve",
            "Note A",
            "--port",
            "0",
        ])
        .env("XDG_CONFIG_HOME", config_home.path())
        .env_remove("PKMS_DB_ROOT")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn pkms serve");

    let stdout = child.stdout.take().unwrap();
    let mut reader = BufReader::new(stdout);
    let mut line = String::new();
    reader.read_line(&mut line).unwrap();
    assert!(
        line.starts_with("Serving http://"),
        "unexpected line: {line}"
    );
    let url = line.trim().strip_prefix("Serving ").unwrap();
    let (_, rest) = url.split_once("://").unwrap();
    let (host_port, path) = rest.split_once('/').unwrap();

    let response = http_get(host_port, &format!("/{path}"));
    assert!(response.contains("HTTP/1.1 200 OK"));
    assert!(response.contains("<h1>Note A</h1>"));
    assert!(response.contains("<details class=\"side-panel contents-panel\">"));
    assert!(response.contains("<details class=\"side-panel backlinks-panel\">"));
    assert!(response.contains("<link rel=\"icon\" href=\"/favicon.svg\" type=\"image/svg+xml\">"));
    assert!(response.contains("window.matchMedia(\"(min-width: 1361px)\")"));
    assert!(response.contains("id=\"note-preview\""));
    assert!(response.contains("href=\"/?id=bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb\""));
    assert!(response.contains("data-preview-id=\"bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb\""));

    let favicon = http_get(host_port, "/favicon.svg");
    assert!(favicon.contains("HTTP/1.1 200 OK"));
    assert!(favicon.contains("Content-Type: image/svg+xml"));
    assert!(favicon.contains("aria-label=\"pkms\""));
    assert!(favicon.contains("stroke=\"#f0c85a\""));

    let linked = http_get(host_port, "/?id=bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb");
    assert!(linked.contains("<h1>Note B</h1>"));
    assert!(linked.contains("<summary>Backlinks</summary>"));
    assert!(linked.contains("href=\"/?id=aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa\""));
    assert!(linked.contains("Note A"));

    let preview = http_get(
        host_port,
        "/preview?id=bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb",
    );
    assert!(preview.contains("HTTP/1.1 200 OK"));
    assert!(preview.contains("<article class=\"note-body note-preview-body\">"));
    assert!(preview.contains("href=\"/?id=cccccccc-cccc-4ccc-cccc-cccccccccccc\""));
    assert!(preview.contains("data-preview-id=\"cccccccc-cccc-4ccc-cccc-cccccccccccc\""));
    assert!(!preview.contains("contents-panel"));
    assert!(!preview.contains("backlinks-panel"));
    assert!(!preview.contains("<html"));

    child.kill().unwrap();
    let _ = child.wait();
}

#[test]
fn test_serve_heading_id_link_anchors_and_preview_block() {
    let _server_guard = lock_server_test();
    let db = TestDb::clean()
        .note_with_content(
            "a.org",
            r#":PROPERTIES:
:ID:       aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa
:END:
#+title: Note A

[[id:dddddddd-dddd-4ddd-8ddd-dddddddddddd][Target section]]
"#,
        )
        .note_with_content(
            "b.org",
            r#":PROPERTIES:
:ID:       bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb
:END:
#+title: Note B

* Before
Before body.
* Target Section
:PROPERTIES:
:ID:       dddddddd-dddd-4ddd-8ddd-dddddddddddd
:END:
Target body.
** Target Child
Child body.
* Sibling
Sibling body.
"#,
        );
    let (_dir, root) = db.into_parts();
    let config_home = setup_test_config_home();
    let mut child = Command::new(pkms_binary())
        .args([
            "--db",
            root.to_str().unwrap(),
            "serve",
            "Note A",
            "--port",
            "0",
        ])
        .env("XDG_CONFIG_HOME", config_home.path())
        .env_remove("PKMS_DB_ROOT")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn pkms serve");

    let stdout = child.stdout.take().unwrap();
    let mut reader = BufReader::new(stdout);
    let mut line = String::new();
    reader.read_line(&mut line).unwrap();
    assert!(
        line.starts_with("Serving http://"),
        "unexpected line: {line}"
    );
    let url = line.trim().strip_prefix("Serving ").unwrap();
    let (_, rest) = url.split_once("://").unwrap();
    let (host_port, path) = rest.split_once('/').unwrap();

    let response = http_get(host_port, &format!("/{path}"));
    assert!(response.contains(
        "href=\"/?id=bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb#h-8\" data-preview-id=\"dddddddd-dddd-4ddd-8ddd-dddddddddddd\""
    ));

    let linked_by_heading_id = http_get(host_port, "/?id=dddddddd-dddd-4ddd-8ddd-dddddddddddd");
    assert!(linked_by_heading_id.contains("<h1>Note B</h1>"));
    assert!(linked_by_heading_id.contains("<h2 id=\"h-8\">Target Section</h2>"));

    let preview = http_get(
        host_port,
        "/preview?id=dddddddd-dddd-4ddd-8ddd-dddddddddddd",
    );
    assert!(preview.contains("HTTP/1.1 200 OK"));
    assert!(preview.contains("<h1>Target Section</h1>"));
    assert!(preview.contains("<h2 id=\"h-1\">Target Section</h2>"));
    assert!(preview.contains("Target body."));
    assert!(preview.contains("<h3 id=\"h-6\">Target Child</h3>"));
    assert!(preview.contains("Child body."));
    assert!(!preview.contains("<h1>Note B</h1>"));
    assert!(!preview.contains("Before body."));
    assert!(!preview.contains("Sibling body."));

    child.kill().unwrap();
    let _ = child.wait();
}

#[test]
fn test_serve_accepts_db_relative_note_path() {
    let _server_guard = lock_server_test();
    let (_dir, root) = setup_db();
    let config_home = setup_test_config_home();
    let mut child = Command::new(pkms_binary())
        .args([
            "--db",
            root.to_str().unwrap(),
            "serve",
            "roam/common/20220101000000-note_a.org",
            "--port",
            "0",
        ])
        .env("XDG_CONFIG_HOME", config_home.path())
        .env_remove("PKMS_DB_ROOT")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn pkms serve");

    let stdout = child.stdout.take().unwrap();
    let mut reader = BufReader::new(stdout);
    let mut line = String::new();
    reader.read_line(&mut line).unwrap();
    assert!(
        line.starts_with("Serving http://"),
        "unexpected line: {line}"
    );

    child.kill().unwrap();
    let _ = child.wait();
}

#[test]
fn test_serve_accepts_cwd_relative_note_path() {
    let _server_guard = lock_server_test();
    let (_dir, root) = setup_db();
    let config_home = setup_test_config_home();
    let parent = root.parent().unwrap();
    let target = root.file_name().unwrap().to_string_lossy().to_string()
        + "/roam/common/20220101000000-note_a.org";
    let mut child = Command::new(pkms_binary())
        .current_dir(parent)
        .args([
            "--db",
            root.to_str().unwrap(),
            "serve",
            &target,
            "--port",
            "0",
        ])
        .env("XDG_CONFIG_HOME", config_home.path())
        .env_remove("PKMS_DB_ROOT")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn pkms serve");

    let stdout = child.stdout.take().unwrap();
    let mut reader = BufReader::new(stdout);
    let mut line = String::new();
    reader.read_line(&mut line).unwrap();
    assert!(
        line.starts_with("Serving http://"),
        "unexpected line: {line}"
    );

    child.kill().unwrap();
    let _ = child.wait();
}

#[test]
fn test_serve_renders_markdown_without_org_only_ui() {
    let _server_guard = lock_server_test();
    let db = tempfile::tempdir().unwrap();
    let root = db.path().to_path_buf();
    let files = tempfile::tempdir().unwrap();
    let config_home = setup_test_config_home();
    let docs = files.path().join("docs");
    std::fs::create_dir_all(&docs).unwrap();
    let markdown_path = docs.join("guide.md");
    std::fs::write(
        &markdown_path,
        r#"# Markdown Guide

Intro with **strong text**.

## Details

<script>alert("unsafe")</script>
"#,
    )
    .unwrap();

    let mut child = Command::new(pkms_binary())
        .args([
            "--db",
            root.to_str().unwrap(),
            "serve",
            markdown_path.to_str().unwrap(),
            "--port",
            "0",
        ])
        .env("XDG_CONFIG_HOME", config_home.path())
        .env_remove("PKMS_DB_ROOT")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn pkms serve");

    let stdout = child.stdout.take().unwrap();
    let mut reader = BufReader::new(stdout);
    let mut line = String::new();
    reader.read_line(&mut line).unwrap();
    assert!(
        line.starts_with("Serving http://"),
        "unexpected line: {line}"
    );
    let url = line.trim().strip_prefix("Serving ").unwrap();
    let (_, rest) = url.split_once("://").unwrap();
    let (host_port, path) = rest.split_once('/').unwrap();

    let response = http_get(host_port, &format!("/{path}"));
    assert!(response.contains("HTTP/1.1 200 OK"));
    assert!(response.contains("<h1 id=\"h-1\">Markdown Guide</h1>"));
    assert!(response.contains("<strong>strong text</strong>"));
    assert!(response.contains("href=\"#h-5\""));
    assert!(response.contains(&format!(
        "data-open-url=\"/open?file={}\"",
        percent_encode(markdown_path.canonicalize().unwrap().to_str().unwrap())
    )));
    assert!(response.contains("&lt;script&gt;alert(\"unsafe\")&lt;/script&gt;"));
    assert!(!response.contains("<details class=\"side-panel backlinks-panel\">"));
    assert!(!response.contains("class=\"uuid\""));
    assert!(!response.contains("id=\"note-preview\""));

    child.kill().unwrap();
    let _ = child.wait();
}

#[test]
fn test_serve_renders_external_org_without_graph_only_ui() {
    let _server_guard = lock_server_test();
    let db = tempfile::tempdir().unwrap();
    let files = tempfile::tempdir().unwrap();
    let config_home = setup_test_config_home();
    let org_path = files.path().join("external.org");
    std::fs::write(
        &org_path,
        r#":PROPERTIES:
:ID:       dddddddd-dddd-4ddd-8ddd-dddddddddddd
:END:
#+title: External Org

* Section
Body with *bold text*.
"#,
    )
    .unwrap();

    let mut child = Command::new(pkms_binary())
        .args([
            "--db",
            db.path().to_str().unwrap(),
            "serve",
            org_path.to_str().unwrap(),
            "--port",
            "0",
        ])
        .env("XDG_CONFIG_HOME", config_home.path())
        .env_remove("PKMS_DB_ROOT")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn pkms serve");

    let stdout = child.stdout.take().unwrap();
    let mut reader = BufReader::new(stdout);
    let mut line = String::new();
    reader.read_line(&mut line).unwrap();
    assert!(
        line.starts_with("Serving http://"),
        "unexpected line: {line}"
    );
    let url = line.trim().strip_prefix("Serving ").unwrap();
    let (_, rest) = url.split_once("://").unwrap();
    let (host_port, path) = rest.split_once('/').unwrap();

    let response = http_get(host_port, &format!("/{path}"));
    assert!(response.contains("HTTP/1.1 200 OK"));
    assert!(response.contains("<h1>External Org</h1>"));
    assert!(response.contains("<h2 id=\"h-6\">Section</h2>"));
    assert!(response.contains("Body with <strong>bold text</strong>."));
    assert!(response.contains("href=\"#h-6\""));
    assert!(response.contains(&format!(
        "data-open-url=\"/open?file={}\"",
        percent_encode(org_path.canonicalize().unwrap().to_str().unwrap())
    )));
    assert!(!response.contains("<details class=\"side-panel backlinks-panel\">"));
    assert!(!response.contains("class=\"uuid\""));
    assert!(!response.contains("dddddddd-dddd-4ddd-8ddd-dddddddddddd"));
    assert!(!response.contains("id=\"note-preview\""));

    child.kill().unwrap();
    let _ = child.wait();
}

#[test]
fn test_serve_ndjson_startup_emits_single_json_line() {
    let _server_guard = lock_server_test();
    let (_dir, root) = setup_db();
    let config_home = setup_test_config_home();
    let mut child = Command::new(pkms_binary())
        .args([
            "--db",
            root.to_str().unwrap(),
            "--output-format",
            "ndjson",
            "serve",
            "Note A",
            "--port",
            "0",
        ])
        .env("XDG_CONFIG_HOME", config_home.path())
        .env_remove("PKMS_DB_ROOT")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn pkms serve");

    let stdout = child.stdout.take().unwrap();
    let mut reader = BufReader::new(stdout);
    let mut line = String::new();
    reader.read_line(&mut line).unwrap();
    let value = assert_single_ndjson_object(&["serve", "Note A"], &line);
    assert_eq!(value["uuid"], "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa");
    assert!(value["url"].as_str().unwrap().starts_with("http://"));

    child.kill().unwrap();
    let _ = child.wait();
}

#[test]
fn test_serve_asset_endpoint_only_serves_linked_note_assets() {
    let _server_guard = lock_server_test();
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().to_path_buf();
    let home = tempfile::tempdir().unwrap();
    let config_home = setup_test_config_home();
    let roam = root.join("roam");
    let assets = roam.join("assets");
    std::fs::create_dir_all(&assets).unwrap();
    std::fs::create_dir_all(root.join(".attach/aa/aaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa")).unwrap();
    std::fs::write(roam.join("linked.txt"), "linked file").unwrap();
    std::fs::write(
        root.join(".attach/aa/aaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa/attach.txt"),
        "linked attachment",
    )
    .unwrap();
    std::fs::write(roam.join("secret.txt"), "db secret").unwrap();
    std::fs::write(home.path().join("home-secret.txt"), "home secret").unwrap();
    std::fs::write(
        roam.join("a.org"),
        r#":PROPERTIES:
:ID:       aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa
:END:
#+title: Alpha

[[file:linked.txt][Linked file]]
[[attachment:attach.txt][Linked attachment]]
"#,
    )
    .unwrap();

    let mut child = Command::new(pkms_binary())
        .args([
            "--db",
            root.to_str().unwrap(),
            "serve",
            "Alpha",
            "--port",
            "0",
        ])
        .env("XDG_CONFIG_HOME", config_home.path())
        .env("HOME", home.path())
        .env_remove("PKMS_DB_ROOT")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn pkms serve");

    let stdout = child.stdout.take().unwrap();
    let mut reader = BufReader::new(stdout);
    let mut line = String::new();
    reader.read_line(&mut line).unwrap();
    assert!(
        line.starts_with("Serving http://"),
        "unexpected line: {line}"
    );
    let url = line.trim().strip_prefix("Serving ").unwrap();
    let (_, rest) = url.split_once("://").unwrap();
    let (host_port, _) = rest.split_once('/').unwrap();

    let linked_file = http_get(
        host_port,
        &asset_path("aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa", "file", "linked.txt"),
    );
    assert!(linked_file.contains("HTTP/1.1 200 OK"));
    assert!(linked_file.contains("linked file"));

    let linked_attachment = http_get(
        host_port,
        &asset_path(
            "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa",
            "attachment",
            "attach.txt",
        ),
    );
    assert!(linked_attachment.contains("HTTP/1.1 200 OK"));
    assert!(linked_attachment.contains("linked attachment"));

    for rejected_target in [
        home.path()
            .join("home-secret.txt")
            .to_string_lossy()
            .to_string(),
        roam.join("secret.txt").to_string_lossy().to_string(),
        "assets/../secret.txt".to_string(),
    ] {
        let response = http_get(
            host_port,
            &asset_path(
                "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa",
                "file",
                &rejected_target,
            ),
        );
        assert!(
            response.contains("HTTP/1.1 404 Not Found"),
            "unexpected response for {rejected_target}:\n{response}"
        );
        assert!(!response.contains("secret"));
    }

    child.kill().unwrap();
    let _ = child.wait();
}

#[test]
fn test_serve_heading_scoped_attachment() {
    let _server_guard = lock_server_test();
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().to_path_buf();
    let config_home = setup_test_config_home();
    let roam = root.join("roam");
    let note_uuid = "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa";
    let heading_uuid = "bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb";
    let attachment_dir = root
        .join(".attach")
        .join(&heading_uuid[..2])
        .join(&heading_uuid[2..]);
    std::fs::create_dir_all(&roam).unwrap();
    std::fs::create_dir_all(&attachment_dir).unwrap();
    std::fs::write(attachment_dir.join("heading.png"), "heading attachment").unwrap();
    std::fs::write(
        roam.join("a.org"),
        format!(
            r#":PROPERTIES:
:ID:       {note_uuid}
:END:
#+title: Alpha

* Attached image
:PROPERTIES:
:ID:       {heading_uuid}
:END:

[[attachment:heading.png]]
"#
        ),
    )
    .unwrap();

    let mut child = Command::new(pkms_binary())
        .args([
            "--db",
            root.to_str().unwrap(),
            "serve",
            "Alpha",
            "--port",
            "0",
        ])
        .env("XDG_CONFIG_HOME", config_home.path())
        .env_remove("PKMS_DB_ROOT")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn pkms serve");

    let stdout = child.stdout.take().unwrap();
    let mut reader = BufReader::new(stdout);
    let mut line = String::new();
    reader.read_line(&mut line).unwrap();
    let url = line
        .trim()
        .strip_prefix("Serving ")
        .expect("serve startup URL");
    let (_, rest) = url.split_once("://").unwrap();
    let (host_port, path) = rest.split_once('/').unwrap();

    let page = http_get(host_port, &format!("/{path}"));
    assert!(page.contains(&format!(
        "/asset?note={heading_uuid}&amp;kind=attachment&amp;target=heading.png"
    )));

    let attachment = http_get(
        host_port,
        &asset_path(heading_uuid, "attachment", "heading.png"),
    );
    assert!(attachment.contains("HTTP/1.1 200 OK"));
    assert!(attachment.contains("heading attachment"));

    child.kill().unwrap();
    let _ = child.wait();
}

fn http_get(host_port: &str, path: &str) -> String {
    let mut stream = TcpStream::connect(host_port).unwrap();
    let write_result = write!(
        stream,
        "GET {path} HTTP/1.1\r\nHost: {host_port}\r\nConnection: close\r\n\r\n"
    );
    if let Err(err) = write_result {
        assert_eq!(err.kind(), std::io::ErrorKind::BrokenPipe);
    }
    let mut response = Vec::new();
    if let Err(err) = stream.read_to_end(&mut response) {
        assert_eq!(err.kind(), std::io::ErrorKind::ConnectionReset);
    }
    assert_complete_http_response(&response, path);
    String::from_utf8(response).expect("serve response should be utf-8")
}

fn asset_path(note_uuid: &str, kind: &str, target: &str) -> String {
    format!(
        "/asset?note={}&kind={}&target={}",
        percent_encode(note_uuid),
        percent_encode(kind),
        percent_encode(target)
    )
}

fn percent_encode(text: &str) -> String {
    let mut encoded = String::new();
    for byte in text.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(byte as char);
            }
            _ => encoded.push_str(&format!("%{byte:02X}")),
        }
    }
    encoded
}

fn assert_complete_http_response(response: &[u8], path: &str) {
    let header_end = response
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .map(|index| index + 4)
        .unwrap_or_else(|| panic!("incomplete HTTP headers for {path}"));
    let headers = std::str::from_utf8(&response[..header_end])
        .unwrap_or_else(|_| panic!("HTTP headers should be utf-8 for {path}"));
    let content_len = headers
        .lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.eq_ignore_ascii_case("content-length")
                .then(|| value.trim().parse::<usize>().ok())
                .flatten()
        })
        .unwrap_or_else(|| panic!("missing Content-Length header for {path}"));
    let body_len = response.len() - header_end;
    assert!(
        body_len >= content_len,
        "partial HTTP response body for {path}: expected {content_len} bytes, got {body_len}"
    );
}
