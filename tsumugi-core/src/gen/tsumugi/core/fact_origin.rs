#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FactOrigin {
    UserOrigin,
    ExtractedOrigin,
    DerivedOrigin,
}
