use std::{
    cmp::Ordering,
    collections::{HashMap, HashSet},
};

use anyhow::{Context, Result};
use pkms_tokens::{Encoding, count_tokens};
use rusqlite::Connection;

use crate::{
    embeddings::EmbeddingProvider,
    models::{
        RetrieveMode, RetrieveRequest, RetrieveResponse, RetrieveResult, RetrieveWeights,
        ScoreBreakdown, SearchResult,
    },
    storage::sqlite::{dense_search, row_to_stored_search_result, search},
};

#[derive(Debug, Clone)]
struct GraphCandidate {
    result: SearchResult,
    score: f64,
}

pub(crate) fn retrieve(
    conn: &Connection,
    request: &RetrieveRequest,
    embedding_provider: &dyn EmbeddingProvider,
) -> Result<RetrieveResponse> {
    Ok(RetrieveResponse {
        query: request.query.clone(),
        mode: request.mode,
        results: retrieve_results(
            conn,
            &request.query,
            request.limit,
            request.mode,
            request.max_token_budget,
            request.weights,
            embedding_provider,
        )?,
    })
}

pub(crate) fn retrieve_results(
    conn: &Connection,
    query: &str,
    limit: usize,
    mode: RetrieveMode,
    max_token_budget: Option<usize>,
    weights: RetrieveWeights,
    embedding_provider: &dyn EmbeddingProvider,
) -> Result<Vec<RetrieveResult>> {
    if limit == 0 {
        return Ok(Vec::new());
    }

    let candidate_limit = limit.saturating_mul(4).max(20);
    let bm25_results = if mode == RetrieveMode::Dense {
        Vec::new()
    } else {
        search(conn, query, candidate_limit)?
    };
    let dense_results = if mode == RetrieveMode::Bm25 {
        Vec::new()
    } else {
        dense_search(conn, query, candidate_limit, embedding_provider)?
    };

    let mut by_chunk = HashMap::new();
    let mut bm25_scores = HashMap::new();
    let mut dense_scores = HashMap::new();

    for result in bm25_results {
        bm25_scores.insert(result.chunk_id.clone(), result.scores.bm25);
        by_chunk.insert(result.chunk_id.clone(), result);
    }
    for result in dense_results {
        dense_scores.insert(result.chunk_id.clone(), result.scores.dense);
        by_chunk.entry(result.chunk_id.clone()).or_insert(result);
    }

    let graph_expansion_scores = expand_graph_candidates(conn, by_chunk.values(), candidate_limit)?;
    for (chunk_id, candidate) in &graph_expansion_scores {
        by_chunk
            .entry(chunk_id.clone())
            .or_insert_with(|| candidate.result.clone());
    }

    let max_bm25 = max_score(bm25_scores.values().copied());
    let max_dense = max_score(dense_scores.values().copied());
    let query_terms = query_terms(query);

    let mut retrieved = Vec::new();
    for (chunk_id, result) in by_chunk {
        let bm25 = normalize(*bm25_scores.get(&chunk_id).unwrap_or(&0.0), max_bm25);
        let dense = normalize(*dense_scores.get(&chunk_id).unwrap_or(&0.0), max_dense);
        let metadata = metadata_boost(&query_terms, &result);
        let expansion_graph = graph_expansion_scores
            .get(&chunk_id)
            .map_or(0.0, |candidate| candidate.score);
        let graph = graph_boost(conn, &result)?.max(expansion_graph);
        let final_score = weights.bm25 * bm25
            + weights.dense * dense
            + weights.metadata * metadata
            + weights.graph * graph;
        let reason = reason(bm25, dense, metadata, graph, expansion_graph);
        let mut result = result;
        result.scores = ScoreBreakdown {
            bm25,
            dense,
            metadata,
            graph,
            final_score,
        };
        retrieved.push(RetrieveResult { result, reason });
    }

    retrieved.sort_by(|left, right| {
        right
            .result
            .scores
            .final_score
            .partial_cmp(&left.result.scores.final_score)
            .unwrap_or(Ordering::Equal)
            .then_with(|| left.result.chunk_id.cmp(&right.result.chunk_id))
    });
    Ok(trim_to_budget(retrieved, limit, max_token_budget))
}

