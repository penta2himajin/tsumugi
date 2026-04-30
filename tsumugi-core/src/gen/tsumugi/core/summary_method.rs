#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SummaryMethod {
    LlmFull,
    LlmLingua2,
    SelectiveContext,
    ExtractiveBM25,
    DistilBart,
    UserManual,
    NoMethod,
}
