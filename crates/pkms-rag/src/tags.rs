use std::{
    cmp::Ordering,
    collections::{BTreeMap, HashMap, HashSet},
};

use anyhow::{Result, ensure};
use pkms_org::ScopeFilter;
use rusqlite::Connection;

use crate::models::{
    SearchResult, TagRecommendation, TagRecommendationEvidence, TagRecommendationRequest, TagScope,
    TagSourceRange,
};
use crate::{
    embeddings::EmbeddingProvider,
    storage::sqlite::{dense_search_filtered, note_tags as indexed_note_tags},
};

pub fn tag_query_text(title: &str, content: &str) -> String {
    let mut query = String::new();
    let title = title.trim();
    if !title.is_empty() {
        query.push_str(title);
        query.push('\n');
    }

    let mut in_property_drawer = false;
    let mut in_source_block = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if in_source_block {
            query.push_str(line);
            query.push('\n');
            if trimmed.eq_ignore_ascii_case("#+end_src") {
                in_source_block = false;
            }
            continue;
        }
        if trimmed
            .get(..11)
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case("#+begin_src"))
        {
            in_source_block = true;
            query.push_str(line);
            query.push('\n');
            continue;
        }
        if trimmed.eq_ignore_ascii_case(":PROPERTIES:") {
            in_property_drawer = true;
            continue;
        }
        if in_property_drawer {
            if trimmed.eq_ignore_ascii_case(":END:") {
                in_property_drawer = false;
            }
            continue;
        }
        if trimmed.starts_with("#+") {
            continue;
        }
        query.push_str(line);
        query.push('\n');
    }
    let maximum = crate::embeddings::DEFAULT_EMBEDDING_MAX_BODY_CHARS;
    if query.chars().count() > maximum {
        query = query.chars().take(maximum).collect();
    }
    query.trim().to_string()
}

pub(crate) fn recommend_tags(
    conn: &Connection,
    request: &TagRecommendationRequest,
    provider: &dyn EmbeddingProvider,
) -> Result<Vec<TagRecommendation>> {
    recommend_tags_scoped(conn, request, provider, None)
}

pub(crate) fn recommend_tags_scoped(
    conn: &Connection,
    request: &TagRecommendationRequest,
    provider: &dyn EmbeddingProvider,
    scope_filter: Option<&ScopeFilter>,
) -> Result<Vec<TagRecommendation>> {
    ensure!(
        request.limit > 0,
        "tag recommendation limit must be positive"
    );
    ensure!(
        request.neighbor_limit > 0,
        "tag recommendation neighbor limit must be positive"
    );
    if request.scope == TagScope::Heading {
        ensure!(
            request.target_range.is_some(),
            "heading tag recommendations require a target source range"
        );
    }
    if let Some(range) = request.target_range {
        ensure!(
            range.start_line <= range.end_line,
            "tag recommendation target range start must not exceed its end"
        );
    }

    let results = dense_search_filtered(
        conn,
        &request.query,
        request.neighbor_limit,
        provider,
        |result| {
            !is_target_result(result, request)
                && scope_filter.is_none_or(|filter| {
                    filter.matches(std::path::Path::new(&result.path), &result.tags, true)
                })
        },
    )?;

    let mut note_tags = HashMap::new();
    for result in &results {
        if !note_tags.contains_key(&result.uuid) {
            note_tags.insert(result.uuid.clone(), indexed_note_tags(conn, &result.uuid)?);
        }
    }
    Ok(rank_tag_candidates(
        &results,
        &note_tags,
        request.scope,
        &request.existing_tags,
        request.limit,
    ))
}

fn is_target_result(result: &SearchResult, request: &TagRecommendationRequest) -> bool {
    if result.uuid != request.target_note_id {
        return false;
    }
    match request.scope {
        TagScope::Note => true,
        TagScope::Heading => request
            .target_range
            .is_some_and(|range| ranges_overlap(result, range)),
    }
}

fn ranges_overlap(result: &SearchResult, range: TagSourceRange) -> bool {
    result.start_line <= range.end_line && result.end_line >= range.start_line
}

