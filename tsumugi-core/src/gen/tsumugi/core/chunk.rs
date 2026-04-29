use super::Fact;
use super::Item;
use super::PendingItem;
use super::SourceLocationValue;
use super::SummaryMethod;
use std::collections::BTreeSet;

/// Invariant: ParentToChildLink
/// Invariant: ChildToParentLink
/// Invariant: NoCyclicParent
/// Invariant: RawLeafHasItems
/// Invariant: SummaryNodeHasChildren
/// Invariant: ParentMoreAbstractThanChild
/// Invariant: RawLeafHasNoMethod
/// Invariant: SummaryNodeHasMethod
/// Invariant: HappensBeforeTransitive
/// Invariant: NoCyclicHappensBefore
/// Invariant: ChunkChildrenUnbounded
/// Invariant: ChunkItemsUnbounded
/// Invariant: ChunkFactsUnbounded
/// Invariant: ChunkPendingUnbounded
/// Invariant: ChunkHappensBeforeUnbounded
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Chunk {
    pub parent: Option<Box<Chunk>>,
    pub children: BTreeSet<Chunk>,
    pub items: BTreeSet<Item>,
    pub facts: BTreeSet<Fact>,
    pub pending: BTreeSet<PendingItem>,
    pub source_location: Option<SourceLocationValue>,
    pub summary_method: Option<SummaryMethod>,
    pub summary_level: i64,
    pub happens_before: BTreeSet<Chunk>,
}
