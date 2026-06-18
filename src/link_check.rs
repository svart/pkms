use crate::config::SshConfig;
use crate::graph::file_link_target_exists;
use crate::util::attachment_target_exists;
use rayon::prelude::*;
use std::cmp::Ordering;
#[cfg(feature = "ssh")]
use std::collections::BTreeMap;
use std::env;
use std::path::{Path, PathBuf};
#[cfg(feature = "ssh")]
use std::sync::Arc;
use std::time::Duration;

const DEFAULT_SSH_CONNECT_TIMEOUT_MS: u64 = 5_000;
const DEFAULT_SSH_OPERATION_TIMEOUT_MS: u32 = 5_000;
const DEFAULT_SSH_MAX_CONNECTIONS: usize = 4;

#[cfg(feature = "ssh")]
const SSH_ERROR_AUTH: &str = "auth";
#[cfg(feature = "ssh")]
const SSH_ERROR_HOSTKEY: &str = "hostkey";
#[cfg(feature = "ssh")]
const SSH_ERROR_NETWORK: &str = "network";
#[cfg(feature = "ssh")]
const SSH_ERROR_TIMEOUT: &str = "timeout";
const SSH_ERROR_UNSUPPORTED: &str = "unsupported";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LinkCheckKind {
    File,
    Attachment,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LinkCheckBackend {
    Local,
    Ssh,
}

impl LinkCheckBackend {
    pub fn as_str(self) -> &'static str {
        match self {
            LinkCheckBackend::Local => "local",
            LinkCheckBackend::Ssh => "ssh",
        }
    }
}

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
    pub identity_file: Option<PathBuf>,
    pub known_hosts: PathBuf,
    pub connect_timeout: Duration,
    pub operation_timeout_ms: u32,
    pub max_connections: usize,
    pub agent: bool,
}