pub(crate) fn rank_tag_candidates(
    results: &[SearchResult],
    note_tags: &HashMap<String, Vec<String>>,
    scope: TagScope,
    existing_tags: &[String],
    limit: usize,
) -> Vec<TagRecommendation> {
    let existing = existing_tags.iter().collect::<HashSet<_>>();
    let mut candidates: BTreeMap<String, HashMap<String, TagRecommendationEvidence>> =
        BTreeMap::new();

    for result in results {
        let inherited = note_tags
            .get(&result.uuid)
            .map(Vec::as_slice)
            .unwrap_or_default();
        let inherited = inherited.iter().collect::<HashSet<_>>();
        let tags = match scope {
            TagScope::Note => note_tags
                .get(&result.uuid)
                .map(Vec::as_slice)
                .unwrap_or_default(),
            TagScope::Heading => result.tags.as_slice(),
        };

        for tag in tags {
            if existing.contains(tag)
                || !is_canonical_tag(tag)
                || (scope == TagScope::Heading && inherited.contains(tag))
                || result.scores.dense <= 0.0
            {
                continue;
            }
            let evidence = TagRecommendationEvidence {
                uuid: result.uuid.clone(),
                title: result.title.clone(),
                path: result.path.clone(),
                heading_path: result.heading_path.clone(),
                score: result.scores.dense,
            };
            candidates
                .entry(tag.clone())
                .or_default()
                .entry(result.uuid.clone())
                .and_modify(|current| {
                    if evidence.score > current.score {
                        *current = evidence.clone();
                    }
                })
                .or_insert(evidence);
        }
    }

    let maximum = candidates
        .values()
        .map(|evidence| evidence.values().map(|item| item.score).sum::<f64>())
        .fold(0.0, f64::max);
    let mut recommendations = candidates
        .into_iter()
        .map(|(tag, by_note)| {
            let raw_score = by_note.values().map(|item| item.score).sum::<f64>();
            let mut evidence = by_note.into_values().collect::<Vec<_>>();
            evidence.sort_by(|left, right| {
                right
                    .score
                    .partial_cmp(&left.score)
                    .unwrap_or(Ordering::Equal)
                    .then_with(|| left.uuid.cmp(&right.uuid))
                    .then_with(|| left.heading_path.cmp(&right.heading_path))
            });
            let support = evidence.len();
            evidence.truncate(3);
            TagRecommendation {
                tag,
                score: if maximum > 0.0 {
                    raw_score / maximum
                } else {
                    0.0
                },
                support,
                evidence,
            }
        })
        .collect::<Vec<_>>();
    recommendations.sort_by(|left, right| {
        right
            .score
            .partial_cmp(&left.score)
            .unwrap_or(Ordering::Equal)
            .then_with(|| right.support.cmp(&left.support))
            .then_with(|| left.tag.cmp(&right.tag))
    });
    recommendations.truncate(limit);
    recommendations
}

