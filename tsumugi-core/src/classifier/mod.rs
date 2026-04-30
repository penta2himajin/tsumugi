//! QueryClassifier implementations.

mod bert_classifier;
mod regex_classifier;
mod setfit_classifier;

pub use bert_classifier::BertClassifier;
pub use regex_classifier::RegexClassifier;
pub use setfit_classifier::{LinearHeadFile, SetFitClassifier, DEFAULT_MINI_LM_DIM};
