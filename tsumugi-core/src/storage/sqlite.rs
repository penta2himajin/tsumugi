//! SqliteStorage — sqlx-backed `StorageProvider` for development and early
//! production use before sqlite-vec / bespoke indexing lands (Phase 3).
//!
//! Entities are stored as JSON blobs keyed by their UUID to avoid schema
//! coupling with `Chunk` / `Fact` / `PendingItem` evolution. A richer
//! normalized schema is a Phase 3 concern and benefits from real-world
//! query profiles.

use crate::domain::{Chunk, ChunkId, Fact, FactId, PendingItem, PendingItemId};
use crate::traits::storage::{StorageError, StorageProvider, StorageResult};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Row, SqlitePool};
use std::str::FromStr;

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS chunks (
    id   TEXT PRIMARY KEY,
    data TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS facts (
    id   TEXT PRIMARY KEY,
    data TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS pending_items (
    id   TEXT PRIMARY KEY,
    data TEXT NOT NULL
);
";

pub struct SqliteStorage {
    pool: SqlitePool,
}

impl SqliteStorage {
    /// Connect to the given sqlx URL. Examples:
    /// - `sqlite::memory:` (in-process, non-persistent)
    /// - `sqlite:///tmp/tsumugi.db` (file-backed)
    /// - `sqlite:tsumugi.db?mode=rwc` (create if missing)
    pub async fn connect(url: &str) -> anyhow::Result<Self> {
        let opts = SqliteConnectOptions::from_str(url)?
            .create_if_missing(true)
            .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal);
        let pool = SqlitePoolOptions::new().connect_with(opts).await?;
        Self::from_pool(pool).await
    }

    /// Build on top of an existing pool; runs schema migration.
    pub async fn from_pool(pool: SqlitePool) -> anyhow::Result<Self> {
        // Split on ';' so each statement runs independently. sqlx doesn't
        // accept multi-statement strings via `query`.
        for stmt in SCHEMA.split(';') {
            let s = stmt.trim();
            if s.is_empty() {
                continue;
            }
            sqlx::query(s).execute(&pool).await?;
        }
        Ok(Self { pool })
    }

    async fn save_row<T: Serialize>(&self, table: &str, id: &str, value: &T) -> StorageResult<()> {
        let data = serde_json::to_string(value)
            .map_err(|e| StorageError::Backend(format!("serialize: {e}")))?;
        let sql = format!("INSERT OR REPLACE INTO {table} (id, data) VALUES (?, ?)");
        sqlx::query(&sql)
            .bind(id)
            .bind(data)
            .execute(&self.pool)
            .await
            .map_err(|e| StorageError::Backend(e.to_string()))?;
        Ok(())
    }

    async fn load_row<T: for<'de> Deserialize<'de>>(
        &self,
        table: &str,
        kind: &'static str,
        id: &str,
    ) -> StorageResult<T> {
        let sql = format!("SELECT data FROM {table} WHERE id = ?");
        let row = sqlx::query(&sql)
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| StorageError::Backend(e.to_string()))?;
        let row = row.ok_or_else(|| StorageError::NotFound {
            kind,
            id: id.to_string(),
        })?;
        let data: String = row
            .try_get("data")
            .map_err(|e| StorageError::Backend(e.to_string()))?;
        serde_json::from_str(&data).map_err(|e| StorageError::Backend(format!("deserialize: {e}")))
    }

    async fn delete_row(&self, table: &str, kind: &'static str, id: &str) -> StorageResult<()> {
        let sql = format!("DELETE FROM {table} WHERE id = ?");
        let res = sqlx::query(&sql)
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| StorageError::Backend(e.to_string()))?;
        if res.rows_affected() == 0 {
            return Err(StorageError::NotFound {
                kind,
                id: id.to_string(),
            });
        }
        Ok(())
    }

    async fn list_rows(&self, table: &str) -> StorageResult<Vec<String>> {
        let sql = format!("SELECT id FROM {table}");
        let rows = sqlx::query(&sql)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| StorageError::Backend(e.to_string()))?;
        rows.iter()
            .map(|r| r.try_get::<String, _>("id"))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| StorageError::Backend(e.to_string()))
    }
}

#[async_trait]
impl StorageProvider for SqliteStorage {
    async fn save_chunk(&self, chunk: &Chunk) -> StorageResult<()> {
        self.save_row("chunks", &chunk.id.to_string(), chunk).await
    }

    async fn load_chunk(&self, id: ChunkId) -> StorageResult<Chunk> {
        self.load_row("chunks", "chunk", &id.to_string()).await
    }

