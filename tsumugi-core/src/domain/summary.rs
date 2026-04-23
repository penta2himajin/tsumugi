//! Summary method tags that mirror the Alloy enum.

use serde::{Deserialize, Serialize};

/// How a given summary was produced. `None` marks a raw leaf.
///
/// Mirrors `models/tsumugi/core.als` (`SummaryMethod`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SummaryMethod {
    LlmFull,
    LlmLingua2,
    SelectiveContext,
    ExtractiveBM25,
    UserManual,
    /// Raw leaf — summary not generated. Corresponds to `NoMethod` in Alloy.
    #[default]
    None,
}

impl SummaryMethod {
    pub fn is_some_method(&self) -> bool {
        !matches!(self, SummaryMethod::None)
    }
}