fn is_canonical_tag(tag: &str) -> bool {
    !tag.is_empty() && tag.chars().all(|ch| ch != ':' && !ch.is_whitespace())
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::{
        RagIndex,
        embeddings::HashEmbeddingProvider,
        models::{
            ChunkRecord, NoteRecord, RetrievalRecord, SUPPORTED_SCHEMA_VERSION, ScoreBreakdown,
            SearchResult, TagRecommendationRequest, TagScope, TagSourceRange,
        },
    };

    use super::{rank_tag_candidates, tag_query_text};

    fn result(note_id: &str, chunk_id: &str, tags: &[&str], score: f64) -> SearchResult {
        SearchResult {
            chunk_id: chunk_id.to_string(),
            uuid: note_id.to_string(),
            title: format!("Note {note_id}"),
            path: format!("{note_id}.org"),
            aliases: Vec::new(),
            tags: tags.iter().map(|tag| (*tag).to_string()).collect(),
            heading_path: vec!["Heading".to_string()],
            heading_level: 1,
            start_line: 5,
            end_line: 8,
            text: String::new(),
            scores: ScoreBreakdown {
                dense: score,
                final_score: score,
                ..ScoreBreakdown::default()
            },
        }
    }

    #[test]
    fn note_scope_uses_filetags_and_caps_each_source_note_contribution() {
        let results = vec![
            result("a", "a:1", &["rust", "task"], 0.9),
            result("a", "a:2", &["rust", "task"], 0.8),
            result("b", "b:1", &["rust", "pkms"], 0.7),
        ];
        let note_tags = HashMap::from([
            ("a".to_string(), vec!["rust".to_string()]),
            ("b".to_string(), vec!["pkms".to_string()]),
        ]);

        let recommendations = rank_tag_candidates(&results, &note_tags, TagScope::Note, &[], 5);

        assert_eq!(
            recommendations
                .iter()
                .map(|item| (item.tag.as_str(), item.support))
                .collect::<Vec<_>>(),
            vec![("rust", 1), ("pkms", 1)]
        );
        assert_eq!(recommendations[0].score, 1.0);
        assert!((recommendations[1].score - (0.7 / 0.9)).abs() < 1e-9);
    }

    #[test]
    fn heading_scope_excludes_inherited_and_existing_tags() {
        let results = vec![
            result("a", "a:1", &["agenda", "phone", "work"], 0.9),
            result("b", "b:1", &["agenda", "phone", "waiting"], 0.7),
        ];
        let note_tags = HashMap::from([
            (
                "a".to_string(),
                vec!["agenda".to_string(), "work".to_string()],
            ),
            ("b".to_string(), vec!["agenda".to_string()]),
        ]);

        let recommendations = rank_tag_candidates(
            &results,
            &note_tags,
            TagScope::Heading,
            &["waiting".to_string()],
            5,
        );

        assert_eq!(recommendations.len(), 1);
        assert_eq!(recommendations[0].tag, "phone");
        assert_eq!(recommendations[0].support, 2);
        assert_eq!(recommendations[0].evidence.len(), 2);
    }

    #[test]
    fn ranking_rejects_noncanonical_tags_and_breaks_ties_by_tag() {
        let results = vec![result(
            "a",
            "a:1",
            &["valid", "also-valid", "bad tag", "bad:tag"],
            0.5,
        )];
        let note_tags = HashMap::from([(
            "a".to_string(),
            vec![
                "valid".to_string(),
                "also-valid".to_string(),
                "bad tag".to_string(),
                "bad:tag".to_string(),
            ],
        )]);

        let recommendations = rank_tag_candidates(&results, &note_tags, TagScope::Note, &[], 5);

        assert_eq!(
            recommendations
                .iter()
                .map(|item| item.tag.as_str())
                .collect::<Vec<_>>(),
            vec!["also-valid", "valid"]
        );
    }

    #[test]
    fn indexed_recommendations_exclude_note_self_matches() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let index = RagIndex::open(tempdir.path().join("rag.sqlite3")).expect("index opens");
        index
            .ingest(&tagged_records(), &HashEmbeddingProvider::default(), true)
            .expect("records ingest");

        let recommendations = index
            .recommend_tags(
                &TagRecommendationRequest {
                    query: "semantic retrieval".to_string(),
                    scope: TagScope::Note,
                    target_note_id: "target".to_string(),
                    target_range: None,
                    existing_tags: Vec::new(),
                    limit: 5,
                    neighbor_limit: 20,
                },
                &HashEmbeddingProvider::default(),
            )
            .expect("recommendations");

        assert_eq!(recommendations.len(), 1);
        assert_eq!(recommendations[0].tag, "rag");
        assert_eq!(recommendations[0].evidence[0].uuid, "neighbor");
    }

    #[test]
    fn indexed_heading_recommendations_exclude_target_subtree_and_filetags() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let index = RagIndex::open(tempdir.path().join("rag.sqlite3")).expect("index opens");
        index
            .ingest(&tagged_records(), &HashEmbeddingProvider::default(), true)
            .expect("records ingest");

        let recommendations = index
            .recommend_tags(
                &TagRecommendationRequest {
                    query: "semantic retrieval".to_string(),
                    scope: TagScope::Heading,
                    target_note_id: "target".to_string(),
                    target_range: Some(TagSourceRange {
                        start_line: 5,
                        end_line: 8,
                    }),
                    existing_tags: Vec::new(),
                    limit: 5,
                    neighbor_limit: 20,
                },
                &HashEmbeddingProvider::default(),
            )
            .expect("recommendations");

        assert_eq!(recommendations.len(), 1);
        assert_eq!(recommendations[0].tag, "phone");
    }

    #[test]
    fn tag_query_text_excludes_org_metadata_and_property_drawers() {
        let query = tag_query_text(
            "Semantic note",
            "#+title: Hidden metadata\n:PROPERTIES:\n:SECRET: hidden\n:END:\n* Visible heading\nVisible body\n",
        );

        assert!(query.starts_with("Semantic note\n"));
        assert!(query.contains("Visible heading"));
        assert!(query.contains("Visible body"));
        assert!(!query.contains("Hidden metadata"));
        assert!(!query.contains("SECRET"));
    }

    #[test]
    fn tag_query_text_limits_unicode_by_character_count() {
        let query = tag_query_text("", &"ж".repeat(9_000));

        assert_eq!(query.chars().count(), 8_000);
    }

    #[test]
    fn indexed_recommendations_reject_same_model_with_different_dimension() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let index = RagIndex::open(tempdir.path().join("rag.sqlite3")).expect("index opens");
        index
            .ingest(
                &tagged_records(),
                &HashEmbeddingProvider::with_dimension(16),
                true,
            )
            .expect("records ingest");

        let error = index
            .recommend_tags(
                &TagRecommendationRequest {
                    query: "semantic retrieval".to_string(),
                    scope: TagScope::Note,
                    target_note_id: "target".to_string(),
                    target_range: None,
                    existing_tags: Vec::new(),
                    limit: 5,
                    neighbor_limit: 20,
                },
                &HashEmbeddingProvider::default(),
            )
            .expect_err("dimension mismatch rejected");

        assert!(error.to_string().contains("compatible embeddings"));
    }

    #[test]
    fn indexed_recommendations_exclude_self_before_neighbor_limit() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let index = RagIndex::open(tempdir.path().join("rag.sqlite3")).expect("index opens");
        let mut records = vec![RetrievalRecord::Note(NoteRecord {
            schema_version: SUPPORTED_SCHEMA_VERSION,
            note_id: "a-target".to_string(),
            path: "a-target.org".to_string(),
            title: "Target".to_string(),
            aliases: Vec::new(),
            tags: vec!["private".to_string()],
            updated_at: 1,
            content_hash: "target-note".to_string(),
        })];
        for index in 0..40 {
            records.push(RetrievalRecord::Chunk(ChunkRecord {
                schema_version: SUPPORTED_SCHEMA_VERSION,
                chunk_id: format!("a-target:{index:02}"),
                note_id: "a-target".to_string(),
                path: "a-target.org".to_string(),
                title: "Target".to_string(),
                aliases: Vec::new(),
                tags: vec!["private".to_string()],
                heading_path: vec![format!("Target {index}")],
                heading_level: 1,
                body: "semantic retrieval".to_string(),
                start_line: index + 1,
                end_line: index + 1,
                outgoing_ids: Vec::new(),
                updated_at: 1,
                content_hash: format!("target-chunk-{index}"),
            }));
        }
        records.extend([
            RetrievalRecord::Note(NoteRecord {
                schema_version: SUPPORTED_SCHEMA_VERSION,
                note_id: "z-neighbor".to_string(),
                path: "z-neighbor.org".to_string(),
                title: "Neighbor".to_string(),
                aliases: Vec::new(),
                tags: vec!["rag".to_string()],
                updated_at: 1,
                content_hash: "neighbor-note".to_string(),
            }),
            RetrievalRecord::Chunk(ChunkRecord {
                schema_version: SUPPORTED_SCHEMA_VERSION,
                chunk_id: "z-neighbor:chunk".to_string(),
                note_id: "z-neighbor".to_string(),
                path: "z-neighbor.org".to_string(),
                title: "Neighbor".to_string(),
                aliases: Vec::new(),
                tags: vec!["rag".to_string()],
                heading_path: vec!["Neighbor".to_string()],
                heading_level: 1,
                body: "semantic retrieval".to_string(),
                start_line: 1,
                end_line: 1,
                outgoing_ids: Vec::new(),
                updated_at: 1,
                content_hash: "neighbor-chunk".to_string(),
            }),
        ]);
        index
            .ingest(&records, &HashEmbeddingProvider::default(), true)
            .expect("records ingest");

        let recommendations = index
            .recommend_tags(
                &TagRecommendationRequest {
                    query: "Target\nprivate\nTarget 0\nsemantic retrieval".to_string(),
                    scope: TagScope::Note,
                    target_note_id: "a-target".to_string(),
                    target_range: None,
                    existing_tags: Vec::new(),
                    limit: 1,
                    neighbor_limit: 1,
                },
                &HashEmbeddingProvider::default(),
            )
            .expect("recommendations");

        assert_eq!(recommendations[0].tag, "rag");
    }

    fn tagged_records() -> Vec<RetrievalRecord> {
        let mut records = Vec::new();
        for (id, note_tags, chunk_tags) in [
            ("target", vec!["private"], vec!["private", "self-only"]),
            ("neighbor", vec!["rag"], vec!["rag", "phone"]),
        ] {
            records.push(RetrievalRecord::Note(NoteRecord {
                schema_version: SUPPORTED_SCHEMA_VERSION,
                note_id: id.to_string(),
                path: format!("{id}.org"),
                title: id.to_string(),
                aliases: Vec::new(),
                tags: note_tags.into_iter().map(str::to_string).collect(),
                updated_at: 1,
                content_hash: format!("hash-note-{id}"),
            }));
            records.push(RetrievalRecord::Chunk(ChunkRecord {
                schema_version: SUPPORTED_SCHEMA_VERSION,
                chunk_id: format!("{id}:chunk"),
                note_id: id.to_string(),
                path: format!("{id}.org"),
                title: id.to_string(),
                aliases: Vec::new(),
                tags: chunk_tags.into_iter().map(str::to_string).collect(),
                heading_path: vec!["Task".to_string()],
                heading_level: 1,
                body: "semantic retrieval".to_string(),
                start_line: 5,
                end_line: 8,
                outgoing_ids: Vec::new(),
                updated_at: 1,
                content_hash: format!("hash-chunk-{id}"),
            }));
        }
        records
    }
}
