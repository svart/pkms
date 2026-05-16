use anyhow::Result;
use fastembed::{EmbeddingModel, TextEmbedding, TextInitOptions};
use std::path::PathBuf;

fn cache_dir() -> PathBuf {
    dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("pkms")
}

fn model_dir() -> PathBuf {
    cache_dir().join("models--Xenova--bge-small-en-v1.5")
}

fn model_is_cached() -> bool {
    model_dir().exists()
}

fn create_model() -> Result<TextEmbedding> {
    if !model_is_cached() {
        eprintln!("Downloading model...");
    }
    let cache = cache_dir();
    std::fs::create_dir_all(&cache).ok();
    let options = TextInitOptions::new(EmbeddingModel::BGESmallENV15)
        .with_show_download_progress(false)
        .with_cache_dir(cache);
    TextEmbedding::try_new(options)
        .map_err(|e| anyhow::anyhow!("Failed to create embedding model: {e}"))
}

pub fn compute_embeddings(texts: &[String]) -> Result<Vec<Vec<f32>>> {
    let mut model = create_model()?;
    let docs: Vec<&str> = texts.iter().map(|s| s.as_str()).collect();
    let embeddings = model.embed(docs, None)?;
    Ok(embeddings)
}

pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f64 {
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }
    (dot / (norm_a * norm_b)) as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cosine_similarity_identical() {
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![1.0, 2.0, 3.0];
        let similarity = cosine_similarity(&a, &b);
        assert!((similarity - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_cosine_similarity_orthogonal() {
        let a = vec![1.0, 0.0];
        let b = vec![0.0, 1.0];
        let similarity = cosine_similarity(&a, &b);
        assert!((similarity - 0.0).abs() < 1e-6);
    }

    #[test]
    fn test_cosine_similarity_opposite() {
        let a = vec![1.0, 2.0];
        let b = vec![-1.0, -2.0];
        let similarity = cosine_similarity(&a, &b);
        assert!((similarity - (-1.0)).abs() < 1e-6);
    }

    #[test]
    fn test_cosine_similarity_zero_vector() {
        let a = vec![0.0, 0.0];
        let b = vec![1.0, 2.0];
        let similarity = cosine_similarity(&a, &b);
        assert!((similarity - 0.0).abs() < 1e-6);
    }

    #[test]
    fn test_cosine_similarity_positive() {
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![2.0, 3.0, 4.0];
        let similarity = cosine_similarity(&a, &b);
        assert!(similarity > 0.9);
        assert!(similarity < 1.0);
    }
}
