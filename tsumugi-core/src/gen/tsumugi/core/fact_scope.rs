use super::Chunk;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FactScope {
    GlobalScope,
    ChunkLocalScope {
        scope_chunk: Chunk,
    },
}