impl SshFileCheckOptions {
    pub fn from_config(config: Option<&SshConfig>) -> Self {
        let identity_file = config
            .and_then(|config| config.identity_file.as_ref())
            .map(|path| expand_home_path(path));
        let known_hosts = config
            .and_then(|config| config.known_hosts.as_ref())
            .map(|path| expand_home_path(path))
            .unwrap_or_else(default_known_hosts_path);
        let connect_timeout_ms = config
            .and_then(|config| config.connect_timeout_ms)
            .unwrap_or(DEFAULT_SSH_CONNECT_TIMEOUT_MS)
            .max(1);
        let operation_timeout_ms = config
            .and_then(|config| config.operation_timeout_ms)
            .unwrap_or(DEFAULT_SSH_OPERATION_TIMEOUT_MS)
            .max(1);
        let max_connections = config
            .and_then(|config| config.max_connections)
            .unwrap_or(DEFAULT_SSH_MAX_CONNECTIONS)
            .max(1);
        let agent = config.and_then(|config| config.agent).unwrap_or(true);

        Self {
            default_user: default_ssh_user(),
            identity_file,
            known_hosts,
            connect_timeout: Duration::from_millis(connect_timeout_ms),
            operation_timeout_ms,
            max_connections,
            agent,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkCheckJob {
    pub kind: LinkCheckKind,
    pub backend: LinkCheckBackend,
    pub source_uuid: String,
    pub source_title: String,
    pub source_path: PathBuf,
    pub target: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkCheckOutcome {
    Ok,
    Broken(LinkCheckBrokenTarget),
    Error(LinkCheckErrorTarget),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkCheckBrokenTarget {
    pub kind: LinkCheckKind,
    pub source_uuid: String,
    pub source_title: String,
    pub source_path: PathBuf,
    pub target: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkCheckErrorTarget {
    pub kind: LinkCheckKind,
    pub backend: LinkCheckBackend,
    pub source_uuid: String,
    pub source_title: String,
    pub source_path: PathBuf,
    pub target: String,
    pub error_kind: String,
    pub message: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LinkCheckResults {
    pub broken: Vec<LinkCheckBrokenTarget>,
    pub errors: Vec<LinkCheckErrorTarget>,
}

impl LinkCheckResults {
    pub fn extend(&mut self, mut other: Self) {
        self.broken.append(&mut other.broken);
        self.errors.append(&mut other.errors);
        self.sort();
    }

    pub fn sort(&mut self) {
        sort_broken_targets(&mut self.broken);
        sort_link_check_errors(&mut self.errors);
    }
}

impl LinkCheckJob {
    pub fn file(
        source_uuid: impl Into<String>,
        source_title: impl Into<String>,
        source_path: impl Into<PathBuf>,
        target: impl Into<String>,
    ) -> Self {
        Self {
            kind: LinkCheckKind::File,
            backend: LinkCheckBackend::Local,
            source_uuid: source_uuid.into(),
            source_title: source_title.into(),
            source_path: source_path.into(),
            target: target.into(),
        }
    }

    pub fn attachment(
        source_uuid: impl Into<String>,
        source_title: impl Into<String>,
        source_path: impl Into<PathBuf>,
        target: impl Into<String>,
    ) -> Self {
        Self {
            kind: LinkCheckKind::Attachment,
            backend: LinkCheckBackend::Local,
            source_uuid: source_uuid.into(),
            source_title: source_title.into(),
            source_path: source_path.into(),
            target: target.into(),
        }
    }

    pub fn ssh_file(
        source_uuid: impl Into<String>,
        source_title: impl Into<String>,
        source_path: impl Into<PathBuf>,
        target: impl Into<String>,
    ) -> Self {
        Self {
            kind: LinkCheckKind::File,
            backend: LinkCheckBackend::Ssh,
            source_uuid: source_uuid.into(),
            source_title: source_title.into(),
            source_path: source_path.into(),
            target: target.into(),
        }
    }
}

pub fn sort_link_check_jobs(jobs: &mut [LinkCheckJob]) {
    jobs.sort_by(compare_jobs);
}

pub fn sort_broken_targets(targets: &mut [LinkCheckBrokenTarget]) {
    targets.sort_by(compare_broken_targets);
}

pub fn sort_link_check_errors(errors: &mut [LinkCheckErrorTarget]) {
    errors.sort_by(compare_error_targets);
}

pub fn is_ssh_file_target(target: &str) -> bool {
    normalized_file_target(target).starts_with("/ssh:")
}

pub fn split_file_link_line_spec(target: &str) -> (&str, Option<&str>) {
    target
        .split_once("::")
        .map_or((target, None), |(path, line_spec)| (path, Some(line_spec)))
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

pub fn local_file_link_target_exists(target: &str, source_path: &Path, db_root: &Path) -> bool {
    if is_ssh_file_target(target) {
        return true;
    }
    file_link_target_exists(target, source_path, db_root)
}

pub fn check_local_link_job(job: LinkCheckJob, db_root: &Path) -> LinkCheckOutcome {
    let exists = match job.backend {
        LinkCheckBackend::Local => match job.kind {
            LinkCheckKind::File => {
                local_file_link_target_exists(&job.target, &job.source_path, db_root)
            }
            LinkCheckKind::Attachment => {
                attachment_target_exists(db_root, &job.source_uuid, &job.target)
            }
        },
        LinkCheckBackend::Ssh => {
            return LinkCheckOutcome::Error(LinkCheckErrorTarget {
                kind: job.kind,
                backend: job.backend,
                source_uuid: job.source_uuid,
                source_title: job.source_title,
                source_path: job.source_path,
                target: job.target,
                error_kind: "unsupported_backend".to_string(),
                message: "SSH link jobs cannot be checked by the local checker".to_string(),
            });
        }
    };

    if exists {
        LinkCheckOutcome::Ok
    } else {
        LinkCheckOutcome::Broken(LinkCheckBrokenTarget {
            kind: job.kind,
            source_uuid: job.source_uuid,
            source_title: job.source_title,
            source_path: job.source_path,
            target: job.target,
        })
    }
}

pub fn run_local_link_checks(jobs: Vec<LinkCheckJob>, db_root: &Path) -> LinkCheckResults {
    let outcomes: Vec<LinkCheckOutcome> = jobs
        .into_par_iter()
        .map(|job| check_local_link_job(job, db_root))
        .collect();
    let mut results = LinkCheckResults::default();
    for outcome in outcomes {
        match outcome {
            LinkCheckOutcome::Ok => {}
            LinkCheckOutcome::Broken(target) => results.broken.push(target),
            LinkCheckOutcome::Error(target) => results.errors.push(target),
        }
    }
    results.sort();
    results
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
                    SSH_ERROR_UNSUPPORTED,
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
        match parse_ssh_file_target(&job.target, &options.default_user) {
            Ok(Some(target)) => grouped
                .entry(target.connection.clone())
                .or_default()
                .push(SshParsedJob { job, target }),
            Ok(None) => results.errors.push(link_check_error(
                job,
                SSH_ERROR_UNSUPPORTED,
                "remote file-link check received a non-SSH file target",
            )),
            Err(error) => {
                results
                    .errors
                    .push(link_check_error(job, SSH_ERROR_UNSUPPORTED, error.message))
            }
        }
    }

    let groups: Vec<_> = grouped.into_iter().collect();
    for group_result in run_ssh_groups(groups, options) {
        results.extend(group_result);
    }
    results.sort();
    results
}

fn compare_jobs(a: &LinkCheckJob, b: &LinkCheckJob) -> Ordering {
    job_sort_key(a).cmp(&job_sort_key(b))
}

fn compare_broken_targets(a: &LinkCheckBrokenTarget, b: &LinkCheckBrokenTarget) -> Ordering {
    broken_target_sort_key(a).cmp(&broken_target_sort_key(b))
}

fn compare_error_targets(a: &LinkCheckErrorTarget, b: &LinkCheckErrorTarget) -> Ordering {
    error_target_sort_key(a).cmp(&error_target_sort_key(b))
}

fn job_sort_key(job: &LinkCheckJob) -> (LinkCheckKind, &str, &str, &Path, &str) {
    (
        job.kind,
        job.source_uuid.as_str(),
        job.source_title.as_str(),
        job.source_path.as_path(),
        job.target.as_str(),
    )
}

fn broken_target_sort_key(
    target: &LinkCheckBrokenTarget,
) -> (LinkCheckKind, &str, &str, &Path, &str) {
    (
        target.kind,
        target.source_uuid.as_str(),
        target.source_title.as_str(),
        target.source_path.as_path(),
        target.target.as_str(),
    )
}

fn error_target_sort_key(
    target: &LinkCheckErrorTarget,
) -> (
    LinkCheckKind,
    LinkCheckBackend,
    &str,
    &str,
    &Path,
    &str,
    &str,
) {
    (
        target.kind,
        target.backend,
        target.source_uuid.as_str(),
        target.source_title.as_str(),
        target.source_path.as_path(),
        target.target.as_str(),
        target.error_kind.as_str(),
    )
}

fn link_check_error(
    job: LinkCheckJob,
    error_kind: impl Into<String>,
    message: impl Into<String>,
) -> LinkCheckErrorTarget {
    LinkCheckErrorTarget {
        kind: job.kind,
        backend: job.backend,
        source_uuid: job.source_uuid,
        source_title: job.source_title,
        source_path: job.source_path,
        target: job.target,
        error_kind: error_kind.into(),
        message: message.into(),
    }
}

#[cfg(feature = "ssh")]
fn link_check_broken(job: LinkCheckJob) -> LinkCheckBrokenTarget {
    LinkCheckBrokenTarget {
        kind: job.kind,
        source_uuid: job.source_uuid,
        source_title: job.source_title,
        source_path: job.source_path,
        target: job.target,
    }
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
    kind: &'static str,
    message: String,
}

#[cfg(feature = "ssh")]
impl SshCheckFailure {
    fn new(kind: &'static str, message: impl Into<String>) -> Self {
        Self {
            kind,
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
            SshCheckFailure::new(
                SSH_ERROR_NETWORK,
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
    let mut session = match connect_ssh_client(connection, options).await {
        Ok(session) => session,
        Err(failure) => return ssh_error_results(jobs, failure),
    };
    let sftp = match open_sftp_session(&mut session, options).await {
        Ok(sftp) => sftp,
        Err(failure) => return ssh_error_results(jobs, failure),
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
fn ssh_error_results(jobs: Vec<SshParsedJob>, failure: SshCheckFailure) -> LinkCheckResults {
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
                SSH_ERROR_HOSTKEY,
                format!(
                    "host {}:{} is not present in {}",
                    self.host,
                    self.port,
                    self.known_hosts.display()
                ),
            ))),
            Err(error) => Err(KnownHostCheckError::Failure(SshCheckFailure::new(
                SSH_ERROR_HOSTKEY,
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
        KnownHostCheckError::Russh(error) => russh_operational_failure(context, error),
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
            russh_auth_failure("failed to inspect server signature algorithms", error)
        })?
        .flatten();

    if let Some(identity_file) = &options.identity_file {
        match russh::keys::load_secret_key(identity_file, None) {
            Ok(key_pair) => {
                let key = russh::keys::PrivateKeyWithHashAlg::new(Arc::new(key_pair), hash_alg);
                match tokio::time::timeout(
                    operation_timeout(options),
                    session.authenticate_publickey(connection.user.clone(), key),
                )
                .await
                {
                    Err(_) => failures.push(format!(
                        "identity file {} timed out",
                        identity_file.display()
                    )),
                    Ok(Ok(result)) if result.success() => return Ok(()),
                    Ok(Ok(_)) => failures.push(format!(
                        "identity file {} did not authenticate",
                        identity_file.display()
                    )),
                    Ok(Err(error)) => failures.push(format!(
                        "identity file {} failed: {error}",
                        identity_file.display()
                    )),
                }
            }
            Err(error) => failures.push(format!(
                "identity file {} failed: {error}",
                identity_file.display()
            )),
        }
    }

    if options.agent {
        match authenticate_with_agent(session, connection, options, hash_alg).await {
            Ok(()) => return Ok(()),
            Err(failure) => failures.push(failure.message),
        };
    }

    if failures.is_empty() {
        failures.push(
            "no SSH authentication method configured; set [ssh].identity_file or agent = true"
                .to_string(),
        );
    }

    Err(SshCheckFailure::new(SSH_ERROR_AUTH, failures.join("; ")))
}

#[cfg(feature = "ssh")]
#[cfg(unix)]
async fn authenticate_with_agent(
    session: &mut russh::client::Handle<KnownHostsHandler>,
    connection: &SshConnectionKey,
    options: &SshFileCheckOptions,
    hash_alg: Option<russh::keys::HashAlg>,
) -> Result<(), SshCheckFailure> {
    let mut agent = russh::keys::agent::client::AgentClient::connect_env()
        .await
        .map_err(|error| {
            SshCheckFailure::new(SSH_ERROR_AUTH, format!("SSH agent failed: {error}"))
        })?;
    let identities = agent.request_identities().await.map_err(|error| {
        SshCheckFailure::new(SSH_ERROR_AUTH, format!("SSH agent failed: {error}"))
    })?;

    if identities.is_empty() {
        return Err(SshCheckFailure::new(
            SSH_ERROR_AUTH,
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
                SshCheckFailure::new(SSH_ERROR_AUTH, format!("SSH agent failed: {error}"))
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
                    SshCheckFailure::new(SSH_ERROR_AUTH, format!("SSH agent failed: {error}"))
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

    Err(SshCheckFailure::new(SSH_ERROR_AUTH, failures.join("; ")))
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
        SSH_ERROR_AUTH,
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
    session: &mut russh::client::Handle<KnownHostsHandler>,
    options: &SshFileCheckOptions,
) -> Result<russh_sftp::client::SftpSession, SshCheckFailure> {
    let channel = tokio::time::timeout(operation_timeout(options), session.channel_open_session())
        .await
        .map_err(|_| timeout_failure("timed out opening SSH session channel"))?
        .map_err(|error| russh_operational_failure("failed to open SSH session channel", error))?;
    tokio::time::timeout(
        operation_timeout(options),
        channel.request_subsystem(true, "sftp"),
    )
    .await
    .map_err(|_| timeout_failure("timed out requesting SFTP subsystem"))?
    .map_err(|error| russh_operational_failure("failed to request SFTP subsystem", error))?;

    let sftp = tokio::time::timeout(
        operation_timeout(options),
        russh_sftp::client::SftpSession::new(channel.into_stream()),
    )
    .await
    .map_err(|_| timeout_failure("timed out initializing SFTP"))?
    .map_err(|error| sftp_operational_failure("failed to initialize SFTP", error))?;
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
        sftp_operational_failure(format!("failed to stat remote path {}", target.path), error)
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
                    error,
                ));
            }
        };
    let content = std::str::from_utf8(&bytes).map_err(|error| {
        SshCheckFailure::new(
            SSH_ERROR_UNSUPPORTED,
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
    SshCheckFailure::new(SSH_ERROR_TIMEOUT, message)
}

#[cfg(feature = "ssh")]
fn russh_auth_failure(context: impl AsRef<str>, error: russh::Error) -> SshCheckFailure {
    let kind = if is_russh_timeout(&error) {
        SSH_ERROR_TIMEOUT
    } else {
        SSH_ERROR_AUTH
    };
    SshCheckFailure::new(kind, format!("{}: {error}", context.as_ref()))
}

#[cfg(feature = "ssh")]
fn russh_operational_failure(context: impl AsRef<str>, error: russh::Error) -> SshCheckFailure {
    let kind = if is_russh_timeout(&error) {
        SSH_ERROR_TIMEOUT
    } else if is_russh_hostkey_error(&error) {
        SSH_ERROR_HOSTKEY
    } else {
        SSH_ERROR_NETWORK
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
    error: russh_sftp::client::error::Error,
) -> SshCheckFailure {
    let kind = if matches!(error, russh_sftp::client::error::Error::Timeout) {
        SSH_ERROR_TIMEOUT
    } else {
        SSH_ERROR_NETWORK
    };
    SshCheckFailure::new(kind, format!("{}: {error}", context.as_ref()))
}

fn normalized_file_target(target: &str) -> &str {
    target.strip_prefix("org:").unwrap_or(target)
}

fn default_ssh_user() -> String {
    env::var("USER")
        .or_else(|_| env::var("LOGNAME"))
        .unwrap_or_default()
}

fn default_known_hosts_path() -> PathBuf {
    dirs::home_dir()
        .map(|home| home.join(".ssh").join("known_hosts"))
        .unwrap_or_else(|| PathBuf::from("~/.ssh/known_hosts"))
}

fn expand_home_path(path: &Path) -> PathBuf {
    let raw = path.to_string_lossy();
    if raw == "~" {
        dirs::home_dir().unwrap_or_else(|| path.to_path_buf())
    } else if let Some(rest) = raw.strip_prefix("~/") {
        dirs::home_dir()
            .map(|home| home.join(rest))
            .unwrap_or_else(|| path.to_path_buf())
    } else {
        path.to_path_buf()
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_jobs_use_existing_line_spec_semantics() {
        let dir = tempfile::tempdir().unwrap();
        let db_root = dir.path();
        let source_path = db_root.join("source.org");
        let target_path = db_root.join("target.org");
        std::fs::write(&source_path, "#+title: Source\n").unwrap();
        std::fs::write(&target_path, "first line\nneedle line\n").unwrap();

        let ok = check_local_link_job(
            LinkCheckJob::file("source", "Source", &source_path, "target.org::needle"),
            db_root,
        );
        let broken = check_local_link_job(
            LinkCheckJob::file("source", "Source", &source_path, "target.org::missing"),
            db_root,
        );

        assert_eq!(ok, LinkCheckOutcome::Ok);
        assert_eq!(
            broken,
            LinkCheckOutcome::Broken(LinkCheckBrokenTarget {
                kind: LinkCheckKind::File,
                source_uuid: "source".to_string(),
                source_title: "Source".to_string(),
                source_path,
                target: "target.org::missing".to_string(),
            })
        );
    }

    #[test]
    fn attachment_jobs_use_hashed_org_attach_layout() {
        let dir = tempfile::tempdir().unwrap();
        let db_root = dir.path();
        let source_path = db_root.join("source.org");
        let uuid = "aaaaaaaa-aaaa-4aaa-aaaa-bbbbbbbbbbbb";
        let attach_dir = db_root.join(".attach").join(&uuid[..2]).join(&uuid[2..]);
        std::fs::create_dir_all(&attach_dir).unwrap();
        std::fs::write(attach_dir.join("image.png"), b"image").unwrap();

        let ok = check_local_link_job(
            LinkCheckJob::attachment(uuid, "Source", &source_path, "image.png"),
            db_root,
        );
        let broken = check_local_link_job(
            LinkCheckJob::attachment(uuid, "Source", &source_path, "missing.png"),
            db_root,
        );

        assert_eq!(ok, LinkCheckOutcome::Ok);
        assert_eq!(
            broken,
            LinkCheckOutcome::Broken(LinkCheckBrokenTarget {
                kind: LinkCheckKind::Attachment,
                source_uuid: uuid.to_string(),
                source_title: "Source".to_string(),
                source_path,
                target: "missing.png".to_string(),
            })
        );
    }

    #[test]
    fn parses_tramp_ssh_file_targets() {
        let target = parse_ssh_file_target(
            "/ssh:alice@example.org#2222:/var/log/app.log::needle",
            "local",
        )
        .unwrap()
        .unwrap();

        assert_eq!(
            target,
            SshFileTarget {
                connection: SshConnectionKey {
                    user: "alice".to_string(),
                    host: "example.org".to_string(),
                    port: 2222,
                },
                path: "/var/log/app.log".to_string(),
                line_spec: Some("needle".to_string()),
                raw_target: "/ssh:alice@example.org#2222:/var/log/app.log::needle".to_string(),
            }
        );
    }

    #[test]
    fn parses_ssh_file_targets_with_default_user_and_port() {
        let target = parse_ssh_file_target("org:/ssh:example.org:/tmp/file.txt", "local")
            .unwrap()
            .unwrap();

        assert_eq!(target.connection.user, "local");
        assert_eq!(target.connection.host, "example.org");
        assert_eq!(target.connection.port, 22);
        assert_eq!(target.path, "/tmp/file.txt");
        assert_eq!(target.line_spec, None);
    }

    #[test]
    fn rejects_unsupported_ssh_file_target_syntax() {
        let error =
            parse_ssh_file_target("/ssh:jump|example.org:/tmp/file.txt", "local").unwrap_err();

        assert_eq!(error.kind, SshFileTargetParseErrorKind::UnsupportedSyntax);
    }

    #[test]
    fn local_file_checks_skip_ssh_file_targets() {
        let dir = tempfile::tempdir().unwrap();
        let db_root = dir.path();
        let source_path = db_root.join("source.org");

        assert!(local_file_link_target_exists(
            "/ssh:example.org:/missing/file.txt::needle",
            &source_path,
            db_root
        ));
    }
}
