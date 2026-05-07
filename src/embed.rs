use anyhow::Result;
use fastembed::{EmbeddingModel, TextEmbedding, TextInitOptions};

fn create_model() -> Result<TextEmbedding> {
    let options =
        TextInitOptions::new(EmbeddingModel::BGESmallENV15).with_show_download_progress(false);
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
