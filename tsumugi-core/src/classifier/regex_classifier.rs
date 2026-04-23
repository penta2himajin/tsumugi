//! RegexClassifier — Tier 0 query classifier based on regex rules.
//!
//! Matches `(pattern, class)` pairs in declaration order; the first match
//! wins. No match falls through to `Unknown`. Regex flavor is Rust's `regex`
//! crate (no lookaround) — patterns that need lookaround should be expressed
//! as multiple rules instead.

use crate::traits::classifier::{QueryClass, QueryClassifier};
use async_trait::async_trait;
use regex::Regex;

pub struct RegexClassifier {
    rules: Vec<(Regex, QueryClass)>,
    default: QueryClass,
}

impl RegexClassifier {
    pub fn new() -> Self {
        Self {
            rules: vec![],
            default: QueryClass::Unknown,
        }
    }

    pub fn with_rule(mut self, pattern: &str, class: QueryClass) -> Result<Self, regex::Error> {
        self.rules.push((Regex::new(pattern)?, class));
        Ok(self)
    }

    pub fn with_default(mut self, class: QueryClass) -> Self {
        self.default = class;
        self
    }
}

impl Default for RegexClassifier {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl QueryClassifier for RegexClassifier {
    async fn classify(&self, query: &str) -> anyhow::Result<QueryClass> {
        for (re, class) in &self.rules {
            if re.is_match(query) {
                return Ok(*class);
            }
        }
        Ok(self.default)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn classifies_by_first_matching_rule() {
        let c = RegexClassifier::new()
            .with_rule(r"(?i)^(what|who|where)", QueryClass::Literal)
            .unwrap()
            .with_rule(r"(?i)(next|then|continue)", QueryClass::Narrative)
            .unwrap()
            .with_default(QueryClass::Analytical);

        assert_eq!(
            c.classify("What is HP?").await.unwrap(),
            QueryClass::Literal
        );
        assert_eq!(
            c.classify("continue the story").await.unwrap(),
            QueryClass::Narrative
        );
        assert_eq!(
            c.classify("explain the plot arc").await.unwrap(),
            QueryClass::Analytical
        );
    }
}