fn normalize(score: f64, maximum: f64) -> f64 {
    if maximum <= 0.0 {
        return 0.0;
    }
    (score / maximum).max(0.0)
}

fn max_score(scores: impl Iterator<Item = f64>) -> f64 {
    scores.fold(0.0, f64::max)
}

fn metadata_boost(query_terms: &HashSet<String>, result: &SearchResult) -> f64 {
    if query_terms.is_empty() {
        return 0.0;
    }
    let metadata = [
        vec![result.title.clone()],
        result.aliases.clone(),
        result.tags.clone(),
        result.heading_path.clone(),
    ]
    .concat()
    .join(" ")
    .to_lowercase();
    let hits = query_terms
        .iter()
        .filter(|term| metadata.contains(term.as_str()))
        .count();
    (hits as f64 / query_terms.len() as f64).min(1.0)
}

fn query_terms(query: &str) -> HashSet<String> {
    let mut terms = HashSet::new();
    let mut current = String::new();
    for ch in query.chars() {
        if ch.is_alphanumeric() || ch == '_' {
            current.extend(ch.to_lowercase());
        } else if !current.is_empty() {
            terms.insert(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        terms.insert(current);
    }
    terms
}

fn reason(bm25: f64, dense: f64, metadata: f64, graph: f64, expansion_graph: f64) -> String {
    let mut parts = Vec::new();
    if bm25 > 0.0 {
        parts.push("bm25");
    }
    if dense > 0.0 {
        parts.push("dense");
    }
    if metadata > 0.0 {
        parts.push("metadata");
    }
    if expansion_graph > 0.0 && bm25 == 0.0 && dense == 0.0 {
        parts.push("graph-expanded");
    } else if graph > 0.0 {
        parts.push("links");
    }
    if parts.is_empty() {
        "fallback".to_string()
    } else {
        parts.join("+")
    }
}

fn expand_graph_candidates<'a>(
    conn: &Connection,
    seeds: impl IntoIterator<Item = &'a SearchResult>,
    limit: usize,
) -> Result<HashMap<String, GraphCandidate>> {
    let mut expanded = HashMap::new();
    for seed in seeds {
        add_graph_rows(
            conn,
            r#"
            SELECT
                c.chunk_id,
                c.note_id,
                c.title,
                c.path,
                c.aliases_json,
                c.tags_json,
                c.heading_path_json,
                c.heading_level,
                c.start_line,
                c.end_line,
                c.body
            FROM links l
            JOIN chunks c ON c.note_id = l.target_note_id
            WHERE l.source_chunk_id = ?
              AND c.stale = 0
            ORDER BY c.note_id, c.heading_level, c.start_line, c.chunk_id
            "#,
            &seed.chunk_id,
            1.0,
            &mut expanded,
        )?;
        add_graph_rows(
            conn,
            r#"
            SELECT
                c.chunk_id,
                c.note_id,
                c.title,
                c.path,
                c.aliases_json,
                c.tags_json,
                c.heading_path_json,
                c.heading_level,
                c.start_line,
                c.end_line,
                c.body
            FROM links l
            JOIN chunks c ON c.chunk_id = l.source_chunk_id
            WHERE l.target_note_id = ?
              AND c.stale = 0
            ORDER BY c.note_id, c.heading_level, c.start_line, c.chunk_id
            "#,
            &seed.uuid,
            0.9,
            &mut expanded,
        )?;
        add_graph_rows(
            conn,
            r#"
            SELECT
                c.chunk_id,
                c.note_id,
                c.title,
                c.path,
                c.aliases_json,
                c.tags_json,
                c.heading_path_json,
                c.heading_level,
                c.start_line,
                c.end_line,
                c.body
            FROM links l
            JOIN chunks c ON c.note_id = l.target_note_id
            WHERE l.source_note_id = ?
              AND c.stale = 0
            ORDER BY c.note_id, c.heading_level, c.start_line, c.chunk_id
            "#,
            &seed.uuid,
            0.8,
            &mut expanded,
        )?;
    }

    let mut expanded = expanded.into_iter().collect::<Vec<_>>();
    expanded.sort_by(|left, right| {
        right
            .1
            .score
            .partial_cmp(&left.1.score)
            .unwrap_or(Ordering::Equal)
            .then_with(|| left.0.cmp(&right.0))
    });
    expanded.truncate(limit);
    Ok(expanded.into_iter().collect())
}

fn add_graph_rows(
    conn: &Connection,
    query: &str,
    parameter: &str,
    score: f64,
    expanded: &mut HashMap<String, GraphCandidate>,
) -> Result<()> {
    let mut stmt = conn
        .prepare(query)
        .context("failed to prepare RAG graph expansion query")?;
    let rows = stmt
        .query_map([parameter], |row| {
            row_to_stored_search_result(
                row,
                ScoreBreakdown {
                    graph: score,
                    final_score: score,
                    ..ScoreBreakdown::default()
                },
            )
        })
        .context("failed to run RAG graph expansion query")?;
    for row in rows {
        let result = row.context("failed to read RAG graph expansion row")?;
        let candidate = GraphCandidate {
            score,
            result: result.clone(),
        };
        expanded
            .entry(result.chunk_id.clone())
            .and_modify(|existing| {
                if score > existing.score {
                    *existing = candidate.clone();
                }
            })
            .or_insert(candidate);
    }
    Ok(())
}

fn graph_boost(conn: &Connection, result: &SearchResult) -> Result<f64> {
    let count = conn
        .query_row(
            r#"
            SELECT COUNT(*)
            FROM links
            WHERE source_chunk_id = ?
               OR source_note_id = ?
               OR target_note_id = ?
            "#,
            [
                result.chunk_id.as_str(),
                result.uuid.as_str(),
                result.uuid.as_str(),
            ],
            |row| row.get::<_, i64>(0),
        )
        .context("failed to query RAG graph boost")?;
    if count <= 0 {
        Ok(0.0)
    } else {
        Ok((count as f64 / 5.0).min(1.0))
    }
}

fn trim_to_budget(
    results: Vec<RetrieveResult>,
    limit: usize,
    max_token_budget: Option<usize>,
) -> Vec<RetrieveResult> {
    let mut selected = Vec::new();
    let mut used_tokens = 0;
    for result in results {
        let estimated_tokens = count_tokens(&result.result.text, Encoding::Cl100kBase).max(1);
        if max_token_budget
            .is_some_and(|budget| !selected.is_empty() && used_tokens + estimated_tokens > budget)
        {
            continue;
        }
        selected.push(result);
        used_tokens += estimated_tokens;
        if selected.len() >= limit {
            break;
        }
    }
    selected
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        embeddings::HashEmbeddingProvider,
        ndjson::load_ndjson,
        storage::sqlite::{connect, ingest_records},
    };
    use std::path::PathBuf;

    fn fixture_records() -> Vec<crate::RetrievalRecord> {
        load_ndjson(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/retrieval-export.ndjson"),
        )
        .expect("fixture records load")
    }

    #[test]
    fn retrieve_hybrid_returns_cited_chunks_and_preserves_exact_terms() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let path = tempdir.path().join("rag.sqlite3");
        let mut conn = connect(&path).expect("connect initializes database");
        let provider = HashEmbeddingProvider::default();
        ingest_records(&mut conn, &fixture_records(), &provider, false).expect("fixture ingests");

        let results = retrieve_results(
            &conn,
            "externalHostname",
            5,
            RetrieveMode::Hybrid,
            None,
            RetrieveWeights::default(),
            &provider,
        )
        .expect("retrieve succeeds");

        assert!(!results.is_empty());
        let first = &results[0];
        assert_eq!(first.result.title, "Media Library Migration to Jellyfin");
        assert_eq!(first.result.uuid, "c6404b7e-5194-4a5a-89b6-cc9d4ae7ee27");
        assert_eq!(first.result.path, "ops/media-library.org");
        assert_eq!(first.result.heading_path, vec!["Seerr", "Jellyfin links"]);
        assert_eq!(first.result.start_line, 40);
        assert_eq!(first.result.end_line, 58);
        assert!(first.result.text.contains("externalHostname"));
        assert!(first.result.scores.final_score > 0.0);
        assert!(first.reason.contains("bm25"));
    }

    #[test]
    fn retrieve_expands_graph_neighbors_as_candidates() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let path = tempdir.path().join("rag.sqlite3");
        let mut conn = connect(&path).expect("connect initializes database");
        let provider = HashEmbeddingProvider::default();
        ingest_records(&mut conn, &fixture_records(), &provider, false).expect("fixture ingests");

        let results = retrieve_results(
            &conn,
            "externalHostname",
            5,
            RetrieveMode::Bm25,
            None,
            RetrieveWeights::default(),
            &provider,
        )
        .expect("retrieve succeeds");

        let expanded = results
            .iter()
            .find(|result| result.result.title == "PKMS Task Backend")
            .expect("graph-expanded task backend result");
        assert_eq!(expanded.result.scores.bm25, 0.0);
        assert_eq!(expanded.result.scores.dense, 0.0);
        assert!(expanded.result.scores.graph > 0.0);
        assert_eq!(expanded.reason, "graph-expanded");
    }

    #[test]
    fn retrieve_dense_mode_uses_dense_scores() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let path = tempdir.path().join("rag.sqlite3");
        let mut conn = connect(&path).expect("connect initializes database");
        let provider = HashEmbeddingProvider::default();
        ingest_records(&mut conn, &fixture_records(), &provider, false).expect("fixture ingests");

        let results = retrieve_results(
            &conn,
            "agenda inspect tasks",
            5,
            RetrieveMode::Dense,
            None,
            RetrieveWeights::default(),
            &provider,
        )
        .expect("retrieve succeeds");

        assert!(!results.is_empty());
        assert_eq!(results[0].result.title, "PKMS Task Backend");
        assert_eq!(results[0].result.scores.bm25, 0.0);
        assert!(results[0].result.scores.dense > 0.0);
        assert!(results[0].reason.contains("dense"));
    }

    #[test]
    fn retrieve_trims_to_token_budget_deterministically() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let path = tempdir.path().join("rag.sqlite3");
        let mut conn = connect(&path).expect("connect initializes database");
        let provider = HashEmbeddingProvider::default();
        ingest_records(&mut conn, &fixture_records(), &provider, false).expect("fixture ingests");

        let results = retrieve_results(
            &conn,
            "externalHostname",
            5,
            RetrieveMode::Bm25,
            Some(1),
            RetrieveWeights::default(),
            &provider,
        )
        .expect("retrieve succeeds");

        assert_eq!(results.len(), 1);
        assert_eq!(
            results[0].result.title,
            "Media Library Migration to Jellyfin"
        );
    }

    #[test]
    fn retrieve_response_preserves_query_and_mode() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let path = tempdir.path().join("rag.sqlite3");
        let mut conn = connect(&path).expect("connect initializes database");
        let provider = HashEmbeddingProvider::default();
        ingest_records(&mut conn, &fixture_records(), &provider, false).expect("fixture ingests");
        let request = RetrieveRequest {
            query: "externalHostname".to_string(),
            limit: 1,
            mode: RetrieveMode::Hybrid,
            max_token_budget: None,
            weights: RetrieveWeights::default(),
        };

        let response = retrieve(&conn, &request, &provider).expect("retrieve succeeds");

        assert_eq!(response.query, "externalHostname");
        assert_eq!(response.mode, RetrieveMode::Hybrid);
        assert_eq!(response.results.len(), 1);
    }
}
