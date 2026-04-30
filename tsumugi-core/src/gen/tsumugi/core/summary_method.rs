#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SummaryMethod {
    LlmLingua2,
    SelectiveContext,
    ExtractiveBM25,
    DistilBart,
    UserManual,
    NoMethod,
}
