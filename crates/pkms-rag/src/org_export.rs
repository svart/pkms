use std::{
    fs,
    path::{Component, Path},
    time::UNIX_EPOCH,
};

use anyhow::{Context, Result};
use pkms_org::Corpus;

use crate::{
    chunking::{ChunkNoteInput, chunk_note, clean_list, content_hash},
    models::{NoteRecord, RetrievalRecord, SUPPORTED_SCHEMA_VERSION},
};

pub fn export_org_notes(root: impl AsRef<Path>) -> Result<Vec<RetrievalRecord>> {
    export_org_notes_with_ignore(root, &[])
}

pub fn export_org_notes_with_ignore(
    root: impl AsRef<Path>,
    ignore_patterns: &[String],
) -> Result<Vec<RetrievalRecord>> {
    let root = root.as_ref().canonicalize().with_context(|| {
        format!(
            "failed to resolve org notes root {}",
            root.as_ref().display()
        )
    })?;
    let corpus = Corpus::scan(&root, ignore_patterns)
        .with_context(|| format!("failed to scan org notes root {}", root.display()))?;
    let mut results = corpus.results().iter().collect::<Vec<_>>();
    results.sort_by_key(|result| &result.path);

    let mut records = Vec::new();
    for result in results {
        let raw_content = result.raw_content.as_deref().with_context(|| {
            format!(
                "failed to read org file {}",
                display_path(result.path.as_path())
            )
        })?;
        let Some(note_id) = result.parsed.uuids.first() else {
            continue;
        };

        let path = relative_org_path(&root, &result.path);
        let title = note_title(&result.path, result.parsed.title.as_deref());
        let aliases = clean_list(result.parsed.aliases.clone());
        let tags = clean_list(result.parsed.filetags.clone());
        let updated_at = modified_unix_seconds(&result.path)?;

        records.push(RetrievalRecord::Note(NoteRecord {
            schema_version: SUPPORTED_SCHEMA_VERSION,
            note_id: note_id.as_str().to_string(),
            path: path.clone(),
            title: title.clone(),
            aliases: aliases.clone(),
            tags: tags.clone(),
            updated_at,
            content_hash: content_hash(raw_content),
        }));

        for chunk in chunk_note(ChunkNoteInput {
            note_id: note_id.as_str(),
            path: &path,
            title: &title,
            aliases: &aliases,
            tags: &tags,
            updated_at,
            parsed: &result.parsed,
            raw_content,
        })? {
            records.push(RetrievalRecord::Chunk(chunk));
        }
    }

    Ok(records)
}

fn modified_unix_seconds(path: &Path) -> Result<i64> {
    let modified = fs::metadata(path)
        .with_context(|| format!("failed to stat org file {}", display_path(path)))?
        .modified()
        .with_context(|| format!("failed to read modified time for {}", display_path(path)))?;
    let duration = modified.duration_since(UNIX_EPOCH).unwrap_or_default();
    i64::try_from(duration.as_secs()).context("file modified time exceeds i64 seconds")
}

fn note_title(path: &Path, parsed_title: Option<&str>) -> String {
    parsed_title
        .map(str::trim)
        .filter(|title| !title.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| {
            path.file_stem()
                .map(|stem| stem.to_string_lossy().into_owned())
                .unwrap_or_else(|| "Untitled".to_string())
        })
}

fn relative_org_path(root: &Path, path: &Path) -> String {
    let relative = path.strip_prefix(root).unwrap_or(path);
    path_to_forward_slashes(relative)
}

fn path_to_forward_slashes(path: &Path) -> String {
    path.components()
        .filter_map(|component| match component {
            Component::Normal(value) => Some(value.to_string_lossy()),
            Component::CurDir => None,
            Component::ParentDir => Some("..".into()),
            Component::RootDir | Component::Prefix(_) => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}

fn display_path(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn org_export_matches_simple_python_parity_fixture() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        fs::write(
            tempdir.path().join("semantic-search.org"),
            "\
#+title: Semantic Search Notes
#+filetags: :pkms:rag:
:PROPERTIES:
:ID: bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb
:END:
* Retrieval workflow
Use [[id:cccccccc-cccc-4ccc-cccc-cccccccccccc][linked context]] for semantic PKMS search.
",
        )
        .expect("note writes");

        let records = export_org_notes(tempdir.path()).expect("notes export");

        assert_eq!(records.len(), 2);
        let RetrievalRecord::Note(note) = &records[0] else {
            panic!("first record is note");
        };
        assert_eq!(note.note_id, "bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb");
        assert_eq!(note.path, "semantic-search.org");
        assert_eq!(note.title, "Semantic Search Notes");
        assert_eq!(note.tags, vec!["pkms", "rag"]);
        assert!(note.content_hash.starts_with("sha256:"));

        let RetrievalRecord::Chunk(chunk) = &records[1] else {
            panic!("second record is chunk");
        };
        assert_eq!(chunk.path, "semantic-search.org");
        assert_eq!(chunk.heading_path, vec!["Retrieval workflow"]);
        assert_eq!(chunk.start_line, 6);
        assert_eq!(chunk.end_line, 7);
        assert_eq!(
            chunk.outgoing_ids,
            vec!["cccccccc-cccc-4ccc-cccc-cccccccccccc"]
        );
        assert!(chunk.body.contains("semantic PKMS search"));
        assert!(!chunk.body.contains(":PROPERTIES:"));
        assert!(!chunk.body.contains("#+title:"));
    }

    #[test]
    fn org_export_skips_files_without_note_level_id() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        fs::write(
            tempdir.path().join("missing-id.org"),
            "\
#+title: Missing ID
* Heading
Text.
",
        )
        .expect("note writes");

        let records = export_org_notes(tempdir.path()).expect("notes export");

        assert!(records.is_empty());
    }

    #[test]
    fn org_export_uses_aliases_and_file_stem_fallback_title() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        fs::create_dir(tempdir.path().join("topics")).expect("dir creates");
        fs::write(
            tempdir.path().join("topics").join("fallback-title.org"),
            "\
:PROPERTIES:
:ID: bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb
:ROAM_ALIASES: \"Semantic Search\" RAG
:END:
* Heading
Text.
",
        )
        .expect("note writes");

        let records = export_org_notes(tempdir.path()).expect("notes export");

        let RetrievalRecord::Note(note) = &records[0] else {
            panic!("first record is note");
        };
        assert_eq!(note.path, "topics/fallback-title.org");
        assert_eq!(note.title, "fallback-title");
        assert_eq!(note.aliases, vec!["RAG", "Semantic Search"]);
    }

    #[test]
    fn org_export_respects_ignore_patterns() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        fs::write(
            tempdir.path().join("keep.org"),
            "\
:PROPERTIES:
:ID: bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb
:END:
* Keep
",
        )
        .expect("note writes");
        fs::write(
            tempdir.path().join("skip.org"),
            "\
:PROPERTIES:
:ID: cccccccc-cccc-4ccc-cccc-cccccccccccc
:END:
* Skip
",
        )
        .expect("note writes");

        let records = export_org_notes_with_ignore(tempdir.path(), &["skip.org".to_string()])
            .expect("notes export");

        let note_ids = records
            .iter()
            .filter_map(|record| match record {
                RetrievalRecord::Note(note) => Some(note.note_id.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(note_ids, vec!["bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb"]);
    }
}
