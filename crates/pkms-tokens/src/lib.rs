//! Token encoding, counting, and truncation primitives for pkms.

use std::str::FromStr;
use tiktoken_rs::CoreBPE;
use tiktoken_rs::cl100k_base_singleton;
use tiktoken_rs::o200k_base_singleton;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Encoding {
    Cl100kBase,
    O200kBase,
}

impl FromStr for Encoding {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "cl100k_base" | "cl100k" => Ok(Self::Cl100kBase),
            "o200k_base" | "o200k" => Ok(Self::O200kBase),
            _ => Err(()),
        }
    }
}

impl std::fmt::Display for Encoding {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Cl100kBase => write!(f, "cl100k_base"),
            Self::O200kBase => write!(f, "o200k_base"),
        }
    }
}

pub fn bpe_for_encoding(encoding: Encoding) -> &'static CoreBPE {
    match encoding {
        Encoding::Cl100kBase => cl100k_base_singleton(),
        Encoding::O200kBase => o200k_base_singleton(),
    }
}

pub fn count_tokens(text: &str, encoding: Encoding) -> usize {
    let bpe = bpe_for_encoding(encoding);
    bpe.count_with_special_tokens(text)
}

pub fn truncate_by_tokens(text: &str, max_tokens: usize, encoding: Encoding) -> String {
    let bpe = bpe_for_encoding(encoding);
    let tokens = bpe.encode_with_special_tokens(text);
    if tokens.len() <= max_tokens {
        return text.to_string();
    }
    bpe.decode(&tokens[..max_tokens]).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_count_tokens_empty() {
        assert_eq!(count_tokens("", Encoding::Cl100kBase), 0);
    }

    #[test]
    fn test_count_tokens_short() {
        let count = count_tokens("hello world", Encoding::Cl100kBase);
        assert!(count > 0);
    }

    #[test]
    fn test_count_tokens_cjk() {
        let count = count_tokens("你好世界", Encoding::Cl100kBase);
        assert!(count > 0);
    }

    #[test]
    fn test_truncate_by_tokens_short() {
        let text = "hello world this is a test";
        let truncated = truncate_by_tokens(text, 100, Encoding::Cl100kBase);
        assert_eq!(truncated, text);
    }

    #[test]
    fn test_truncate_by_tokens_long() {
        let text = "aaaa bbbb cccc dddd";
        let truncated = truncate_by_tokens(text, 2, Encoding::Cl100kBase);
        assert!(truncated.len() < text.len());
    }

    #[test]
    fn test_encoding_from_str() {
        assert_eq!("cl100k_base".parse::<Encoding>(), Ok(Encoding::Cl100kBase));
        assert_eq!("cl100k".parse::<Encoding>(), Ok(Encoding::Cl100kBase));
        assert_eq!("CL100K".parse::<Encoding>(), Ok(Encoding::Cl100kBase));
        assert_eq!("o200k_base".parse::<Encoding>(), Ok(Encoding::O200kBase));
        assert_eq!("o200k".parse::<Encoding>(), Ok(Encoding::O200kBase));
        assert!("invalid".parse::<Encoding>().is_err());
    }
}
