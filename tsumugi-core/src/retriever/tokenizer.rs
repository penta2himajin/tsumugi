//! Tokenizer abstraction.
//!
//! - `WhitespaceTokenizer` (always available): splits on whitespace and ASCII
//!   punctuation, lowercases. Sufficient for ASCII-only corpora.
//! - `JapaneseCharTokenizer` (always available): runs-of-same-script splitter
//!   that handles hiragana / katakana / CJK unified ideographs / ASCII words
//!   without external dictionaries. Produces CJK bi-grams for kanji runs so
//!   single-character BM25 does not explode. A pragmatic middle ground until
//!   full lindera integration lands (Phase 3).

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

/// Dict-free Japanese tokenizer. Breaks a string into same-script runs
/// (`Hiragana` / `Katakana` / `Han` / `Latin` / `Digit`), and for `Han` runs
/// emits bi-grams (`ab`, `bc`, ...) so BM25 on Japanese text doesn't reduce to
/// per-character noise. Other runs are kept as whole tokens, lowercased for
/// Latin.
pub struct JapaneseCharTokenizer;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Script {
    Hiragana,
    Katakana,
    Han,
    Latin,
    Digit,
    Other,
}

fn classify(c: char) -> Script {
    match c {
        '\u{3040}'..='\u{309F}' => Script::Hiragana,
        '\u{30A0}'..='\u{30FF}' => Script::Katakana,
        '\u{4E00}'..='\u{9FFF}' => Script::Han,
        '\u{3400}'..='\u{4DBF}' => Script::Han, // CJK Extension A
        c if c.is_ascii_alphabetic() => Script::Latin,
        c if c.is_ascii_digit() => Script::Digit,
        _ => Script::Other,
    }
}

fn push_han_bigrams(run: &str, out: &mut Vec<String>) {
    let chars: Vec<char> = run.chars().collect();
    if chars.len() == 1 {
        out.push(chars[0].to_string());
        return;
    }
    for w in chars.windows(2) {
        out.push(w.iter().collect());
    }
}

impl Tokenizer for JapaneseCharTokenizer {
    fn tokenize(&self, text: &str) -> Vec<String> {
        let mut out = Vec::new();
        let mut buf = String::new();
        let mut current: Option<Script> = None;

        let flush = |script: Option<Script>, buf: &mut String, out: &mut Vec<String>| {
            if buf.is_empty() {
                return;
            }
            match script {
                Some(Script::Han) => push_han_bigrams(buf, out),
                Some(Script::Latin) => out.push(buf.to_lowercase()),
                Some(Script::Other) | None => {}
                Some(_) => out.push(buf.clone()),
            }
            buf.clear();
        };

        for c in text.chars() {
            let script = classify(c);
            if Some(script) != current {
                flush(current, &mut buf, &mut out);
                current = Some(script);
            }
            if script != Script::Other {
                buf.push(c);
            }
        }
        flush(current, &mut buf, &mut out);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whitespace_splits_on_whitespace_and_punctuation() {
        let t = WhitespaceTokenizer;
        let toks = t.tokenize("Hello, world! This is tsumugi.");
        assert_eq!(toks, vec!["hello", "world", "this", "is", "tsumugi"]);
    }

    #[test]
    fn whitespace_empty_string_yields_empty() {
        let t = WhitespaceTokenizer;
        assert!(t.tokenize("").is_empty());
    }

    #[test]
    fn japanese_splits_by_script() {
        let t = JapaneseCharTokenizer;
        let toks = t.tokenize("東京の駅");
        // 東京の駅: Han run "東京", Hiragana "の", Han run "駅"
        // "東京" → bigram ["東京"]; "の" → ["の"]; "駅" → single-char ["駅"]
        assert_eq!(toks, vec!["東京", "の", "駅"]);
    }

    #[test]
    fn japanese_han_run_emits_bigrams() {
        let t = JapaneseCharTokenizer;
        let toks = t.tokenize("機械学習");
        // 機械学習 → bigrams [機械, 械学, 学習]
        assert_eq!(toks, vec!["機械", "械学", "学習"]);
    }

    #[test]
    fn japanese_handles_mixed_script() {
        let t = JapaneseCharTokenizer;
        let toks = t.tokenize("Rust言語とJavaScript");
        // "Rust" → ["rust"]
        // "言語" → bigram ["言語"]
        // "と" → ["と"]
        // "JavaScript" → ["javascript"]
        assert_eq!(toks, vec!["rust", "言語", "と", "javascript"]);
    }

    #[test]
    fn japanese_drops_punctuation_and_whitespace() {
        let t = JapaneseCharTokenizer;
        let toks = t.tokenize("今日は、いい天気です。");
        // 今日 (bigram) / は (hira) / いい (hira) / 天気 (bigram) / です (hira)
        assert_eq!(toks, vec!["今日", "は", "いい", "天気", "です"]);
    }

    #[test]
    fn japanese_empty_input() {
        let t = JapaneseCharTokenizer;
        assert!(t.tokenize("").is_empty());
        assert!(t.tokenize("。、！？").is_empty());
    }
}
