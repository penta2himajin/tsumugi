//! Summarizer implementations.

mod distilbart;
mod extractive_bm25;
mod hierarchical;
mod protection;

pub use distilbart::{
    DistilBartSummarizer, DEFAULT_BOS_TOKEN_ID, DEFAULT_DECODER_START_TOKEN_ID,
    DEFAULT_EOS_TOKEN_ID, DEFAULT_MAX_INPUT_LENGTH, DEFAULT_MAX_OUTPUT_LENGTH,
    DEFAULT_MIN_OUTPUT_LENGTH, DEFAULT_PAD_TOKEN_ID,
};
pub use extractive_bm25::ExtractiveBM25Summarizer;
pub use hierarchical::HierarchicalSummarizer;
pub use protection::{apply_summary_update, SummaryUpdate, SummaryUpdateOutcome};
