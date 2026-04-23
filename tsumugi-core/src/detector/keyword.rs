//! KeywordDetector — Tier 0 exact-substring matcher. Emits a `DetectedEvent`
//! for every (label, keyword) pair whose keyword is found in the chunk's
//! text or in the new turn's serialized JSON payload.

use crate::domain::Chunk;
use crate::traits::detector::EventDetector;
use async_trait::async_trait;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DetectedEvent {
    pub label: String,
    pub matched_keyword: String,
}

pub struct KeywordDetector {
    rules: Vec<(String, Vec<String>)>,
}

impl KeywordDetector {
    pub fn new() -> Self {
        Self { rules: vec![] }
    }

    pub fn with_rule(mut self, label: impl Into<String>, keywords: Vec<String>) -> Self {
        self.rules.push((label.into(), keywords));
        self
    }
}

impl Default for KeywordDetector {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl EventDetector for KeywordDetector {
    type Event = DetectedEvent;

    async fn detect(
        &self,
        chunk: &Chunk,
        new_turn: &serde_json::Value,
    ) -> anyhow::Result<Vec<Self::Event>> {
        let haystack = format!("{} {}", chunk.text, new_turn);
        let mut out = Vec::new();
        for (label, keywords) in &self.rules {
            for k in keywords {
                if haystack.contains(k.as_str()) {
                    out.push(DetectedEvent {
                        label: label.clone(),
                        matched_keyword: k.clone(),
                    });
                }
            }
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn matches_keyword_in_text() {
        let d = KeywordDetector::new().with_rule("combat", vec!["attack".into(), "slash".into()]);
        let mut c = Chunk::raw_leaf("The hero attacks the goblin.");
        c.text = "The hero attacks the goblin.".to_string();
        let events = d.detect(&c, &json!({})).await.unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].label, "combat");
    }

    #[tokio::test]
    async fn matches_keyword_in_turn_payload() {
        let d = KeywordDetector::new().with_rule("item", vec!["sword".into()]);
        let c = Chunk::raw_leaf("");
        let events = d
            .detect(&c, &json!({"action": "pick up sword"}))
            .await
            .unwrap();
        assert_eq!(events.len(), 1);
    }
}
