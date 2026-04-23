//! QueryClassifier: cheap routing of queries to processing tiers.

use async_trait::async_trait;

/// Coarse classification buckets used by the Context Compiler to decide
/// which retrieval / scoring / compression path to take.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum QueryClass {
    /// Keyword lookup, no reasoning needed.
    Literal,
    /// Narrative continuation, requires character + recent context.
    Narrative,
    /// Analysis / reasoning, may need structured summaries.
    Analytical,
    /// Unknown / fallback — treat as Analytical.
    Unknown,
}

#[async_trait]
pub trait QueryClassifier: Send + Sync {
    async fn classify(&self, query: &str) -> anyhow::Result<QueryClass>;
}
