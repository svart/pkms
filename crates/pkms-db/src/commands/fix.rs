use anyhow::{Context, Result};
use pkms_org::attachments::resolve_attachment_path;
use pkms_org::discovery;
use pkms_org::parser::Link;
use pkms_org::{Graph, OrgConfig};
use regex::Regex;
use serde::Serialize;
use std::fmt::Write;
use std::path::{Component, Path, PathBuf};

#[derive(Serialize)]
pub struct UuidFixOutput {
    pub broken_uuid: String,
    pub replacement_uuid: String,
    pub replacement_title: String,
    pub files_affected: Vec<String>,
    pub total_replacements: usize,
    pub applied: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AttachMode {
    Move,
    Copy,
}

impl AttachMode {
    fn as_str(self) -> &'static str {
        match self {
            AttachMode::Move => "move",
            AttachMode::Copy => "copy",
        }
    }
}

#[derive(Serialize)]
pub struct AttachFixOutput {
    pub applied: bool,
    pub mode: String,
    pub total_repaired: usize,
    pub total_skipped: usize,
    pub repairs: Vec<AttachFixRepair>,
    pub skipped: Vec<AttachFixSkipped>,
}

#[derive(Serialize)]
pub struct AttachFixRepair {
    pub source_uuid: String,
    pub source_title: String,
    pub source_path: String,
    pub target_path: String,
    pub found_path: String,
    pub expected_path: String,
    pub applied: bool,
}

#[derive(Serialize)]
pub struct AttachFixSkipped {
    pub source_uuid: String,
    pub source_title: String,
    pub source_path: String,
    pub target_path: String,
    pub expected_path: String,
    pub reason: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub matches: Vec<String>,
}

#[derive(Debug, Clone)]
struct AttachmentCandidate {
    source_uuid: String,
    source_title: String,
    source_path: PathBuf,
    target_path: String,
    expected_path: PathBuf,
}

#[derive(Debug, Clone)]
struct AttachmentSearchRoot {
    path: PathBuf,
    canonical: PathBuf,
}

pub fn render_uuid_text(output: &UuidFixOutput) -> String {
    let mut text = String::new();

    if output.applied {
        let _ = writeln!(
            text,
            "Fixed {} broken link(s) in {} file(s):",
            output.total_replacements,
            output.files_affected.len()
        );
    } else {
        let _ = writeln!(
            text,
            "Would fix {} broken link(s) in {} file(s):",
            output.total_replacements,
            output.files_affected.len()
        );
        let _ = writeln!(text, "  Broken UUID: {}", output.broken_uuid);
        let _ = writeln!(
            text,
            "  Replace with: {} ({})",
            output.replacement_title, output.replacement_uuid
        );
        let _ = writeln!(text, "  (use --apply to apply)");
    }
    for f in &output.files_affected {
        let _ = writeln!(text, "  {f}");
    }

    text
}

pub fn render_attach_text(output: &AttachFixOutput) -> String {
    let mut text = String::new();

    if output.applied {
        let _ = writeln!(
            text,
            "{} {} attachment file(s); skipped {}:",
            attach_past_verb(&output.mode),
            output.total_repaired,
            output.total_skipped
        );
    } else {
        let _ = writeln!(
            text,
            "Would {} {} attachment file(s); skipped {}:",
            attach_present_verb(&output.mode),
            output.total_repaired,
            output.total_skipped
        );
        let _ = writeln!(text, "  (use --apply to apply)");
    }
    for repair in &output.repairs {
        let _ = writeln!(text, "  {} -> {}", repair.found_path, repair.expected_path);
    }
    for skipped in &output.skipped {
        let _ = writeln!(
            text,
            "  skipped {} in {}: {}",
            skipped.target_path, skipped.source_title, skipped.reason
        );
    }

    text
}

fn attach_present_verb(mode: &str) -> &str {
    match mode {
        "copy" => "copy",
        _ => "move",
    }
}

fn attach_past_verb(mode: &str) -> &str {
    match mode {
        "copy" => "Copied",
        _ => "Moved",
    }
}

fn find_and_replace_links(
    db_root: &std::path::Path,
    ignore_patterns: &[String],
    broken_str: &str,
    replacement_uuid: &str,
    apply: bool,
) -> Result<(Vec<String>, usize)> {
    let mut files_affected = Vec::new();
    let mut total_replacements = 0;

    let escaped = regex::escape(broken_str);
    let re = Regex::new(&format!(r"(id:){escaped}")).unwrap();

    for path in discovery::walk_org_files(db_root, ignore_patterns)? {
        let Ok(content) = std::fs::read_to_string(&path) else {
            continue;
        };

        let count = re.find_iter(&content).count();
        if count > 0 {
            files_affected.push(path.display().to_string());
            total_replacements += count;

            if apply {
                let new_content = re.replace_all(&content, |caps: &regex::Captures| {
                    format!("{}{}", &caps[1], replacement_uuid)
                });
                std::fs::write(&path, new_content.as_ref())?;
            }
        }
    }

    Ok((files_affected, total_replacements))
}

