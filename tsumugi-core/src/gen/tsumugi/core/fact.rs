use super::FactOrigin;
use super::FactScope;

/// Invariant: SupersededByDirect
/// Invariant: NoCyclicSupersession
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Fact {
    pub scope: FactScope,
    pub superseded_by: Option<Box<Fact>>,
    pub origin: FactOrigin,
}
