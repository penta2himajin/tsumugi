# つむぎ — 技術アーキテクチャ

## ツールチェーン

- 言語: Rust (コア) + TypeScript (SDK)
- パッケージマネージャ: cargo, bun
- ドメインモデル: oxidtr (Alloy → Rust / TypeScript 型・テスト・不変条件)
- テスト: Rust の `#[test]` + 結合テスト、TypeScript の vitest (SDK 段階)
- 日本語トークナイザ: lindera (BM25 用)
- ベクトル検索: 初期は InMemory、将来 sqlite-vec 検討

## ワークスペース構成

```
tsumugi/
├── tsumugi-core/        # コアライブラリ (Rust crate)
│   ├── src/
│   │   ├── domain/      # ドメイン型 (oxidtr 生成 + 手書き拡張)
│   │   ├── traits/      # 6 種の trait (Storage / Embedding / ...)
│   │   ├── retriever/   # BM25 + cosine hybrid
│   │   ├── scorer/      # RelevanceScorer 実装群
│   │   ├── detector/    # EventDetector 実装群
│   │   └── compiler/    # Context Compiler
│   └── tests/           # 結合テスト (小説・TRPG シナリオ各 1 本)
├── tsumugi-cli/         # 開発・検証用 REPL (Rust binary)
├── tsumugi-ts/          # TypeScript SDK (Phase 2 以降)
└── models/              # Alloy ソース (oxidtr 入力)
    └── tsumugi.als
```

`tsumugi-server/` は作らない。上位製品 (つかさ / つづり / つくも) は Tauri アプリとしてコアを直接埋め込む。

`tsumugi-tauri-adapter/` も作らない。Tauri 連携は各製品が自前で書いた方が簡潔。

`tsumugi-kv/` のサブクレート分離は保留。つくもの要件が固まった時点で再検討。

---

## 核心抽象 (Rust 型)

### Chunk

```rust
pub struct Chunk {
    pub id: ChunkId,
    pub text: String,                      // 正規化表示用テキスト
    pub items: Vec<serde_json::Value>,     // ドメイン Turn の serialize
    pub summary: String,                   // 要約 (空文字可、遅延生成)
    pub keywords: HashSet<Keyword>,
    pub facts: Vec<FactId>,                // Fact への参照
    pub pending: Vec<PendingItemId>,       // PendingItem への参照
    pub parent: Option<ChunkId>,
    pub children: Vec<ChunkId>,
    pub metadata: serde_json::Map<String, Value>,  // 製品固有タグ (scene / session / chapter 等)
    pub last_active_at: DateTime<Utc>,
    pub order_in_parent: i64,              // 階層内の順序 (章番号・シーン番号等)
}
```

- text / items の分離により、製品は自由なドメイン型を持てる
- metadata に `{"kind": "scene", "location": "cafe"}` 等を入れて製品固有に識別
- order_in_parent は ChapterOrderScorer が使う

### Fact

```rust
pub struct Fact {
    pub id: FactId,
    pub key: String,                       // "currency", "san_rate", "protagonist_name"
    pub value: String,                     // "gold coin", "1/1d10", "Alice"
    pub scope: FactScope,                  // Global / ChunkLocal(ChunkId)
    pub superseded_by: Option<FactId>,     // 版管理
    pub created_at: DateTime<Utc>,
    pub origin: FactOrigin,                // User | Extracted | Derived
}

pub enum FactScope {
    Global,
    ChunkLocal(ChunkId),
}
```

- supersede 機構で「以前は X、今は Y」を表現
- origin で「ユーザー明示入力」「LLM 抽出」「他 Fact から派生」を区別

### Character

```rust
pub struct Character {
    pub id: CharacterId,
    pub name: String,
    pub voice_samples: Vec<String>,        // few-shot 注入用の台詞例
    pub speech_traits: Option<SpeechTraits>,
    pub relationship_notes: HashMap<CharacterId, String>,
    pub sheet: serde_json::Map<String, Value>,  // ドメイン固有 (SAN/HP/backstory/visual_desc 等)
    pub first_appearance: Option<ChunkId>,
    pub style_tags: Vec<String>,           // 検索用タグ
}

pub struct SpeechTraits {
    pub formality: Formality,
    pub quirks: Vec<String>,
    pub emotional_state: Option<String>,
}
```

### LoreEntry

```rust
pub struct LoreEntry {
    pub id: LoreEntryId,
    pub category: String,                  // "world", "faction", "item", "history", "mythos"
    pub title: String,
    pub content: String,
    pub scope: LoreScope,
    pub keywords: Vec<Keyword>,            // トリガーキーワード
}

pub enum LoreScope {
    Global,
    ChunkLocal(ChunkId),
    Conditional(String),                   // "when character X is present"
}
```

### PendingItem

```rust
pub struct PendingItem {
    pub id: PendingItemId,
    pub kind: String,                      // "plot", "clue", "investigation", "ho_trigger"
    pub description: String,
    pub introduced_at: ChunkId,
    pub expected_resolution_chunk: Option<ChunkId>,
    pub resolved_at: Option<ChunkId>,
    pub priority: Priority,
}
```