pub struct FixUuidOptions {
    pub broken_uuid: String,
    pub target_uuid: String,
    pub apply: bool,
}

pub struct FixAttachOptions {
    pub apply: bool,
    pub copy: bool,
}

pub fn execute_uuid(config: &OrgConfig, opts: &FixUuidOptions) -> Result<UuidFixOutput> {
    let graph = crate::load_graph(config)?;
    let db_root = config.db_root.as_path();
    let ignore_patterns = config.ignore_patterns.as_slice();

    let (replacement_uuid, replacement_title) = graph
        .node(opts.target_uuid.as_str())
        .map(|n| (n.uuid.clone(), n.title.clone()))
        .ok_or_else(|| {
            anyhow::anyhow!(
                "Replacement UUID not found in database: {}",
                opts.target_uuid
            )
        })?;

    let (files_affected, total_replacements) = find_and_replace_links(
        db_root,
        ignore_patterns,
        &opts.broken_uuid,
        &replacement_uuid,
        opts.apply,
    )?;

    let output = UuidFixOutput {
        broken_uuid: opts.broken_uuid.clone(),
        replacement_uuid: replacement_uuid.to_string(),
        replacement_title,
        files_affected: files_affected.clone(),
        total_replacements,
        applied: opts.apply,
    };

    Ok(output)
}

pub fn execute_attach(config: &OrgConfig, opts: &FixAttachOptions) -> Result<AttachFixOutput> {
    let graph = crate::load_graph(config)?;
    let db_root = config.db_root.as_path();
    let mode = if opts.copy {
        AttachMode::Copy
    } else {
        AttachMode::Move
    };

    let search_roots = collect_attachment_search_roots(db_root)?;
    let mut repairs = Vec::new();
    let mut skipped = Vec::new();

    for candidate in collect_attachment_candidates(&graph, db_root) {
        if !is_safe_attachment_target(&candidate.target_path) {
            skipped.push(skipped_attachment(&candidate, "unsafe_target", Vec::new()));
            continue;
        }
        if candidate.expected_path.exists() {
            continue;
        }

        let matches = find_safe_attachment_matches(&search_roots, &candidate.target_path);
        match matches.as_slice() {
            [found_path] => {
                if opts.apply {
                    apply_attachment_repair(mode, found_path, &candidate.expected_path)?;
                }
                repairs.push(repaired_attachment(&candidate, found_path, opts.apply));
            }
            [] => skipped.push(skipped_attachment(&candidate, "not_found", Vec::new())),
            _ => skipped.push(skipped_attachment(
                &candidate,
                "ambiguous",
                matches.iter().map(|path| path_string(path)).collect(),
            )),
        }
    }

    let output = AttachFixOutput {
        applied: opts.apply,
        mode: mode.as_str().to_string(),
        total_repaired: repairs.len(),
        total_skipped: skipped.len(),
        repairs,
        skipped,
    };

    Ok(output)
}

fn collect_attachment_candidates(graph: &Graph, db_root: &Path) -> Vec<AttachmentCandidate> {
    let mut candidates = Vec::new();
    for result in graph.files() {
        if result.parse_error.is_some() {
            continue;
        }
        let Some(primary_uuid) = result.parsed.uuids.first() else {
            continue;
        };
        let Some(primary_node) = graph.node(primary_uuid.as_str()) else {
            continue;
        };
        let primary_title = primary_node.title.clone();

        for heading in &result.parsed.headings {
            let (source_uuid, source_title) = heading
                .uuid
                .as_ref()
                .and_then(|uuid| {
                    graph
                        .node(uuid.as_str())
                        .map(|node| (uuid.to_string(), node.title.clone()))
                })
                .unwrap_or_else(|| (primary_uuid.to_string(), primary_title.clone()));

            for link in &heading.outgoing {
                if let Link::Attachment(target) = link {
                    let target_path = target.to_string();
                    candidates.push(AttachmentCandidate {
                        expected_path: resolve_attachment_path(db_root, &source_uuid, &target_path),
                        source_uuid: source_uuid.clone(),
                        source_title: source_title.clone(),
                        source_path: result.path.clone(),
                        target_path,
                    });
                }
            }
        }
    }
    candidates.sort_by(|a, b| {
        (
            &a.source_uuid,
            &a.source_title,
            &a.source_path,
            &a.target_path,
        )
            .cmp(&(
                &b.source_uuid,
                &b.source_title,
                &b.source_path,
                &b.target_path,
            ))
    });
    candidates
}

