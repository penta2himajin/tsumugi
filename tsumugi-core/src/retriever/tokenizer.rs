//! Tokenizer abstraction. The default `WhitespaceTokenizer` lowercases and
//! splits on whitespace + punctuation, which is sufficient for ASCII and
//! CJK-mixed text at a basic level. Japanese-specific tokenization (lindera)
//! is planned for Phase 2 behind a feature flag.

pub trait Tokenizer: Send + Sync {
    fn tokenize(&self, text: &str) -> Vec<String>;
}

pub struct WhitespaceTokenizer;

impl Tokenizer for WhitespaceTokenizer {
    fn tokenize(&self, text: &str) -> Vec<String> {
        text.split(|c: char| c.is_whitespace() || c.is_ascii_punctuation())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_lowercase())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_on_whitespace_and_punctuation() {
        let t = WhitespaceTokenizer;
        let toks = t.tokenize("Hello, world! This is tsumugi.");
        assert_eq!(toks, vec!["hello", "world", "this", "is", "tsumugi"]);
    }

    #[test]
    fn empty_string_yields_empty() {
        let t = WhitespaceTokenizer;
        assert!(t.tokenize("").is_empty());
    }
}
