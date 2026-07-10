//! Process and HTTP fixtures shared by Todoist task integration scenarios.

use super::super::*;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::process::{Command, Output, Stdio};
use std::thread;
use std::time::{Duration, Instant};

const TODOIST_COMMAND_TIMEOUT: Duration = Duration::from_secs(30);
const TODOIST_MOCK_ACCEPT_TIMEOUT: Duration = Duration::from_secs(10);
const TODOIST_MOCK_READ_TIMEOUT: Duration = Duration::from_secs(2);

pub(super) fn run_with_todoist_env(args: &[&str], base_url: &str) -> Output {
    let config_home = setup_test_config_home();
    let mut command = Command::new(pkms_binary());
    configure_test_command(&mut command, config_home.path());
    command
        .args(args)
        .env("TODOIST_API_TOKEN", "test-token")
        .env("PKMS_TODOIST_API_BASE_URL", base_url)
        .env("COLUMNS", "120");
    output_with_timeout(&mut command)
}

pub(super) fn run_with_todoist_config_token(args: &[&str], base_url: &str) -> Output {
    let config_home = tempfile::tempdir().unwrap();
    std::fs::write(
        config_home.path().join("pkms.toml"),
        format!(
            r#"{TEST_CONFIG}

[todoist]
token = "config-token"
"#
        ),
    )
    .unwrap();
    let mut command = Command::new(pkms_binary());
    configure_test_command(&mut command, config_home.path());
    command
        .args(args)
        .env_remove("TODOIST_API_TOKEN")
        .env("PKMS_TODOIST_API_BASE_URL", base_url);
    output_with_timeout(&mut command)
}

fn output_with_timeout(command: &mut Command) -> Output {
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = command.spawn().expect("failed to spawn pkms command");
    let deadline = Instant::now() + TODOIST_COMMAND_TIMEOUT;
    loop {
        if child
            .try_wait()
            .expect("failed to poll pkms command")
            .is_some()
        {
            return child
                .wait_with_output()
                .expect("failed to collect pkms command output");
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let output = child
                .wait_with_output()
                .expect("failed to collect timed-out pkms command output");
            panic!(
                "pkms command timed out after {:?}\nstdout:\n{}\nstderr:\n{}",
                TODOIST_COMMAND_TIMEOUT,
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }
        thread::sleep(Duration::from_millis(10));
    }
}

pub(super) fn spawn_todoist_mock(
    responses: Vec<(&'static str, &'static str, &'static str)>,
) -> (String, thread::JoinHandle<()>) {
    spawn_todoist_mock_with_token(responses, "test-token")
}

pub(super) fn spawn_todoist_mock_expect_bodies(
    responses: Vec<(&'static str, &'static str, serde_json::Value, &'static str)>,
) -> (String, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let base_url = format!("http://{}", listener.local_addr().unwrap());
    let handle = thread::spawn(move || {
        for (method, expected_path, expected_body, body) in responses {
            let mut stream = accept_todoist_connection(&listener, method, expected_path);
            let request = read_todoist_request(&mut stream, method, expected_path);
            assert!(
                request.starts_with(&format!("{method} {expected_path} HTTP/1.1")),
                "unexpected request: {request}"
            );
            assert!(
                request
                    .to_ascii_lowercase()
                    .contains("authorization: bearer test-token")
            );
            if !expected_body.is_null() {
                let (_, request_body) = request
                    .split_once("\r\n\r\n")
                    .expect("expected request body separator");
                let actual_body: serde_json::Value = serde_json::from_str(request_body).unwrap();
                assert_eq!(actual_body, expected_body);
            }
            write_response(&mut stream, body);
        }
    });
    (base_url, handle)
}

pub(super) fn spawn_todoist_mock_with_token(
    responses: Vec<(&'static str, &'static str, &'static str)>,
    expected_token: &'static str,
) -> (String, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let base_url = format!("http://{}", listener.local_addr().unwrap());
    let handle = thread::spawn(move || {
        for (method, expected_path, body) in responses {
            let mut stream = accept_todoist_connection(&listener, method, expected_path);
            let request = read_todoist_request(&mut stream, method, expected_path);
            assert!(
                request.starts_with(&format!("{method} {expected_path} HTTP/1.1")),
                "unexpected request: {request}"
            );
            assert!(
                request
                    .to_ascii_lowercase()
                    .contains(&format!("authorization: bearer {expected_token}"))
            );
            write_response(&mut stream, body);
        }
    });
    (base_url, handle)
}

fn write_response(stream: &mut TcpStream, body: &str) {
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    );
    stream.write_all(response.as_bytes()).unwrap();
}

fn accept_todoist_connection(
    listener: &TcpListener,
    method: &'static str,
    expected_path: &'static str,
) -> TcpStream {
    let deadline = Instant::now() + TODOIST_MOCK_ACCEPT_TIMEOUT;
    loop {
        match listener.accept() {
            Ok((stream, _)) => return stream,
            Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                if Instant::now() >= deadline {
                    panic!(
                        "timed out after {:?} waiting for Todoist mock request {method} {expected_path}",
                        TODOIST_MOCK_ACCEPT_TIMEOUT
                    );
                }
                thread::sleep(Duration::from_millis(10));
            }
            Err(err) => {
                panic!("failed to accept Todoist mock request {method} {expected_path}: {err}")
            }
        }
    }
}

fn read_todoist_request(
    stream: &mut TcpStream,
    method: &'static str,
    expected_path: &'static str,
) -> String {
    stream
        .set_read_timeout(Some(TODOIST_MOCK_READ_TIMEOUT))
        .unwrap();
    let mut request_bytes = Vec::new();
    let mut buffer = [0_u8; 4096];
    loop {
        let read = stream.read(&mut buffer).unwrap_or_else(|err| {
            panic!(
                "failed to read Todoist mock request {method} {expected_path} within {:?}: {err}",
                TODOIST_MOCK_READ_TIMEOUT
            )
        });
        if read == 0 {
            break;
        }
        request_bytes.extend_from_slice(&buffer[..read]);
        if request_is_complete(&request_bytes) {
            break;
        }
    }
    String::from_utf8_lossy(&request_bytes).into_owned()
}

fn request_is_complete(request_bytes: &[u8]) -> bool {
    let request = String::from_utf8_lossy(request_bytes);
    let Some((headers, body)) = request.split_once("\r\n\r\n") else {
        return false;
    };
    let content_length = headers
        .lines()
        .find_map(|line| {
            line.to_ascii_lowercase()
                .strip_prefix("content-length:")
                .and_then(|value| value.trim().parse::<usize>().ok())
        })
        .unwrap_or(0);
    body.len() >= content_length
}
