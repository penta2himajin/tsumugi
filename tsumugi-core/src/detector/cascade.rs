//! CascadeDetector — runs child detectors in order, short-circuiting when
//! any stage returns events. Mirrors the chatstream 3-tier cascade
//! (keyword → embedding → LLM) and keeps cheap tiers in the common path.

use super::keyword::DetectedEvent;
use crate::domain::Chunk;
use crate::traits::detector::EventDetector;
use async_trait::async_trait;
use std::sync::Arc;

pub struct CascadeDetector {
    stages: Vec<Arc<dyn EventDetector<Event = DetectedEvent>>>,
}

impl CascadeDetector {
    pub fn new() -> Self {
        Self { stages: vec![] }
    }

    pub fn add_stage(mut self, detector: Arc<dyn EventDetector<Event = DetectedEvent>>) -> Self {
        self.stages.push(detector);
        self
    }
}

impl Default for CascadeDetector {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl EventDetector for CascadeDetector {
    type Event = DetectedEvent;

    async fn detect(
        &self,
        chunk: &Chunk,
        new_turn: &serde_json::Value,
    ) -> anyhow::Result<Vec<Self::Event>> {
        for stage in &self.stages {
            let events = stage.detect(chunk, new_turn).await?;
            if !events.is_empty() {
                return Ok(events);
            }
        }
        Ok(vec![])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::detector::KeywordDetector;
    use serde_json::json;

    #[tokio::test]
    async fn short_circuits_on_first_hit() {
        let tier0 = Arc::new(KeywordDetector::new().with_rule("combat", vec!["attack".into()]));
        let tier1 =
            Arc::new(KeywordDetector::new().with_rule("other", vec!["never-matches-xyz".into()]));
        let cascade = CascadeDetector::new().add_stage(tier0).add_stage(tier1);
        let c = Chunk::raw_leaf("attack!");
        let events = cascade.detect(&c, &json!({})).await.unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].label, "combat");
    }

    #[tokio::test]
    async fn falls_through_empty_stages() {
        let tier0 =
            Arc::new(KeywordDetector::new().with_rule("combat", vec!["never-matches-xyz".into()]));
        let tier1 = Arc::new(KeywordDetector::new().with_rule("item", vec!["sword".into()]));
        let cascade = CascadeDetector::new().add_stage(tier0).add_stage(tier1);
        let c = Chunk::raw_leaf("found a sword");
        let events = cascade.detect(&c, &json!({})).await.unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].label, "item");
    }
}
