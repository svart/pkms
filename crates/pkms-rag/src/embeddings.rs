use std::mem::{size_of, size_of_val};

use anyhow::{Result, bail};
use blake2::{
    Blake2bVar,
    digest::{Update, VariableOutput},
};

use crate::models::ChunkRecord;

pub const DEFAULT_HASH_EMBEDDING_MODEL: &str = "hashing-v1";
pub const DEFAULT_HASH_EMBEDDING_DIMENSION: usize = 384;
pub const DEFAULT_EMBEDDING_MAX_BODY_CHARS: usize = 8000;

pub trait EmbeddingProvider {
    fn model_name(&self) -> &str;
    fn dimension(&self) -> usize;
    fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HashEmbeddingProvider {
    model_name: String,
    dimension: usize,
}

impl HashEmbeddingProvider {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_dimension(dimension: usize) -> Self {
        assert!(dimension > 0, "hash embedding dimension must be positive");
        Self {
            model_name: DEFAULT_HASH_EMBEDDING_MODEL.to_string(),
            dimension,
        }
    }

    fn embed_one(&self, text: &str) -> Vec<f32> {
        let mut vector = vec![0.0_f32; self.dimension];
        let normalized_text = text.to_lowercase();
        for token in normalized_text.split_whitespace() {
            let digest = blake2b_8(token.as_bytes());
            let index = u32::from_le_bytes(digest[0..4].try_into().expect("digest length is fixed"))
                as usize
                % self.dimension;
            let sign = if digest[4] & 1 == 1 { 1.0 } else { -1.0 };
            vector[index] += sign;
        }
        normalize_vector(vector)
    }
}

impl Default for HashEmbeddingProvider {
    fn default() -> Self {
        Self::with_dimension(DEFAULT_HASH_EMBEDDING_DIMENSION)
    }
}

impl EmbeddingProvider for HashEmbeddingProvider {
    fn model_name(&self) -> &str {
        &self.model_name
    }

    fn dimension(&self) -> usize {
        self.dimension
    }

    fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        Ok(texts.iter().map(|text| self.embed_one(text)).collect())
    }
}

pub fn embedding_text(chunk: &ChunkRecord) -> String {
    embedding_text_with_max_body_chars(chunk, embedding_max_body_chars_from_env())
}

fn embedding_text_with_max_body_chars(chunk: &ChunkRecord, max_body_chars: usize) -> String {
    let body = chunk.body.chars().take(max_body_chars).collect::<String>();
    [
        chunk.title.clone(),
        chunk.aliases.join(" "),
        chunk.tags.join(" "),
        chunk.heading_path.join(" / "),
        body,
    ]
    .into_iter()
    .filter(|part| !part.is_empty())
    .collect::<Vec<_>>()
    .join("\n")
}

fn embedding_max_body_chars_from_env() -> usize {
    std::env::var("PKMS_RAG_EMBEDDING_MAX_BODY_CHARS")
        .ok()
        .and_then(|value| value.parse().ok())
        .filter(|value| *value > 0)
        .unwrap_or(DEFAULT_EMBEDDING_MAX_BODY_CHARS)
}