fn collect_attachment_search_roots(db_root: &Path) -> Result<Vec<AttachmentSearchRoot>> {
    let attach_root = db_root.join(".attach");
    let Ok(entries) = std::fs::read_dir(&attach_root) else {
        return Ok(Vec::new());
    };

    let mut roots = Vec::new();
    for entry in entries {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let root_name = entry.file_name().to_string_lossy().to_string();
        if is_uuid(&root_name) {
            roots.push(entry.path());
        }
        if root_name.len() == 2 {
            collect_hashed_attachment_search_roots(&entry.path(), &root_name, &mut roots)?;
        }
    }

    roots.sort();
    roots.dedup();
    Ok(roots
        .into_iter()
        .filter_map(|path| {
            std::fs::canonicalize(&path)
                .ok()
                .map(|canonical| AttachmentSearchRoot { path, canonical })
        })
        .collect())
}

fn collect_hashed_attachment_search_roots(
    bucket_path: &Path,
    bucket_name: &str,
    roots: &mut Vec<PathBuf>,
) -> Result<()> {
    let Ok(entries) = std::fs::read_dir(bucket_path) else {
        return Ok(());
    };
    for entry in entries {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let rest = entry.file_name().to_string_lossy().to_string();
        if is_uuid(&format!("{bucket_name}{rest}")) {
            roots.push(entry.path());
        }
    }
    Ok(())
}

fn is_uuid(value: &str) -> bool {
    uuid::Uuid::parse_str(value).is_ok()
}

fn find_safe_attachment_matches(roots: &[AttachmentSearchRoot], target: &str) -> Vec<PathBuf> {
    let mut matches = Vec::new();
    for root in roots {
        let candidate = root.path.join(target);
        if let Some(path) = safe_existing_attachment_file(root, &candidate) {
            matches.push(path);
        }
    }
    matches.sort();
    matches.dedup();
    matches
}

fn safe_existing_attachment_file(root: &AttachmentSearchRoot, path: &Path) -> Option<PathBuf> {
    let metadata = std::fs::symlink_metadata(path).ok()?;
    if !metadata.file_type().is_file() {
        return None;
    }
    let canonical_path = std::fs::canonicalize(path).ok()?;
    if !canonical_path.starts_with(&root.canonical) {
        return None;
    }
    Some(path.to_path_buf())
}

fn is_safe_attachment_target(target: &str) -> bool {
    if target.is_empty() {
        return false;
    }
    let path = Path::new(target);
    !path.is_absolute()
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
}

fn apply_attachment_repair(
    mode: AttachMode,
    found_path: &Path,
    expected_path: &Path,
) -> Result<()> {
    if let Some(parent) = expected_path.parent() {
        std::fs::create_dir_all(parent).with_context(|| {
            format!(
                "Failed to create attachment directory: {}",
                parent.display()
            )
        })?;
    }
    match mode {
        AttachMode::Move => std::fs::rename(found_path, expected_path).with_context(|| {
            format!(
                "Failed to move attachment from {} to {}",
                found_path.display(),
                expected_path.display()
            )
        })?,
        AttachMode::Copy => {
            std::fs::copy(found_path, expected_path).with_context(|| {
                format!(
                    "Failed to copy attachment from {} to {}",
                    found_path.display(),
                    expected_path.display()
                )
            })?;
        }
    }
    Ok(())
}

fn repaired_attachment(
    candidate: &AttachmentCandidate,
    found_path: &Path,
    applied: bool,
) -> AttachFixRepair {
    AttachFixRepair {
        source_uuid: candidate.source_uuid.clone(),
        source_title: candidate.source_title.clone(),
        source_path: path_string(&candidate.source_path),
        target_path: candidate.target_path.clone(),
        found_path: path_string(found_path),
        expected_path: path_string(&candidate.expected_path),
        applied,
    }
}

fn skipped_attachment(
    candidate: &AttachmentCandidate,
    reason: &str,
    matches: Vec<String>,
) -> AttachFixSkipped {
    AttachFixSkipped {
        source_uuid: candidate.source_uuid.clone(),
        source_title: candidate.source_title.clone(),
        source_path: path_string(&candidate.source_path),
        target_path: candidate.target_path.clone(),
        expected_path: path_string(&candidate.expected_path),
        reason: reason.to_string(),
        matches,
    }
}

fn path_string(path: &Path) -> String {
    path.display().to_string()
}
