//! Summarizer implementations.

mod extractive_bm25;
mod hierarchical;
mod llm;
mod protection;

pub use extractive_bm25::ExtractiveBM25Summarizer;
pub use hierarchical::HierarchicalSummarizer;
pub use llm::LlmSummarizer;
pub use protection::{apply_summary_update, SummaryUpdate, SummaryUpdateOutcome};