pub fn pack_vector(vector: &[f32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(size_of_val(vector));
    for value in vector {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes
}

pub fn unpack_vector(bytes: &[u8]) -> Result<Vec<f32>> {
    if !bytes.len().is_multiple_of(size_of::<f32>()) {
        bail!(
            "invalid vector blob length {}; expected a multiple of {}",
            bytes.len(),
            size_of::<f32>()
        );
    }
    Ok(bytes
        .chunks_exact(size_of::<f32>())
        .map(|chunk| f32::from_le_bytes(chunk.try_into().expect("chunk size is checked")))
        .collect())
}

pub fn cosine_similarity(left: &[f32], right: &[f32]) -> f32 {
    if left.is_empty() || left.len() != right.len() {
        return 0.0;
    }

    let mut dot = 0.0_f32;
    let mut left_norm = 0.0_f32;
    let mut right_norm = 0.0_f32;
    for (left_value, right_value) in left.iter().zip(right) {
        dot += left_value * right_value;
        left_norm += left_value * left_value;
        right_norm += right_value * right_value;
    }
    if left_norm == 0.0 || right_norm == 0.0 {
        return 0.0;
    }
    dot / (left_norm.sqrt() * right_norm.sqrt())
}

fn normalize_vector(mut vector: Vec<f32>) -> Vec<f32> {
    let norm = vector.iter().map(|value| value * value).sum::<f32>().sqrt();
    if norm == 0.0 {
        return vector;
    }
    for value in &mut vector {
        *value /= norm;
    }
    vector
}

fn blake2b_8(input: &[u8]) -> [u8; 8] {
    let mut hasher = Blake2bVar::new(8).expect("valid BLAKE2b output size");
    hasher.update(input);
    let mut digest = [0_u8; 8];
    hasher
        .finalize_variable(&mut digest)
        .expect("digest buffer has requested size");
    digest
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{ChunkRecord, SUPPORTED_SCHEMA_VERSION};

    #[test]
    fn embeddings_hash_provider_exposes_python_metadata() {
        let provider = HashEmbeddingProvider::default();

        assert_eq!(provider.model_name(), "hashing-v1");
        assert_eq!(provider.dimension(), 384);
    }

    #[test]
    fn embeddings_hash_provider_is_deterministic_and_python_compatible() {
        let provider = HashEmbeddingProvider::default();
        let input = vec!["agenda inspect tasks".to_string()];

        let first = provider.embed(&input).expect("embedding succeeds");
        let second = provider.embed(&input).expect("embedding succeeds");

        assert_eq!(first, second);
        assert_eq!(
            non_zero_entries(&first[0]),
            vec![(185, 0.57735026), (188, 0.57735026), (237, -0.57735026)]
        );
    }

    #[test]
    fn embeddings_hash_provider_normalizes_non_empty_vectors() {
        let provider = HashEmbeddingProvider::default();
        let vectors = provider
            .embed(&["pkms task agenda today".to_string()])
            .expect("embedding succeeds");

        let norm = vectors[0]
            .iter()
            .map(|value| value * value)
            .sum::<f32>()
            .sqrt();

        assert!((norm - 1.0).abs() < 0.000001);
    }

    #[test]
    fn embeddings_hash_provider_returns_zero_vector_for_empty_text() {
        let provider = HashEmbeddingProvider::default();
        let vectors = provider
            .embed(&[String::new()])
            .expect("embedding succeeds");

        assert_eq!(vectors[0].len(), DEFAULT_HASH_EMBEDDING_DIMENSION);
        assert!(vectors[0].iter().all(|value| *value == 0.0));
    }

    #[test]
    fn embeddings_text_includes_metadata_and_body() {
        let chunk = sample_chunk("Chunk body");

        let text = embedding_text_with_max_body_chars(&chunk, DEFAULT_EMBEDDING_MAX_BODY_CHARS);

        assert_eq!(
            text,
            "PKMS Task Backend\nTask backend\npkms tasks\nTodoist agenda\nChunk body"
        );
    }

    #[test]
    fn embeddings_text_truncates_body_by_character_count() {
        let chunk = sample_chunk("abcde");

        let text = embedding_text_with_max_body_chars(&chunk, 3);

        assert!(text.ends_with("abc"));
        assert!(!text.contains("de"));
    }

    fn sample_chunk(body: &str) -> ChunkRecord {
        ChunkRecord {
            schema_version: SUPPORTED_SCHEMA_VERSION,
            chunk_id: "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa:todoist-agenda:def456".to_string(),
            note_id: "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa".to_string(),
            path: "tasks/pkms-task.org".to_string(),
            title: "PKMS Task Backend".to_string(),
            aliases: vec!["Task backend".to_string()],
            tags: vec!["pkms".to_string(), "tasks".to_string()],
            heading_path: vec!["Todoist agenda".to_string()],
            heading_level: 1,
            body: body.to_string(),
            start_line: 10,
            end_line: 16,
            outgoing_ids: Vec::new(),
            updated_at: 1781686801,
            content_hash: "sha256:chunk-task".to_string(),
        }
    }

    fn non_zero_entries(vector: &[f32]) -> Vec<(usize, f32)> {
        vector
            .iter()
            .enumerate()
            .filter_map(|(index, value)| {
                if *value == 0.0 {
                    None
                } else {
                    Some((index, (*value * 100000000.0).round() / 100000000.0))
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod vector {
    use super::*;

    #[test]
    fn vector_pack_uses_little_endian_f32_bytes() {
        let vector = [1.0_f32, -2.5_f32];

        let bytes = pack_vector(&vector);

        let mut expected = Vec::new();
        expected.extend_from_slice(&1.0_f32.to_le_bytes());
        expected.extend_from_slice(&(-2.5_f32).to_le_bytes());
        assert_eq!(bytes, expected);
    }

    #[test]
    fn vector_unpack_round_trips_packed_vectors() {
        let vector = [0.0_f32, 1.5_f32, -3.25_f32, std::f32::consts::PI];

        let unpacked = unpack_vector(&pack_vector(&vector)).expect("packed vector unpacks");

        assert_eq!(unpacked, vector.to_vec());
    }

    #[test]
    fn vector_unpack_rejects_invalid_blob_lengths() {
        let err = unpack_vector(&[1, 2, 3]).expect_err("invalid length is rejected");

        assert!(err.to_string().contains("invalid vector blob length 3"));
    }

    #[test]
    fn vector_cosine_similarity_handles_common_cases() {
        assert_eq!(cosine_similarity(&[1.0, 0.0], &[1.0, 0.0]), 1.0);
        assert_eq!(cosine_similarity(&[1.0, 0.0], &[0.0, 1.0]), 0.0);
        assert_eq!(cosine_similarity(&[0.0, 0.0], &[1.0, 0.0]), 0.0);
        assert_eq!(cosine_similarity(&[1.0, 0.0], &[1.0]), 0.0);
    }
}
