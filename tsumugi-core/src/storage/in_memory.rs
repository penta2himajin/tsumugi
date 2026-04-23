//! InMemoryStorage — external-dependency-free StorageProvider.
//!
//! Used in tests and early product iteration before SQLite lands.

use crate::domain::{Chunk, ChunkId, Fact, FactId, PendingItem, PendingItemId};
use crate::traits::storage::{StorageError, StorageProvider, StorageResult};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Mutex;

#[cfg(feature = "creative")]
use crate::creative::{Character, CharacterId, LoreEntry, LoreEntryId};

#[derive(Default)]
pub struct InMemoryStorage {
    inner: Mutex<Inner>,
}

#[derive(Default)]
struct Inner {
    chunks: HashMap<ChunkId, Chunk>,
    facts: HashMap<FactId, Fact>,
    pending: HashMap<PendingItemId, PendingItem>,
    #[cfg(feature = "creative")]
    characters: HashMap<CharacterId, Character>,
    #[cfg(feature = "creative")]
    lore: HashMap<LoreEntryId, LoreEntry>,
}

impl InMemoryStorage {
    pub fn new() -> Self {
        Self::default()
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().expect("InMemoryStorage mutex poisoned")
    }
}

#[async_trait]
impl StorageProvider for InMemoryStorage {
    async fn save_chunk(&self, chunk: &Chunk) -> StorageResult<()> {
        self.lock().chunks.insert(chunk.id, chunk.clone());
        Ok(())
    }

    async fn load_chunk(&self, id: ChunkId) -> StorageResult<Chunk> {
        self.lock()
            .chunks
            .get(&id)
            .cloned()
            .ok_or(StorageError::NotFound {
                kind: "chunk",
                id: id.to_string(),
            })
    }

    async fn delete_chunk(&self, id: ChunkId) -> StorageResult<()> {
        self.lock()
            .chunks
            .remove(&id)
            .map(|_| ())
            .ok_or(StorageError::NotFound {
                kind: "chunk",
                id: id.to_string(),
            })
    }

    async fn list_chunks(&self) -> StorageResult<Vec<ChunkId>> {
        Ok(self.lock().chunks.keys().copied().collect())
    }

    async fn save_fact(&self, fact: &Fact) -> StorageResult<()> {
        self.lock().facts.insert(fact.id, fact.clone());
        Ok(())
    }

    async fn load_fact(&self, id: FactId) -> StorageResult<Fact> {
        self.lock()
            .facts
            .get(&id)
            .cloned()
            .ok_or(StorageError::NotFound {
                kind: "fact",
                id: id.to_string(),
            })
    }

    async fn delete_fact(&self, id: FactId) -> StorageResult<()> {
        self.lock()
            .facts
            .remove(&id)
            .map(|_| ())
            .ok_or(StorageError::NotFound {
                kind: "fact",
                id: id.to_string(),
            })
    }

    async fn list_facts(&self) -> StorageResult<Vec<FactId>> {
        Ok(self.lock().facts.keys().copied().collect())
    }

    async fn save_pending(&self, item: &PendingItem) -> StorageResult<()> {
        self.lock().pending.insert(item.id, item.clone());
        Ok(())
    }

    async fn load_pending(&self, id: PendingItemId) -> StorageResult<PendingItem> {
        self.lock()
            .pending
            .get(&id)
            .cloned()
            .ok_or(StorageError::NotFound {
                kind: "pending_item",
                id: id.to_string(),
            })
    }

    async fn delete_pending(&self, id: PendingItemId) -> StorageResult<()> {
        self.lock()
            .pending
            .remove(&id)
            .map(|_| ())
            .ok_or(StorageError::NotFound {
                kind: "pending_item",
                id: id.to_string(),
            })
    }

    async fn list_pending(&self) -> StorageResult<Vec<PendingItemId>> {
        Ok(self.lock().pending.keys().copied().collect())
    }

    #[cfg(feature = "creative")]
    async fn save_character(&self, character: &Character) -> StorageResult<()> {
        self.lock()
            .characters
            .insert(character.id, character.clone());
        Ok(())
    }

