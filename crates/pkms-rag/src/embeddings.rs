use std::{
    mem::{size_of, size_of_val},
    path::{Path, PathBuf},
};

#[cfg(feature = "fastembed")]
use anyhow::Context;
use anyhow::{Result, bail};
use blake2::{
    Blake2bVar,
    digest::{Update, VariableOutput},
};
#[cfg(feature = "fastembed")]
use fastembed::{
    EmbeddingModel, InitOptionsUserDefined, TextEmbedding, TextInitOptions, TokenizerFiles,
    UserDefinedEmbeddingModel,
};
#[cfg(feature = "fastembed")]
use std::sync::Mutex;

use crate::models::ChunkRecord;

pub const DEFAULT_HASH_EMBEDDING_MODEL: &str = "hashing-v1";
pub const DEFAULT_HASH_EMBEDDING_DIMENSION: usize = 384;
pub const DEFAULT_EMBEDDING_MAX_BODY_CHARS: usize = 8000;
pub const DEFAULT_FASTEMBED_MODEL: &str =
    "sentence-transformers/paraphrase-multilingual-MiniLM-L12-v2";
pub const DEFAULT_FASTEMBED_BATCH_SIZE: usize = 256;

#[cfg(feature = "fastembed")]
const DEFAULT_FASTEMBED_MODEL_CODE: &str = "Xenova/paraphrase-multilingual-MiniLM-L12-v2";

const HASH_PROVIDER_NAME: &str = "hash";
const FASTEMBED_PROVIDER_NAME: &str = "fastembed";
const EMBEDDING_PROVIDER_ENV: &str = "PKMS_RAG_EMBEDDING_PROVIDER";
const EMBEDDING_MODEL_ENV: &str = "PKMS_RAG_EMBEDDING_MODEL";
const FASTEMBED_MODEL_DIR_ENV: &str = "PKMS_RAG_FASTEMBED_MODEL_DIR";