    async fn delete_chunk(&self, id: ChunkId) -> StorageResult<()> {
        self.delete_row("chunks", "chunk", &id.to_string()).await
    }

    async fn list_chunks(&self) -> StorageResult<Vec<ChunkId>> {
        let ids = self.list_rows("chunks").await?;
        parse_ids(ids, ChunkId::from_uuid)
    }

    async fn save_fact(&self, fact: &Fact) -> StorageResult<()> {
        self.save_row("facts", &fact.id.to_string(), fact).await
    }

    async fn load_fact(&self, id: FactId) -> StorageResult<Fact> {
        self.load_row("facts", "fact", &id.to_string()).await
    }

    async fn delete_fact(&self, id: FactId) -> StorageResult<()> {
        self.delete_row("facts", "fact", &id.to_string()).await
    }

    async fn list_facts(&self) -> StorageResult<Vec<FactId>> {
        let ids = self.list_rows("facts").await?;
        parse_ids(ids, FactId::from_uuid)
    }

    async fn save_pending(&self, item: &PendingItem) -> StorageResult<()> {
        self.save_row("pending_items", &item.id.to_string(), item)
            .await
    }

    async fn load_pending(&self, id: PendingItemId) -> StorageResult<PendingItem> {
        self.load_row("pending_items", "pending_item", &id.to_string())
            .await
    }

    async fn delete_pending(&self, id: PendingItemId) -> StorageResult<()> {
        self.delete_row("pending_items", "pending_item", &id.to_string())
            .await
    }

    async fn list_pending(&self) -> StorageResult<Vec<PendingItemId>> {
        let ids = self.list_rows("pending_items").await?;
        parse_ids(ids, PendingItemId::from_uuid)
    }
}

fn parse_ids<T>(raw: Vec<String>, ctor: fn(uuid::Uuid) -> T) -> StorageResult<Vec<T>> {
    raw.into_iter()
        .map(|s| {
            uuid::Uuid::parse_str(&s)
                .map(ctor)
                .map_err(|e| StorageError::Backend(format!("parse uuid {s}: {e}")))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{Chunk, Fact, FactOrigin, FactScope, PendingItem, Priority};

    async fn new_store() -> SqliteStorage {
        SqliteStorage::connect("sqlite::memory:").await.unwrap()
    }

    #[tokio::test]
    async fn chunk_crud_roundtrip() {
        let store = new_store().await;
        let mut chunk = Chunk::raw_leaf("sqlite test");
        chunk
            .keywords
            .insert(crate::domain::Keyword::from("sqlite"));
        let id = chunk.id;
        store.save_chunk(&chunk).await.unwrap();

        let loaded = store.load_chunk(id).await.unwrap();
        assert_eq!(loaded.id, chunk.id);
        assert_eq!(loaded.text, "sqlite test");
        assert!(loaded
            .keywords
            .contains(&crate::domain::Keyword::from("sqlite")));

        let list = store.list_chunks().await.unwrap();
        assert_eq!(list, vec![id]);

        store.delete_chunk(id).await.unwrap();
        assert!(store.load_chunk(id).await.is_err());
    }

    #[tokio::test]
    async fn fact_and_pending_roundtrip() {
        let store = new_store().await;
        let fact = Fact::new("hp", "12", FactScope::Global, FactOrigin::User);
        store.save_fact(&fact).await.unwrap();
        assert_eq!(store.load_fact(fact.id).await.unwrap().key, "hp");

        let anchor = Chunk::raw_leaf("anchor");
        store.save_chunk(&anchor).await.unwrap();
        let pi = PendingItem::new("plot", "x", anchor.id, Priority::High);
        store.save_pending(&pi).await.unwrap();
        assert_eq!(store.load_pending(pi.id).await.unwrap().description, "x");
    }

    #[tokio::test]
    async fn delete_missing_returns_not_found() {
        let store = new_store().await;
        let err = store.delete_chunk(ChunkId::new()).await.unwrap_err();
        assert!(matches!(err, StorageError::NotFound { .. }));
    }

    #[tokio::test]
    async fn save_is_upsert() {
        let store = new_store().await;
        let mut chunk = Chunk::raw_leaf("v1");
        let id = chunk.id;
        store.save_chunk(&chunk).await.unwrap();
        chunk.text = "v2".to_string();
        store.save_chunk(&chunk).await.unwrap();
        assert_eq!(store.load_chunk(id).await.unwrap().text, "v2");
    }
}