### Scene (= 特殊化された Chunk)

Scene は独立 entity ではなく、`Chunk.metadata` に `{"kind": "scene"}` を持つ Chunk として扱う。`SceneView` は読み取り専用ビュー:

```rust
pub struct SceneView<'a> {
    chunk: &'a Chunk,
    participants: Vec<CharacterId>,        // 対話 items から抽出
    location: Option<String>,              // metadata から
    time_marker: Option<String>,           // metadata から
}
```

---

## 核心 trait

### StorageProvider

```rust
pub trait StorageProvider: Send + Sync {
    async fn save_chunk(&self, chunk: &Chunk) -> Result<()>;
    async fn load_chunk(&self, id: ChunkId) -> Result<Option<Chunk>>;
    async fn delete_chunk(&self, id: ChunkId) -> Result<()>;
    async fn list_children(&self, parent: ChunkId) -> Result<Vec<ChunkId>>;

    async fn save_fact(&self, fact: &Fact) -> Result<()>;
    async fn load_fact(&self, id: FactId) -> Result<Option<Fact>>;
    async fn list_facts_by_scope(&self, scope: &FactScope) -> Result<Vec<Fact>>;

    async fn save_character(&self, c: &Character) -> Result<()>;
    async fn load_character(&self, id: CharacterId) -> Result<Option<Character>>;

    async fn save_lore_entry(&self, e: &LoreEntry) -> Result<()>;
    async fn load_lore_entry(&self, id: LoreEntryId) -> Result<Option<LoreEntry>>;

    async fn save_pending(&self, p: &PendingItem) -> Result<()>;
    async fn load_pending(&self, id: PendingItemId) -> Result<Option<PendingItem>>;
    async fn list_unresolved(&self) -> Result<Vec<PendingItem>>;
}
```

デフォルト実装: `InMemoryStorage` (Phase 1)、`SqliteStorage` (Phase 2)

### EmbeddingProvider

```rust
pub trait EmbeddingProvider: Send + Sync {
    async fn embed(&self, text: &str) -> Result<Vec<f32>>;
    async fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>>;
    fn dim(&self) -> usize;
}
```

デフォルト実装: `MockEmbedding` (テスト用、ハッシュベース)、`LMStudioEmbedding` (実使用)

### LLMProvider

```rust
pub trait LLMProvider: Send + Sync {
    async fn complete(&self, req: LLMRequest) -> Result<LLMResponse>;
    async fn stream(&self, req: LLMRequest) -> Result<BoxStream<LLMChunk>>;
}

pub struct LLMRequest {
    pub messages: Vec<Message>,
    pub max_tokens: Option<u32>,
    pub temperature: Option<f32>,
    pub stop: Vec<String>,
    pub grammar: Option<String>,           // GBNF / JSON Schema
}
```

デフォルト実装: `LMStudioProvider`, `OllamaProvider` (OpenAI 互換 API)

### Retriever

```rust
pub trait Retriever: Send + Sync {
    async fn retrieve(
        &self,
        query: &RetrievalQuery,
        candidates: &[ChunkId],
    ) -> Result<Vec<RetrievalHit>>;
}

pub struct RetrievalQuery {
    pub text: String,
    pub embedding: Option<Vec<f32>>,
    pub top_k: usize,
}

pub struct RetrievalHit {
    pub chunk_id: ChunkId,
    pub bm25_score: f32,
    pub cosine_score: f32,
    pub combined_score: f32,
}
```

デフォルト実装: `HybridRetriever` (BM25 + cosine 重み付き)、`BM25OnlyRetriever`, `CosineOnlyRetriever`

### RelevanceScorer

```rust
pub trait RelevanceScorer: Send + Sync {
    fn score(&self, chunk: &Chunk, ctx: &ScoringContext) -> f32;
}

pub struct ScoringContext {
    pub current_chunk_id: Option<ChunkId>,
    pub current_time: DateTime<Utc>,
    pub current_order: Option<i64>,
    pub retrieval_hit: Option<&RetrievalHit>,
}
```

同梱実装:
- `TemporalDecayScorer { half_life: Duration }` — つかさ向け (セッション時刻ベース)
- `ChapterOrderScorer { decay_per_chapter: f32 }` — つづり向け (章順距離ベース)
- `NoDecayScorer` — つくも向け (時間無関係)
- `CompositeScorer(Vec<Box<dyn RelevanceScorer>>)` — 複数スコアの合成

### EventDetector

```rust
#[async_trait]
pub trait EventDetector: Send + Sync {
    type Event;

    async fn detect(
        &self,
        new_text: &str,
        context: &Chunk,
    ) -> Result<Vec<Self::Event>>;
}
```

