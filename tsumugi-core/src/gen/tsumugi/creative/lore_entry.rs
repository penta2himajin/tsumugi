use super::LoreScope;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LoreEntry {
    pub scope: LoreScope,
}
