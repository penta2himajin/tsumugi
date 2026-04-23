//! QueryClassifier implementations.

mod bert_classifier;
mod regex_classifier;

pub use bert_classifier::BertClassifier;
pub use regex_classifier::RegexClassifier;