同梱実装:
- `KeywordDetector` — 完全一致、コスト 0、即応
- `EmbeddingSimilarityDetector` — top-K 意味類似
- `LLMClassifierDetector` — 軽量 LLM で yes/no 判定
- `CascadeDetector<D1, D2, D3>` — 3 段カスケード (Keyword → Embedding → LLM)

chatstream の話題切替検知も将来同じ構造で実装可能 (`TopicSwitchDetector impl EventDetector`)。

---

## Context Compiler

各ターンの LLM 呼び出しで渡すコンテキストを組み立てるコンポーネント。

```rust
pub struct ContextCompiler {
    storage: Arc<dyn StorageProvider>,
    retriever: Arc<dyn Retriever>,
    scorer: Arc<dyn RelevanceScorer>,
    embedding: Arc<dyn EmbeddingProvider>,
}

pub struct CompiledContext {
    pub resident: ResidentLayer,           // 常駐レイヤー
    pub dynamic: DynamicLayer,             // 動的レイヤー
}

pub struct ResidentLayer {
    pub current_scene_summary: Option<String>,
    pub recent_turns: Vec<String>,         // 生の直近 N ターン
    pub active_characters: Vec<Character>, // シーン登場キャラの sheet
    pub style_preset: Option<String>,      // 製品固有の文体指示
}

pub struct DynamicLayer {
    pub related_past_chunks: Vec<Chunk>,
    pub related_lore: Vec<LoreEntry>,
    pub unresolved_pending: Vec<PendingItem>,
    pub supporting_facts: Vec<Fact>,
}
```

コンパイル手順:
1. 現在の ChunkId から常駐レイヤーを構築
2. クエリ (次ターンの文脈、キャラ言及、キーワード抽出) から候補 chunk を取得
3. Retriever で検索 → RelevanceScorer で並べ替え → top-K を動的レイヤーに
4. 関連 LoreEntry を keyword 一致 + embedding 類似で抽出
5. 未解決 PendingItem を現在 Chunk と関連するもので絞り込み
6. Fact を scope で取得

製品側は CompiledContext を受け取り、最終プロンプトを組み立てる (system プロンプト、few-shot 整形等は製品の責任)。

---

## Alloy モデル戦略

`models/tsumugi.als` で記述する内容:

### 含むもの

- 型構造 (sig, enum, record フィールド)
- 参照整合性 (ChunkId → Chunk, CharacterId → Character 等、dangling なし)
- 階層の非循環 (parent チェーンに自分を含まない)
- PendingItem のライフサイクル (introduced_at < resolved_at、resolved_at 存在時のみ resolved 扱い)
- Fact の supersession 関係 (superseded_by が循環しない)

### 含まないもの

- 業務ロジック (Context Compiler の動作、スコアリング式)
- パフォーマンス関連 (インデックス設計等)
- 製品固有の制約 (TRPG の SAN ルール等)

### 生成物

- Rust: `tsumugi-core/src/domain/gen/` に型定義、fixture、property test
- TypeScript: `tsumugi-ts/src/gen/` に型定義

### 検証

- `oxidtr check` を CI で実行、Alloy モデルと手書き実装のズレを検知
- `oxidtr generate` を pre-commit フックで実行、生成コードの更新漏れを防ぐ

---

## chatstream との関係

両者は**兄弟プロジェクト**。

| 観点 | chatstream | つむぎ |
|---|---|---|
| 主ターゲット | 音声 AI デバイス | テキスト創作 (TRPG / 小説) |
| Turn 抽象 | 音声入出力ペア | 非依存 (`Chunk.items: Value`) |
| 話題検知 | 3 段カスケード | `EventDetector` trait (将来共通化) |
| ドメイン状態 | facts 中心 | Character / Scene / LoreEntry / PendingItem 追加 |
| Stage 3 UI | 音声制約 (選択肢 2 個) | テキスト制約ほぼなし |
| レイテンシ要件 | 厳しい | 緩い (数秒許容) |

共通するのは:
- 階層的コンテキスト管理の設計思想
- trait 駆動の拡張性
- oxidtr + Alloy の開発フロー
- 全データ保持 (損失なき要約)

将来の統合可能性として、`EventDetector` trait を chatstream 側にも輸入して話題検知を差し替え可能にする、等の道筋がある。現時点では**各自独立実装、設計の互換性を保つ**が方針。

---

## 未確定論点 (Phase 1 以降で詰める)

- `Chunk.items` の具体的な serialize 形式 (JSON? MessagePack?)
- `SpeechTraits` の拡張性 (struct のまま vs Map<String, Value>)
- `PendingItem.kind` の型安全化 (String vs enum)
- sqlite-vec 採用のタイミング (Phase 2 で SqliteStorage を作るとき)
- `Retriever` の BM25/cosine 重み調整 (定数 vs 学習)
- Context Compiler の結果キャッシュ戦略
- `EventDetector::Event` の型パラメータ化の妥当性 (トレイトオブジェクト化が必要か)
- tsumugi-ts の実装時期 (MVP 後、つづり / つかさで実需が出てから)
