use std::mem::{size_of, size_of_val};

use anyhow::{Result, bail};

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