    #[cfg(feature = "creative")]
    async fn load_character(&self, id: CharacterId) -> StorageResult<Character> {
        self.lock()
            .characters
            .get(&id)
            .cloned()
            .ok_or(StorageError::NotFound {
                kind: "character",
                id: format!("{:?}", id),
            })
    }

    #[cfg(feature = "creative")]
    async fn delete_character(&self, id: CharacterId) -> StorageResult<()> {
        self.lock()
            .characters
            .remove(&id)
            .map(|_| ())
            .ok_or(StorageError::NotFound {
                kind: "character",
                id: format!("{:?}", id),
            })
    }

    #[cfg(feature = "creative")]
    async fn list_characters(&self) -> StorageResult<Vec<CharacterId>> {
        Ok(self.lock().characters.keys().copied().collect())
    }

    #[cfg(feature = "creative")]
    async fn save_lore(&self, entry: &LoreEntry) -> StorageResult<()> {
        self.lock().lore.insert(entry.id, entry.clone());
        Ok(())
    }

    #[cfg(feature = "creative")]
    async fn load_lore(&self, id: LoreEntryId) -> StorageResult<LoreEntry> {
        self.lock()
            .lore
            .get(&id)
            .cloned()
            .ok_or(StorageError::NotFound {
                kind: "lore_entry",
                id: format!("{:?}", id),
            })
    }

    #[cfg(feature = "creative")]
    async fn delete_lore(&self, id: LoreEntryId) -> StorageResult<()> {
        self.lock()
            .lore
            .remove(&id)
            .map(|_| ())
            .ok_or(StorageError::NotFound {
                kind: "lore_entry",
                id: format!("{:?}", id),
            })
    }

    #[cfg(feature = "creative")]
    async fn list_lore(&self) -> StorageResult<Vec<LoreEntryId>> {
        Ok(self.lock().lore.keys().copied().collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{Fact, FactOrigin, FactScope, PendingItem, Priority};

    #[tokio::test]
    async fn chunk_save_load_delete_list() {
        let store = InMemoryStorage::new();
        let chunk = Chunk::raw_leaf("hello");
        let id = chunk.id;

        store.save_chunk(&chunk).await.unwrap();
        let loaded = store.load_chunk(id).await.unwrap();
        assert_eq!(loaded.text, "hello");
        assert_eq!(store.list_chunks().await.unwrap(), vec![id]);

        store.delete_chunk(id).await.unwrap();
        assert!(store.load_chunk(id).await.is_err());
        assert!(store.list_chunks().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn fact_crud() {
        let store = InMemoryStorage::new();
        let fact = Fact::new("hp", "12", FactScope::Global, FactOrigin::User);
        let id = fact.id;

        store.save_fact(&fact).await.unwrap();
        let loaded = store.load_fact(id).await.unwrap();
        assert_eq!(loaded.key, "hp");

        store.delete_fact(id).await.unwrap();
        assert!(store.load_fact(id).await.is_err());
    }

    #[tokio::test]
    async fn pending_item_crud() {
        let store = InMemoryStorage::new();
        let chunk = Chunk::raw_leaf("intro");
        store.save_chunk(&chunk).await.unwrap();
        let item = PendingItem::new("plot", "Find the key", chunk.id, Priority::High);
        let id = item.id;
        store.save_pending(&item).await.unwrap();

        let loaded = store.load_pending(id).await.unwrap();
        assert_eq!(loaded.description, "Find the key");
        assert_eq!(store.list_pending().await.unwrap(), vec![id]);

        store.delete_pending(id).await.unwrap();
        assert!(store.load_pending(id).await.is_err());
    }

    #[tokio::test]
    #[cfg(feature = "creative")]
    async fn creative_crud() {
        use crate::creative::{Character, LoreEntry, LoreScope};
        let store = InMemoryStorage::new();
        let ch = Character::new("Alice");
        let ch_id = ch.id;
        store.save_character(&ch).await.unwrap();
        let loaded = store.load_character(ch_id).await.unwrap();
        assert_eq!(loaded.name, "Alice");

        let lore = LoreEntry::new("item", "Sword", "A blade.", LoreScope::Global);
        let lore_id = lore.id;
        store.save_lore(&lore).await.unwrap();
        assert_eq!(store.load_lore(lore_id).await.unwrap().title, "Sword");
    }
}
