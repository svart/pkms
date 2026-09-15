use super::{LinkCheckJob, split_file_link_line_spec};
use crate::link_check::model::{LinkCheckErrorKind, LinkCheckResults, link_check_error};
#[cfg(feature = "ssh")]
use crate::link_check::model::{SshErrorKind, link_check_broken};
#[cfg(feature = "ssh")]
use rayon::prelude::*;
#[cfg(feature = "ssh")]
use std::collections::BTreeMap;
#[cfg(feature = "ssh")]
use std::path::Path;
use std::path::PathBuf;
#[cfg(feature = "ssh")]
use std::sync::Arc;
use std::time::Duration;

const DEFAULT_SSH_CONNECT_TIMEOUT_MS: u64 = 5_000;
const DEFAULT_SSH_OPERATION_TIMEOUT_MS: u32 = 5_000;
const DEFAULT_SSH_MAX_CONNECTIONS: usize = 4;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct SshConnectionKey {
    pub user: String,
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SshFileTarget {
    pub connection: SshConnectionKey,
    pub path: String,
    pub line_spec: Option<String>,
    pub raw_target: String,
}

#[derive(Debug, Clone)]
pub struct SshFileCheckOptions {
    pub default_user: String,
    pub identity_files: Vec<PathBuf>,
    pub known_hosts: PathBuf,
    pub agent_socket: Option<PathBuf>,
    pub connect_timeout: Duration,
    pub operation_timeout_ms: u32,
    pub max_connections: usize,
}

impl Default for SshFileCheckOptions {
    fn default() -> Self {
        Self {
            default_user: String::new(),
            identity_files: Vec::new(),
            known_hosts: PathBuf::from("~/.ssh/known_hosts"),
            agent_socket: None,
            connect_timeout: Duration::from_millis(DEFAULT_SSH_CONNECT_TIMEOUT_MS),
            operation_timeout_ms: DEFAULT_SSH_OPERATION_TIMEOUT_MS,
            max_connections: DEFAULT_SSH_MAX_CONNECTIONS,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SshFileTargetParseErrorKind {
    EmptyUser,
    EmptyHost,
    InvalidPort,
    MissingPath,
    UnsupportedSyntax,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SshFileTargetParseError {
    pub kind: SshFileTargetParseErrorKind,
    pub message: String,
}

pub fn parse_ssh_file_target(
    target: &str,
    default_user: &str,
) -> Result<Option<SshFileTarget>, SshFileTargetParseError> {
    let normalized = normalized_file_target(target);
    if !normalized.starts_with("/ssh:") {
        return Ok(None);
    }

    let (without_line_spec, line_spec) = split_file_link_line_spec(normalized);
    let rest = without_line_spec.trim_start_matches("/ssh:");
    if rest.contains('|') {
        return Err(ssh_parse_error(
            SshFileTargetParseErrorKind::UnsupportedSyntax,
            "SSH file links do not support multi-hop TRAMP syntax",
        ));
    }

    let (login, path) = rest.split_once(':').ok_or_else(|| {
        ssh_parse_error(
            SshFileTargetParseErrorKind::MissingPath,
            "SSH file link is missing a remote path",
        )
    })?;
    if path.is_empty() {
        return Err(ssh_parse_error(
            SshFileTargetParseErrorKind::MissingPath,
            "SSH file link is missing a remote path",
        ));
    }
    if !path.starts_with('/') {
        return Err(ssh_parse_error(
            SshFileTargetParseErrorKind::MissingPath,
            "SSH file links must use absolute remote paths",
        ));
    }

    let (user, host_and_port) = if let Some((user, host_and_port)) = login.rsplit_once('@') {
        if user.is_empty() {
            return Err(ssh_parse_error(
                SshFileTargetParseErrorKind::EmptyUser,
                "SSH file link has an empty user",
            ));
        }
        (user, host_and_port)
    } else {
        (default_user, login)
    };
    if user.is_empty() {
        return Err(ssh_parse_error(
            SshFileTargetParseErrorKind::EmptyUser,
            "SSH file link needs a user or a non-empty default user",
        ));
    }

    let (host, port) = parse_host_and_port(host_and_port)?;
    Ok(Some(SshFileTarget {
        connection: SshConnectionKey {
            user: user.to_string(),
            host,
            port,
        },
        path: path.to_string(),
        line_spec: line_spec.map(str::to_string),
        raw_target: target.to_string(),
    }))
}

#[cfg(not(feature = "ssh"))]
pub fn run_ssh_link_checks(
    jobs: Vec<LinkCheckJob>,
    _options: &SshFileCheckOptions,
) -> LinkCheckResults {
    let mut results = LinkCheckResults {
        broken: Vec::new(),
        errors: jobs
            .into_iter()
            .map(|job| {
                link_check_error(
                    job,
                    LinkCheckErrorKind::Unsupported,
                    "SSH file-link checks are not available in this build. Rebuild with --features ssh.",
                )
            })
            .collect(),
    };
    results.sort();
    results
}

#[cfg(feature = "ssh")]
pub fn run_ssh_link_checks(
    jobs: Vec<LinkCheckJob>,
    options: &SshFileCheckOptions,
) -> LinkCheckResults {
    let mut results = LinkCheckResults::default();
    let mut grouped: BTreeMap<SshConnectionKey, Vec<SshParsedJob>> = BTreeMap::new();

    for job in jobs {
        match parse_ssh_file_target(job.target.as_str(), &options.default_user) {
            Ok(Some(target)) => grouped
                .entry(target.connection.clone())
                .or_default()
                .push(SshParsedJob { job, target }),
            Ok(None) => results.errors.push(link_check_error(
                job,
                LinkCheckErrorKind::Unsupported,
                "remote file-link check received a non-SSH file target",
            )),
            Err(error) => results.errors.push(link_check_error(
                job,
                LinkCheckErrorKind::Unsupported,
                error.message,
            )),
        }
    }

    let groups: Vec<_> = grouped.into_iter().collect();
    for group_result in run_ssh_groups(groups, options) {
        results.extend(group_result);
    }
    results.sort();
    results
}

#[cfg(feature = "ssh")]
#[derive(Debug)]
struct SshParsedJob {
    job: LinkCheckJob,
    target: SshFileTarget,
}

#[cfg(feature = "ssh")]
#[derive(Debug, Clone)]
struct SshCheckFailure {
    kind: LinkCheckErrorKind,
    message: String,
}

#[cfg(feature = "ssh")]
impl SshCheckFailure {
    fn new(kind: impl Into<LinkCheckErrorKind>, message: impl Into<String>) -> Self {
        Self {
            kind: kind.into(),
            message: message.into(),
        }
    }
}

#[cfg(feature = "ssh")]
fn run_ssh_groups(
    groups: Vec<(SshConnectionKey, Vec<SshParsedJob>)>,
    options: &SshFileCheckOptions,
) -> Vec<LinkCheckResults> {
    if options.max_connections == 1 {
        return groups
            .into_iter()
            .map(|(connection, jobs)| check_ssh_group(&connection, jobs, options))
            .collect();
    }

    match rayon::ThreadPoolBuilder::new()
        .num_threads(options.max_connections)
        .build()
    {
        Ok(pool) => pool.install(|| {
            groups
                .into_par_iter()
                .map(|(connection, jobs)| check_ssh_group(&connection, jobs, options))
                .collect()
        }),
        Err(_) => groups
            .into_iter()
            .map(|(connection, jobs)| check_ssh_group(&connection, jobs, options))
            .collect(),
    }
}

#[cfg(feature = "ssh")]
fn check_ssh_group(
    connection: &SshConnectionKey,
    jobs: Vec<SshParsedJob>,
    options: &SshFileCheckOptions,
) -> LinkCheckResults {
    match tokio::runtime::Builder::new_current_thread()
        .enable_io()
        .enable_time()
        .build()
    {
        Ok(runtime) => runtime.block_on(check_ssh_group_async(connection, jobs, options)),
        Err(error) => ssh_error_results(
            jobs,
            &SshCheckFailure::new(
                SshErrorKind::Network,
                format!("failed to initialize SSH runtime: {error}"),
            ),
        ),
    }
}

#[cfg(feature = "ssh")]
async fn check_ssh_group_async(
    connection: &SshConnectionKey,
    jobs: Vec<SshParsedJob>,
    options: &SshFileCheckOptions,
) -> LinkCheckResults {
    let session = match connect_ssh_client(connection, options).await {
        Ok(session) => session,
        Err(failure) => return ssh_error_results(jobs, &failure),
    };
    let sftp = match open_sftp_session(&session, options).await {
        Ok(sftp) => sftp,
        Err(failure) => return ssh_error_results(jobs, &failure),
    };

    let mut results = LinkCheckResults::default();
    for parsed in jobs {
        match check_ssh_file_target(&sftp, &parsed.target, options).await {
            Ok(true) => {}
            Ok(false) => results.broken.push(link_check_broken(parsed.job)),
            Err(failure) => {
                results
                    .errors
                    .push(link_check_error(parsed.job, failure.kind, failure.message))
            }
        }
    }
    let _ = sftp.close().await;
    let _ = session
        .disconnect(russh::Disconnect::ByApplication, "pkms check complete", "")
        .await;
    results.sort();
    results
}

#[cfg(feature = "ssh")]
fn ssh_error_results(jobs: Vec<SshParsedJob>, failure: &SshCheckFailure) -> LinkCheckResults {
    let mut results = LinkCheckResults {
        broken: Vec::new(),
        errors: jobs
            .into_iter()
            .map(|parsed| link_check_error(parsed.job, failure.kind, failure.message.clone()))
            .collect(),
    };
    results.sort();
    results
}

#[cfg(feature = "ssh")]
#[derive(Debug)]
enum KnownHostCheckError {
    Russh(russh::Error),
    Failure(SshCheckFailure),
}

#[cfg(feature = "ssh")]
impl From<russh::Error> for KnownHostCheckError {
    fn from(error: russh::Error) -> Self {
        Self::Russh(error)
    }
}

#[cfg(feature = "ssh")]
#[derive(Debug)]
struct KnownHostsHandler {
    host: String,
    port: u16,
    known_hosts: PathBuf,
}

#[cfg(feature = "ssh")]
impl russh::client::Handler for KnownHostsHandler {
    type Error = KnownHostCheckError;

    async fn check_server_key(
        &mut self,
        server_public_key: &russh::keys::PublicKey,
    ) -> Result<bool, Self::Error> {
        match russh::keys::known_hosts::check_known_hosts_path(
            &self.host,
            self.port,
            server_public_key,
            &self.known_hosts,
        ) {
            Ok(true) => Ok(true),
            Ok(false) => Err(KnownHostCheckError::Failure(SshCheckFailure::new(
                SshErrorKind::HostKey,
                format!(
                    "host {}:{} is not present in {}",
                    self.host,
                    self.port,
                    self.known_hosts.display()
                ),
            ))),
            Err(error) => Err(KnownHostCheckError::Failure(SshCheckFailure::new(
                SshErrorKind::HostKey,
                format!(
                    "failed to check known host entry for {}:{} in {}: {error}",
                    self.host,
                    self.port,
                    self.known_hosts.display()
                ),
            ))),
        }
    }
}

#[cfg(feature = "ssh")]
fn known_host_connect_failure(
    context: impl AsRef<str>,
    error: KnownHostCheckError,
) -> SshCheckFailure {
    match error {
        KnownHostCheckError::Failure(failure) => failure,
        KnownHostCheckError::Russh(error) => russh_operational_failure(context, &error),
    }
}

#[cfg(feature = "ssh")]
async fn connect_ssh_client(
    connection: &SshConnectionKey,
    options: &SshFileCheckOptions,
) -> Result<russh::client::Handle<KnownHostsHandler>, SshCheckFailure> {
    let config = russh::client::Config {
        inactivity_timeout: Some(operation_timeout(options)),
        ..Default::default()
    };
    let handler = KnownHostsHandler {
        host: connection.host.clone(),
        port: connection.port,
        known_hosts: options.known_hosts.clone(),
    };

    let mut session = tokio::time::timeout(
        options.connect_timeout,
        russh::client::connect(
            Arc::new(config),
            (connection.host.as_str(), connection.port),
            handler,
        ),
    )
    .await
    .map_err(|_| timeout_failure("SSH connection timed out"))?
    .map_err(|error| known_host_connect_failure("SSH connection failed", error))?;

    authenticate_ssh_client(&mut session, connection, options).await?;
    Ok(session)
}

#[cfg(feature = "ssh")]
async fn authenticate_ssh_client(
    session: &mut russh::client::Handle<KnownHostsHandler>,
    connection: &SshConnectionKey,
    options: &SshFileCheckOptions,
) -> Result<(), SshCheckFailure> {
    let mut failures = Vec::new();
    let hash_alg = session
        .best_supported_rsa_hash()
        .await
        .map_err(|error| {
            russh_auth_failure("failed to inspect server signature algorithms", &error)
        })?
        .flatten();

    for identity_file in &options.identity_files {
        match authenticate_with_identity_file(session, connection, identity_file, options, hash_alg)
            .await
        {
            SshAuthAttempt::Success => return Ok(()),
            SshAuthAttempt::Failure(message) => failures.push(message),
        }
    }

    match authenticate_with_agent(session, connection, options, hash_alg).await {
        Ok(()) => return Ok(()),
        Err(failure) => failures.push(failure.message),
    }

    Err(ssh_auth_failure(failures))
}

#[cfg(feature = "ssh")]
enum SshAuthAttempt {
    Success,
    Failure(String),
}

#[cfg(feature = "ssh")]
async fn authenticate_with_identity_file(
    session: &mut russh::client::Handle<KnownHostsHandler>,
    connection: &SshConnectionKey,
    identity_file: &Path,
    options: &SshFileCheckOptions,
    hash_alg: Option<russh::keys::HashAlg>,
) -> SshAuthAttempt {
    let key_pair = match russh::keys::load_secret_key(identity_file, None) {
        Ok(key_pair) => key_pair,
        Err(error) => {
            return SshAuthAttempt::Failure(format!(
                "identity file {} failed: {error}",
                identity_file.display()
            ));
        }
    };

    let key = russh::keys::PrivateKeyWithHashAlg::new(Arc::new(key_pair), hash_alg);
    match tokio::time::timeout(
        operation_timeout(options),
        session.authenticate_publickey(connection.user.clone(), key),
    )
    .await
    {
        Err(_) => SshAuthAttempt::Failure(format!(
            "identity file {} timed out",
            identity_file.display()
        )),
        Ok(Ok(result)) if result.success() => SshAuthAttempt::Success,
        Ok(Ok(_)) => SshAuthAttempt::Failure(format!(
            "identity file {} did not authenticate",
            identity_file.display()
        )),
        Ok(Err(error)) => SshAuthAttempt::Failure(format!(
            "identity file {} failed: {error}",
            identity_file.display()
        )),
    }
}

#[cfg(feature = "ssh")]
fn ssh_auth_failure(mut failures: Vec<String>) -> SshCheckFailure {
    if failures.is_empty() {
        failures.push(
            "no SSH authentication method succeeded; load a key into the SSH agent or use a standard passwordless identity file"
                .to_string(),
        );
    }

    SshCheckFailure::new(SshErrorKind::Auth, failures.join("; "))
}

#[cfg(feature = "ssh")]
#[cfg(unix)]
async fn authenticate_with_agent(
    session: &mut russh::client::Handle<KnownHostsHandler>,
    connection: &SshConnectionKey,
    options: &SshFileCheckOptions,
    hash_alg: Option<russh::keys::HashAlg>,
) -> Result<(), SshCheckFailure> {
    let Some(agent_socket) = options.agent_socket.as_deref() else {
        return Err(SshCheckFailure::new(
            SshErrorKind::Auth,
            "SSH agent socket is not configured",
        ));
    };
    let mut agent = russh::keys::agent::client::AgentClient::connect_uds(agent_socket)
        .await
        .map_err(|error| {
            SshCheckFailure::new(SshErrorKind::Auth, format!("SSH agent failed: {error}"))
        })?;
    let identities = agent.request_identities().await.map_err(|error| {
        SshCheckFailure::new(SshErrorKind::Auth, format!("SSH agent failed: {error}"))
    })?;

    if identities.is_empty() {
        return Err(SshCheckFailure::new(
            SshErrorKind::Auth,
            "SSH agent had no identities",
        ));
    }

    let mut failures = Vec::new();
    for identity in identities {
        let comment = agent_identity_comment(&identity).to_string();
        let auth = match identity {
            russh::keys::agent::AgentIdentity::PublicKey { key, .. } => tokio::time::timeout(
                operation_timeout(options),
                session.authenticate_publickey_with(
                    connection.user.clone(),
                    key,
                    hash_alg,
                    &mut agent,
                ),
            )
            .await
            .map_err(|_| timeout_failure("SSH agent authentication timed out"))?
            .map_err(|error| {
                SshCheckFailure::new(SshErrorKind::Auth, format!("SSH agent failed: {error}"))
            })?,
            russh::keys::agent::AgentIdentity::Certificate { certificate, .. } => {
                tokio::time::timeout(
                    operation_timeout(options),
                    session.authenticate_certificate_with(
                        connection.user.clone(),
                        certificate,
                        hash_alg,
                        &mut agent,
                    ),
                )
                .await
                .map_err(|_| timeout_failure("SSH agent authentication timed out"))?
                .map_err(|error| {
                    SshCheckFailure::new(SshErrorKind::Auth, format!("SSH agent failed: {error}"))
                })?
            }
        };
        if auth.success() {
            return Ok(());
        }
        if comment.is_empty() {
            failures.push("SSH agent identity did not authenticate".to_string());
        } else {
            failures.push(format!("SSH agent identity {comment} did not authenticate"));
        }
    }

    Err(SshCheckFailure::new(
        SshErrorKind::Auth,
        failures.join("; "),
    ))
}

#[cfg(feature = "ssh")]
#[cfg(not(unix))]
async fn authenticate_with_agent(
    _session: &mut russh::client::Handle<KnownHostsHandler>,
    _connection: &SshConnectionKey,
    _options: &SshFileCheckOptions,
    _hash_alg: Option<russh::keys::HashAlg>,
) -> Result<(), SshCheckFailure> {
    Err(SshCheckFailure::new(
        SshErrorKind::Auth,
        "SSH agent authentication is not supported on this platform",
    ))
}

#[cfg(feature = "ssh")]
fn agent_identity_comment(identity: &russh::keys::agent::AgentIdentity) -> &str {
    match identity {
        russh::keys::agent::AgentIdentity::PublicKey { comment, .. }
        | russh::keys::agent::AgentIdentity::Certificate { comment, .. } => comment,
    }
}

#[cfg(feature = "ssh")]
async fn open_sftp_session(
    session: &russh::client::Handle<KnownHostsHandler>,
    options: &SshFileCheckOptions,
) -> Result<russh_sftp::client::SftpSession, SshCheckFailure> {
    let channel = tokio::time::timeout(operation_timeout(options), session.channel_open_session())
        .await
        .map_err(|_| timeout_failure("timed out opening SSH session channel"))?
        .map_err(|error| russh_operational_failure("failed to open SSH session channel", &error))?;
    tokio::time::timeout(
        operation_timeout(options),
        channel.request_subsystem(true, "sftp"),
    )
    .await
    .map_err(|_| timeout_failure("timed out requesting SFTP subsystem"))?
    .map_err(|error| russh_operational_failure("failed to request SFTP subsystem", &error))?;

    let sftp = tokio::time::timeout(
        operation_timeout(options),
        russh_sftp::client::SftpSession::new(channel.into_stream()),
    )
    .await
    .map_err(|_| timeout_failure("timed out initializing SFTP"))?
    .map_err(|error| sftp_operational_failure("failed to initialize SFTP", &error))?;
    sftp.set_timeout(operation_timeout_secs(options));
    Ok(sftp)
}

#[cfg(feature = "ssh")]
async fn check_ssh_file_target(
    sftp: &russh_sftp::client::SftpSession,
    target: &SshFileTarget,
    options: &SshFileCheckOptions,
) -> Result<bool, SshCheckFailure> {
    let exists = tokio::time::timeout(
        operation_timeout(options),
        sftp.try_exists(target.path.clone()),
    )
    .await
    .map_err(|_| timeout_failure(format!("timed out checking remote path {}", target.path)))?
    .map_err(|error| {
        sftp_operational_failure(
            format!("failed to stat remote path {}", target.path),
            &error,
        )
    })?;
    if !exists {
        return Ok(false);
    }

    let Some(line_spec) = &target.line_spec else {
        return Ok(true);
    };
    if line_spec.is_empty() {
        return Ok(true);
    }

    let bytes =
        match tokio::time::timeout(operation_timeout(options), sftp.read(target.path.clone()))
            .await
            .map_err(|_| {
                timeout_failure(format!("timed out reading remote path {}", target.path))
            })? {
            Ok(bytes) => bytes,
            Err(error) if is_sftp_missing(&error) => return Ok(false),
            Err(error) => {
                return Err(sftp_operational_failure(
                    format!("failed to read remote path {}", target.path),
                    &error,
                ));
            }
        };
    let content = std::str::from_utf8(&bytes).map_err(|error| {
        SshCheckFailure::new(
            LinkCheckErrorKind::Unsupported,
            format!("failed to read remote file as UTF-8: {error}"),
        )
    })?;

    Ok(content.lines().any(|line| line.contains(line_spec)))
}

#[cfg(feature = "ssh")]
fn is_sftp_missing(error: &russh_sftp::client::error::Error) -> bool {
    matches!(
        error,
        russh_sftp::client::error::Error::Status(status)
            if status.status_code == russh_sftp::protocol::StatusCode::NoSuchFile
    )
}

#[cfg(feature = "ssh")]
fn operation_timeout(options: &SshFileCheckOptions) -> Duration {
    Duration::from_millis(u64::from(options.operation_timeout_ms))
}

#[cfg(feature = "ssh")]
fn operation_timeout_secs(options: &SshFileCheckOptions) -> u64 {
    u64::from(options.operation_timeout_ms)
        .div_ceil(1_000)
        .max(1)
}

#[cfg(feature = "ssh")]
fn timeout_failure(message: impl Into<String>) -> SshCheckFailure {
    SshCheckFailure::new(SshErrorKind::Timeout, message)
}

#[cfg(feature = "ssh")]
fn russh_auth_failure(context: impl AsRef<str>, error: &russh::Error) -> SshCheckFailure {
    let kind = if is_russh_timeout(error) {
        SshErrorKind::Timeout
    } else {
        SshErrorKind::Auth
    };
    SshCheckFailure::new(kind, format!("{}: {error}", context.as_ref()))
}

#[cfg(feature = "ssh")]
fn russh_operational_failure(context: impl AsRef<str>, error: &russh::Error) -> SshCheckFailure {
    let kind = if is_russh_timeout(error) {
        SshErrorKind::Timeout
    } else if is_russh_hostkey_error(error) {
        SshErrorKind::HostKey
    } else {
        SshErrorKind::Network
    };
    SshCheckFailure::new(kind, format!("{}: {error}", context.as_ref()))
}

#[cfg(feature = "ssh")]
fn is_russh_timeout(error: &russh::Error) -> bool {
    matches!(
        error,
        russh::Error::ConnectionTimeout
            | russh::Error::KeepaliveTimeout
            | russh::Error::InactivityTimeout
            | russh::Error::Elapsed(_)
    )
}

#[cfg(feature = "ssh")]
fn is_russh_hostkey_error(error: &russh::Error) -> bool {
    matches!(
        error,
        russh::Error::UnknownKey
            | russh::Error::WrongServerSig
            | russh::Error::KeyChanged { .. }
            | russh::Error::Keys(russh::keys::Error::KeyChanged { .. })
    )
}

#[cfg(feature = "ssh")]
fn sftp_operational_failure(
    context: impl AsRef<str>,
    error: &russh_sftp::client::error::Error,
) -> SshCheckFailure {
    let kind = if matches!(error, russh_sftp::client::error::Error::Timeout) {
        SshErrorKind::Timeout
    } else {
        SshErrorKind::Network
    };
    SshCheckFailure::new(kind, format!("{}: {error}", context.as_ref()))
}

fn normalized_file_target(target: &str) -> &str {
    target.strip_prefix("org:").unwrap_or(target)
}

fn parse_host_and_port(host_and_port: &str) -> Result<(String, u16), SshFileTargetParseError> {
    let (host, port) = if let Some((host, raw_port)) = host_and_port.rsplit_once('#') {
        if raw_port.is_empty() {
            return Err(ssh_parse_error(
                SshFileTargetParseErrorKind::InvalidPort,
                "SSH file link has an empty port",
            ));
        }
        let port = raw_port.parse::<u16>().map_err(|_| {
            ssh_parse_error(
                SshFileTargetParseErrorKind::InvalidPort,
                "SSH file link has an invalid port",
            )
        })?;
        (host, port)
    } else {
        (host_and_port, 22)
    };

    if host.is_empty() {
        return Err(ssh_parse_error(
            SshFileTargetParseErrorKind::EmptyHost,
            "SSH file link has an empty host",
        ));
    }
    Ok((host.to_string(), port))
}

fn ssh_parse_error(
    kind: SshFileTargetParseErrorKind,
    message: impl Into<String>,
) -> SshFileTargetParseError {
    SshFileTargetParseError {
        kind,
        message: message.into(),
    }
}
