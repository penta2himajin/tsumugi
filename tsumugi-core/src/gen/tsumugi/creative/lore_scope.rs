use crate::tsumugi::core::Chunk;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LoreScope {
    LoreGlobal,
    LoreChunkLocal { lore_chunk: Chunk },
    LoreConditional,
}