pub trait EmbeddingProvider {
    fn model_name(&self) -> &str;
    fn dimension(&self) -> usize;
    fn preferred_batch_size(&self) -> Option<usize> {
        None
    }
    fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EmbeddingProviderConfig {
    Hash,
    FastEmbed {
        model_name: String,
        batch_size: usize,
        model_dir: Option<PathBuf>,
    },
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

#[cfg(feature = "fastembed")]
pub struct FastEmbeddingProvider {
    model_name: String,
    dimension: usize,
    batch_size: usize,
    model: Mutex<TextEmbedding>,
}

#[cfg(feature = "fastembed")]
impl FastEmbeddingProvider {
    pub fn try_new(model_name: &str, batch_size: usize, model_dir: Option<&Path>) -> Result<Self> {
        anyhow::ensure!(batch_size > 0, "FastEmbed batch size must be positive");
        let model = parse_fastembed_model(model_name)?;
        let info = TextEmbedding::get_model_info(&model)
            .with_context(|| format!("failed to read FastEmbed model info for {model_name}"))?;
        let normalized_model_name = info.model_code.clone();
        let dimension = info.dim;
        let model = match model_dir {
            Some(model_dir) => text_embedding_from_model_dir(&model, model_dir),
            None => TextEmbedding::try_new(TextInitOptions::new(model)),
        }
        .with_context(|| format!("failed to initialize FastEmbed model {normalized_model_name}"))?;
        Ok(Self {
            model_name: normalized_model_name,
            dimension,
            batch_size,
            model: Mutex::new(model),
        })
    }
}

#[cfg(feature = "fastembed")]
impl EmbeddingProvider for FastEmbeddingProvider {
    fn model_name(&self) -> &str {
        &self.model_name
    }

    fn dimension(&self) -> usize {
        self.dimension
    }

    fn preferred_batch_size(&self) -> Option<usize> {
        Some(self.batch_size)
    }

    fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        let mut model = self
            .model
            .lock()
            .map_err(|_| anyhow::anyhow!("FastEmbed model lock is poisoned"))?;
        model.embed(texts, Some(self.batch_size)).with_context(|| {
            format!(
                "failed to embed texts with FastEmbed model {}",
                self.model_name
            )
        })
    }
}

pub fn embedding_provider_config_from_env() -> Result<EmbeddingProviderConfig> {
    embedding_provider_config_from_env_with_model(None)
}

pub fn embedding_provider_config_from_env_with_model(
    configured_model_name: Option<&str>,
) -> Result<EmbeddingProviderConfig> {
    embedding_provider_config_from_env_with_model_and_dir(configured_model_name, None)
}

pub fn embedding_provider_config_from_env_with_model_and_dir(
    configured_model_name: Option<&str>,
    configured_model_dir: Option<&Path>,
) -> Result<EmbeddingProviderConfig> {
    let provider = std::env::var(EMBEDDING_PROVIDER_ENV)
        .ok()
        .map(|provider| provider.trim().to_ascii_lowercase())
        .filter(|provider| !provider.is_empty())
        .unwrap_or_else(|| FASTEMBED_PROVIDER_NAME.to_string());
    match provider.as_str() {
        HASH_PROVIDER_NAME | "hashing-v1" => Ok(EmbeddingProviderConfig::Hash),
        FASTEMBED_PROVIDER_NAME => Ok(EmbeddingProviderConfig::FastEmbed {
            model_name: std::env::var(EMBEDDING_MODEL_ENV)
                .ok()
                .map(|model| model.trim().to_string())
                .filter(|model| !model.is_empty())
                .or_else(|| {
                    configured_model_name
                        .map(str::trim)
                        .filter(|model| !model.is_empty())
                        .map(ToOwned::to_owned)
                })
                .unwrap_or_else(|| DEFAULT_FASTEMBED_MODEL.to_string()),
            batch_size: DEFAULT_FASTEMBED_BATCH_SIZE,
            model_dir: std::env::var(FASTEMBED_MODEL_DIR_ENV)
                .ok()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
                .map(PathBuf::from)
                .or_else(|| configured_model_dir.map(Path::to_path_buf)),
        }),
        _ => bail!(
            "unsupported embedding provider '{}'; expected '{}' or '{}'",
            provider,
            HASH_PROVIDER_NAME,
            FASTEMBED_PROVIDER_NAME
        ),
    }
}

pub fn provider_from_env() -> Result<Box<dyn EmbeddingProvider>> {
    let config = embedding_provider_config_from_env()?;
    provider_from_config(&config)
}

pub fn provider_from_config(
    config: &EmbeddingProviderConfig,
) -> Result<Box<dyn EmbeddingProvider>> {
    match config {
        EmbeddingProviderConfig::Hash => Ok(Box::new(HashEmbeddingProvider::default())),
        EmbeddingProviderConfig::FastEmbed {
            model_name,
            batch_size,
            model_dir,
        } => fastembed_provider_boxed(model_name, *batch_size, model_dir.as_deref()),
    }
}

#[cfg(feature = "fastembed")]
fn fastembed_provider_boxed(
    model_name: &str,
    batch_size: usize,
    model_dir: Option<&Path>,
) -> Result<Box<dyn EmbeddingProvider>> {
    Ok(Box::new(FastEmbeddingProvider::try_new(
        model_name, batch_size, model_dir,
    )?))
}

#[cfg(not(feature = "fastembed"))]
fn fastembed_provider_boxed(
    model_name: &str,
    _batch_size: usize,
    _model_dir: Option<&Path>,
) -> Result<Box<dyn EmbeddingProvider>> {
    bail!(
        "FastEmbed provider requested for model '{}', but pkms-rag was built without the 'fastembed' feature",
        model_name
    )
}

#[cfg(feature = "fastembed")]
fn text_embedding_from_model_dir(
    model: &EmbeddingModel,
    model_dir: &Path,
) -> Result<TextEmbedding> {
    let info = TextEmbedding::get_model_info(model)?;
    let tokenizer_files = TokenizerFiles {
        tokenizer_file: read_fastembed_model_file(model_dir, "tokenizer.json")?,
        config_file: read_fastembed_model_file(model_dir, "config.json")?,
        special_tokens_map_file: read_fastembed_model_file(model_dir, "special_tokens_map.json")?,
        tokenizer_config_file: read_fastembed_model_file(model_dir, "tokenizer_config.json")?,
    };
    let mut model_files = UserDefinedEmbeddingModel::new(
        read_fastembed_model_file(model_dir, &info.model_file)?,
        tokenizer_files,
    )
    .with_quantization(TextEmbedding::get_quantization_mode(model));
    if let Some(pooling) = TextEmbedding::get_default_pooling_method(model) {
        model_files = model_files.with_pooling(pooling);
    }
    for additional_file in &info.additional_files {
        model_files = model_files.with_external_initializer(
            additional_file.clone(),
            read_fastembed_model_file(model_dir, additional_file)?,
        );
    }
    model_files.output_key = info.output_key.clone();
    TextEmbedding::try_new_from_user_defined(model_files, InitOptionsUserDefined::default())
}

#[cfg(feature = "fastembed")]
fn read_fastembed_model_file(model_dir: &Path, relative_path: &str) -> Result<Vec<u8>> {
    let path = model_dir.join(relative_path);
    std::fs::read(&path)
        .with_context(|| format!("failed to read FastEmbed model file {}", path.display()))
}

pub fn embedding_text(chunk: &ChunkRecord) -> String {
    embedding_text_with_max_body_chars(chunk, DEFAULT_EMBEDDING_MAX_BODY_CHARS)
}

pub(crate) fn embedding_text_with_max_body_chars(
    chunk: &ChunkRecord,
    max_body_chars: usize,
) -> String {
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

#[cfg(feature = "fastembed")]
fn parse_fastembed_model(model_name: &str) -> Result<EmbeddingModel> {
    if let Ok(model) = model_name.parse::<EmbeddingModel>() {
        return Ok(model);
    }
    let model_name = fastembed_model_alias(model_name).unwrap_or(model_name);
    TextEmbedding::list_supported_models()
        .into_iter()
        .find(|info| info.model_code.eq_ignore_ascii_case(model_name))
        .map(|info| info.model)
        .ok_or_else(|| anyhow::anyhow!("unsupported FastEmbed model '{}'", model_name))
}

#[cfg(feature = "fastembed")]
fn fastembed_model_alias(model_name: &str) -> Option<&'static str> {
    match model_name {
        "sentence-transformers/paraphrase-multilingual-MiniLM-L12-v2" => {
            Some(DEFAULT_FASTEMBED_MODEL_CODE)
        }
        "BAAI/bge-small-en-v1.5" => Some("Xenova/bge-small-en-v1.5"),
        _ => None,
    }
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

    #[test]
    fn embeddings_text_ignores_removed_max_body_chars_env() {
        let _guard = ENV_LOCK.lock().expect("env lock");
        let _snapshot = EnvSnapshot::capture();
        clear_embedding_env();
        set_env("PKMS_RAG_EMBEDDING_MAX_BODY_CHARS", "3");
        let chunk = sample_chunk("abcde");

        let text = embedding_text(&chunk);

        assert!(text.ends_with("abcde"));
    }

    #[test]
    fn embeddings_provider_config_defaults_to_fastembed() {
        let _guard = ENV_LOCK.lock().expect("env lock");
        let _snapshot = EnvSnapshot::capture();
        clear_embedding_env();

        let config = embedding_provider_config_from_env().expect("config reads");

        assert_eq!(
            config,
            EmbeddingProviderConfig::FastEmbed {
                model_name: DEFAULT_FASTEMBED_MODEL.to_string(),
                batch_size: DEFAULT_FASTEMBED_BATCH_SIZE,
                model_dir: None,
            }
        );
    }

    #[test]
    fn embeddings_provider_config_selects_hash_provider() {
        let _guard = ENV_LOCK.lock().expect("env lock");
        let _snapshot = EnvSnapshot::capture();
        clear_embedding_env();
        set_env(EMBEDDING_PROVIDER_ENV, "hash");

        let config = embedding_provider_config_from_env().expect("config reads");
        let provider = provider_from_env().expect("hash provider builds");

        assert_eq!(config, EmbeddingProviderConfig::Hash);
        assert_eq!(provider.model_name(), DEFAULT_HASH_EMBEDDING_MODEL);
        assert_eq!(provider.dimension(), DEFAULT_HASH_EMBEDDING_DIMENSION);
    }

    #[test]
    fn embeddings_provider_config_reads_fastembed_model_and_dir() {
        let _guard = ENV_LOCK.lock().expect("env lock");
        let _snapshot = EnvSnapshot::capture();
        clear_embedding_env();
        set_env(EMBEDDING_PROVIDER_ENV, "fastembed");
        set_env(EMBEDDING_MODEL_ENV, "Xenova/all-MiniLM-L12-v2");
        set_env("PKMS_RAG_EMBEDDING_BATCH_SIZE", "7");
        set_env(
            "PKMS_RAG_FASTEMBED_MODEL_DIR",
            "/opt/pkms/models/all-minilm",
        );

        let config = embedding_provider_config_from_env().expect("config reads");

        assert_eq!(
            config,
            EmbeddingProviderConfig::FastEmbed {
                model_name: "Xenova/all-MiniLM-L12-v2".to_string(),
                batch_size: DEFAULT_FASTEMBED_BATCH_SIZE,
                model_dir: Some("/opt/pkms/models/all-minilm".into()),
            }
        );
    }

    #[test]
    fn embeddings_provider_config_uses_configured_model_when_env_model_absent() {
        let _guard = ENV_LOCK.lock().expect("env lock");
        let _snapshot = EnvSnapshot::capture();
        clear_embedding_env();
        set_env(EMBEDDING_PROVIDER_ENV, "fastembed");

        let config =
            embedding_provider_config_from_env_with_model(Some("Xenova/bge-small-en-v1.5"))
                .expect("config reads");

        assert_eq!(
            config,
            EmbeddingProviderConfig::FastEmbed {
                model_name: "Xenova/bge-small-en-v1.5".to_string(),
                batch_size: DEFAULT_FASTEMBED_BATCH_SIZE,
                model_dir: None,
            }
        );
    }

    #[test]
    fn embeddings_provider_config_uses_configured_model_dir_when_env_dir_absent() {
        let _guard = ENV_LOCK.lock().expect("env lock");
        let _snapshot = EnvSnapshot::capture();
        clear_embedding_env();
        set_env(EMBEDDING_PROVIDER_ENV, "fastembed");

        let config = embedding_provider_config_from_env_with_model_and_dir(
            None,
            Some(Path::new("/srv/pkms/models/bge-small")),
        )
        .expect("config reads");

        assert_eq!(
            config,
            EmbeddingProviderConfig::FastEmbed {
                model_name: DEFAULT_FASTEMBED_MODEL.to_string(),
                batch_size: DEFAULT_FASTEMBED_BATCH_SIZE,
                model_dir: Some("/srv/pkms/models/bge-small".into()),
            }
        );
    }

    #[test]
    fn embeddings_provider_config_prefers_env_model_dir_over_configured_model_dir() {
        let _guard = ENV_LOCK.lock().expect("env lock");
        let _snapshot = EnvSnapshot::capture();
        clear_embedding_env();
        set_env(EMBEDDING_PROVIDER_ENV, "fastembed");
        set_env(FASTEMBED_MODEL_DIR_ENV, "/env/pkms/models");

        let config = embedding_provider_config_from_env_with_model_and_dir(
            None,
            Some(Path::new("/config/pkms/models")),
        )
        .expect("config reads");

        assert_eq!(
            config,
            EmbeddingProviderConfig::FastEmbed {
                model_name: DEFAULT_FASTEMBED_MODEL.to_string(),
                batch_size: DEFAULT_FASTEMBED_BATCH_SIZE,
                model_dir: Some("/env/pkms/models".into()),
            }
        );
    }

    #[test]
    fn embeddings_provider_config_prefers_env_model_over_configured_model() {
        let _guard = ENV_LOCK.lock().expect("env lock");
        let _snapshot = EnvSnapshot::capture();
        clear_embedding_env();
        set_env(EMBEDDING_PROVIDER_ENV, "fastembed");
        set_env(EMBEDDING_MODEL_ENV, "Xenova/all-MiniLM-L12-v2");

        let config =
            embedding_provider_config_from_env_with_model(Some("Xenova/bge-small-en-v1.5"))
                .expect("config reads");

        assert_eq!(
            config,
            EmbeddingProviderConfig::FastEmbed {
                model_name: "Xenova/all-MiniLM-L12-v2".to_string(),
                batch_size: DEFAULT_FASTEMBED_BATCH_SIZE,
                model_dir: None,
            }
        );
    }

    #[test]
    fn embeddings_provider_config_rejects_unknown_provider() {
        let _guard = ENV_LOCK.lock().expect("env lock");
        let _snapshot = EnvSnapshot::capture();
        clear_embedding_env();
        set_env(EMBEDDING_PROVIDER_ENV, "unknown");

        let err = embedding_provider_config_from_env().expect_err("provider is rejected");

        assert!(err.to_string().contains("unsupported embedding provider"));
    }

    #[cfg(feature = "fastembed")]
    #[test]
    fn embeddings_fastembed_model_info_reads_without_model_instantiation() {
        let model = parse_fastembed_model(DEFAULT_FASTEMBED_MODEL).expect("model parses");
        let info = TextEmbedding::get_model_info(&model).expect("model info reads");

        assert_eq!(info.model_code, DEFAULT_FASTEMBED_MODEL_CODE);
        assert!(info.dim > 0);
    }

    #[cfg(feature = "fastembed")]
    #[test]
    fn embeddings_fastembed_model_dir_reads_local_files() {
        let model_dir = tempfile::tempdir().expect("tempdir creates");

        let err = match FastEmbeddingProvider::try_new(
            DEFAULT_FASTEMBED_MODEL,
            DEFAULT_FASTEMBED_BATCH_SIZE,
            Some(model_dir.path()),
        ) {
            Ok(_) => panic!("missing local model files should fail"),
            Err(err) => err,
        };

        let message = format!("{err:#}");
        assert!(message.contains("failed to read FastEmbed model file"));
        assert!(message.contains("tokenizer.json"));
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

    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    struct EnvSnapshot {
        values: Vec<(&'static str, Option<String>)>,
    }

    impl EnvSnapshot {
        fn capture() -> Self {
            Self {
                values: vec![
                    (
                        EMBEDDING_PROVIDER_ENV,
                        std::env::var(EMBEDDING_PROVIDER_ENV).ok(),
                    ),
                    (EMBEDDING_MODEL_ENV, std::env::var(EMBEDDING_MODEL_ENV).ok()),
                    (
                        "PKMS_RAG_EMBEDDING_BATCH_SIZE",
                        std::env::var("PKMS_RAG_EMBEDDING_BATCH_SIZE").ok(),
                    ),
                    (
                        "PKMS_RAG_EMBEDDING_MAX_BODY_CHARS",
                        std::env::var("PKMS_RAG_EMBEDDING_MAX_BODY_CHARS").ok(),
                    ),
                    (
                        "PKMS_RAG_FASTEMBED_MODEL_DIR",
                        std::env::var("PKMS_RAG_FASTEMBED_MODEL_DIR").ok(),
                    ),
                ],
            }
        }
    }

    impl Drop for EnvSnapshot {
        fn drop(&mut self) {
            for (key, value) in &self.values {
                match value {
                    Some(value) => set_env(key, value),
                    None => remove_env(key),
                }
            }
        }
    }

    fn clear_embedding_env() {
        remove_env(EMBEDDING_PROVIDER_ENV);
        remove_env(EMBEDDING_MODEL_ENV);
        remove_env("PKMS_RAG_EMBEDDING_BATCH_SIZE");
        remove_env("PKMS_RAG_EMBEDDING_MAX_BODY_CHARS");
        remove_env("PKMS_RAG_FASTEMBED_MODEL_DIR");
    }

    fn set_env(key: &str, value: &str) {
        // SAFETY: these tests serialize environment changes with ENV_LOCK and restore values.
        unsafe { std::env::set_var(key, value) };
    }

    fn remove_env(key: &str) {
        // SAFETY: these tests serialize environment changes with ENV_LOCK and restore values.
        unsafe { std::env::remove_var(key) };
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
