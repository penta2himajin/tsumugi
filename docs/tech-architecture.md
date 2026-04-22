# つむぎ — 技術アーキテクチャ

## ツールチェーン

- 言語: Rust (コア) + TypeScript (SDK)
- パッケージマネージャ: cargo, bun
- ドメインモデル: oxidtr (Alloy → Rust / TypeScript 型・テスト・不変条件)
- テスト: Rust の `#[test]` + 結合テスト、TypeScript の vitest (SDK 段階)
- 日本語トークナイザ: lindera (BM25 用)
- ベクトル検索: 初期は InMemory、将来 sqlite-vec 検討

推論ランタイム (上位製品での想定): Ollama (Apple Silicon で MLX バックエンド、2026-03〜) / LM Studio / llama.cpp。`LLMProvider` trait は OpenAI 互換 API をまず実装し、Ollama / LM Studio の両方を単一実装でカバーする。

ハードウェア帯域別の推奨モデルは `docs/runtime-environment.md` を参照。調査背景は `docs/research/2026-04-model-landscape.md` を参照。

## ワークスペース構成

```
tsumugi/
├── tsumugi-core/        # コアライブラリ (Rust crate)
│   ├── src/
│   │   ├── domain/      # ドメイン非依存の型 (Chunk, Fact, LoreEntry, PendingItem)
│   │   ├── creative/    # 創作拡張 (Character, Scene, StylePreset) ★ feature = "creative"
│   │   ├── traits/      # 6 種の trait
│   │   ├── retriever/   # BM25 + cosine hybrid
│   │   ├── scorer/      # RelevanceScorer 実装群
│   │   ├── detector/    # EventDetector 実装群
│   │   └── compiler/    # Context Compiler
│   └── tests/           # 結合テスト (小説・TRPG・ツクールシナリオ各 1 本)
├── tsumugi-cli/         # 開発・検証用 REPL (Rust binary)
├── tsumugi-ts/          # TypeScript SDK (Phase 2 以降)
└── models/              # Alloy ソース (oxidtr 入力)
    ├── tsumugi-core.als
    └── tsumugi-creative.als
```

### Feature flag 方針

`tsumugi-core` の `Cargo.toml`:

```toml
[features]
default = []
creative = []
```

- `default`: ドメイン非依存コアのみ (つくもが使用)
- `creative`: Character / Scene / StylePreset を有効化 (つかさ・つづりが使用)

上位製品での依存指定例:

```toml
# つかさ / つづり
tsumugi-core = { path = "../tsumugi/tsumugi-core", features = ["creative"] }

# つくも
tsumugi-core = { path = "../tsumugi/tsumugi-core" }
```

別クレートに分けるより**軽量**で、コア側の機能追加が creative 側に自然に波及する。将来別ドメイン拡張が必要になれば、同じパターンで `coding`, `research` 等の feature を追加。

---

## 核心抽象 (Rust 型)

### コア (ドメイン非依存、default features)

#### Chunk

```rust
pub struct Chunk {
    pub id: ChunkId,
    pub text: String,                      // 正規化表示用テキスト
    pub items: Vec<serde_json::Value>,     // ドメイン Turn の serialize
    pub summary: String,
    pub keywords: HashSet<Keyword>,
    pub facts: Vec<FactId>,
    pub pending: Vec<PendingItemId>,
    pub parent: Option<ChunkId>,
    pub children: Vec<ChunkId>,
    pub metadata: serde_json::Map<String, Value>,
    pub last_active_at: DateTime<Utc>,
    pub order_in_parent: i64,
    pub source_location: Option<SourceLocation>, // ★ FileProximityScorer 用
}

pub struct SourceLocation {
    pub path: String,                      // ファイルパスやモジュール識別子
    pub span: Option<Range<usize>>,        // 位置範囲 (必要時)
}
```

#### Fact

```rust
pub struct Fact {
    pub id: FactId,
    pub key: String,
    pub value: String,
    pub scope: FactScope,
    pub superseded_by: Option<FactId>,
    pub created_at: DateTime<Utc>,
    pub origin: FactOrigin,
}

pub enum FactScope {
    Global,
    ChunkLocal(ChunkId),
}

pub enum FactOrigin {
    User,
    Extracted,
    Derived,
}
```

#### LoreEntry

```rust
pub struct LoreEntry {
    pub id: LoreEntryId,
    pub category: String,
    pub title: String,
    pub content: String,
    pub scope: LoreScope,
    pub keywords: Vec<Keyword>,
}

pub enum LoreScope {
    Global,
    ChunkLocal(ChunkId),
    Conditional(String),
}
```

#### PendingItem

```rust
pub struct PendingItem {
    pub id: PendingItemId,
    pub kind: String,                      // "plot" / "clue" / "todo" / "refactor" / ...
    pub description: String,
    pub introduced_at: ChunkId,
    pub expected_resolution_chunk: Option<ChunkId>,
    pub resolved_at: Option<ChunkId>,
    pub priority: Priority,
}
```

### 創作拡張 (feature = "creative")

#### Character

```rust
#[cfg(feature = "creative")]
pub struct Character {
    pub id: CharacterId,
    pub name: String,
    pub voice_samples: Vec<String>,
    pub speech_traits: Option<SpeechTraits>,
    pub relationship_notes: HashMap<CharacterId, String>,
    pub sheet: serde_json::Map<String, Value>,
    pub first_appearance: Option<ChunkId>,
    pub style_tags: Vec<String>,
}

#[cfg(feature = "creative")]
pub struct SpeechTraits {
    pub formality: Formality,
    pub quirks: Vec<String>,
    pub emotional_state: Option<String>,
}
```

#### Scene (= 特殊化された Chunk ビュー)

```rust
#[cfg(feature = "creative")]
pub struct SceneView<'a> {
    chunk: &'a Chunk,
    participants: Vec<CharacterId>,
    location: Option<String>,
    time_marker: Option<String>,
}
```

#### StylePreset

```rust
#[cfg(feature = "creative")]
pub struct StylePreset {
    pub pov: PoV,
    pub tense: Tense,
    pub formality: Formality,
    pub reference_samples: Vec<String>,
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

    async fn save_lore_entry(&self, e: &LoreEntry) -> Result<()>;
    async fn load_lore_entry(&self, id: LoreEntryId) -> Result<Option<LoreEntry>>;

    async fn save_pending(&self, p: &PendingItem) -> Result<()>;
    async fn load_pending(&self, id: PendingItemId) -> Result<Option<PendingItem>>;
    async fn list_unresolved(&self) -> Result<Vec<PendingItem>>;

    #[cfg(feature = "creative")]
    async fn save_character(&self, c: &Character) -> Result<()>;
    #[cfg(feature = "creative")]
    async fn load_character(&self, id: CharacterId) -> Result<Option<Character>>;
}
```

デフォルト実装: `InMemoryStorage` (Phase 1)、`SqliteStorage` (Phase 2)

### LLMProvider

LLM 呼び出しの抽象。OpenAI 互換 API を第一実装に据えることで、Ollama / LM Studio / llama.cpp server の**すべてを単一実装でカバー**する。

```rust
pub trait LLMProvider: Send + Sync {
    async fn complete(&self, req: LLMRequest) -> Result<LLMResponse>;
    async fn stream(&self, req: LLMRequest) -> Result<BoxStream<LLMChunk>>;
    fn metadata(&self) -> &ModelMetadata;
}

pub struct LLMRequest {
    pub messages: Vec<Message>,
    pub max_tokens: Option<u32>,
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub stop: Vec<String>,
    pub grammar: Option<GrammarSpec>,      // 構造化出力制約
    pub tools: Vec<ToolSpec>,              // tool calling / function calling
    pub response_format: Option<ResponseFormat>, // json_mode / text
}

pub enum GrammarSpec {
    Gbnf(String),                          // llama.cpp GBNF (第一選択、移植性高)
    JsonSchema(serde_json::Value),         // JSON Schema → 各ランタイムで変換
    Regex(String),                         // 簡易制約
}
```

#### ModelMetadata

使用しているモデルの特性を保持。Context Compiler の動作調整や、製品側の UX (推奨モデルへのアップグレード提案等) で利用される。

```rust
pub struct ModelMetadata {
    pub name: String,                      // "qwen3-swallow-8b"
    pub family: ModelFamily,               // Qwen3, Gemma4, Llama, ...
    pub parameters_total: u64,             // 30_500_000_000
    pub parameters_active: Option<u64>,    // MoE で 3_300_000_000、dense なら None
    pub quantization: QuantizationLevel,
    pub context_window: u32,               // 32768, 131072, 262144 等
    pub supports_tools: bool,
    pub supports_json_mode: bool,
    pub supports_grammar: bool,
    pub language_focus: Vec<LanguageCode>, // ["ja", "en"] など
}

pub enum QuantizationLevel {
    Fp16,
    Fp8,
    Int8,                          // Q8_0
    Int6,                          // Q6_K
    Int5,                          // Q5_K_M
    Int4,                          // Q4_K_M, AWQ, GPTQ, MXFP4, MLX 4bit
    Int3,                          // Q3_K_M, IQ3_XXS
    Int2,                          // Q2_K, IQ2_M, MLX 2bit
    Ternary,                       // BitNet b1.58, Ternary Bonsai
    OneBit,                        // 1-bit Bonsai
    Unknown,
}

pub enum ModelFamily {
    Qwen3, Qwen35, Qwen36,
    Gemma3, Gemma4,
    Llama3, Llama4,
    Mistral, Mixtral,
    Phi4,
    Swallow,                       // Qwen3 Swallow, Llama Swallow 等の派生
    Elyza,
    Bonsai,                        // deepgrove / PrismML 系
    GptOss,
    GlmV5,
    KimiK,
    DeepseekV3,
    Other(String),
}
```

#### KV cache 量子化

長文脈でのメモリ節約のため、KV cache 量子化をリクエスト単位で指定可能にする:

```rust
pub struct LLMRequest {
    // ... 上記フィールド
    pub kv_cache_quantization: Option<KvCacheQuant>,
}

pub enum KvCacheQuant {
    None,                          // FP16
    Q8,
    Q5,
    Q4,
    MlxTurboQuant,                 // MLX ランタイム専用
}
```

これは Context Compiler が大量コンテキストを渡す際に製品側が設定する。つむぎコアは指定しない、製品の責任。

#### リファレンス実装

- `OpenAICompatibleProvider` — Ollama / LM Studio / llama.cpp server を包括
- `MockLLMProvider` — テスト用、固定レスポンス
- (将来) `AnthropicProvider`, `CloudflareWorkersAIProvider` 等の直接実装

ローカル推奨前提だが、製品側がユーザー選択でクラウドプロバイダを追加できる柔軟性を保つ (特につづりの AIのべりすと乗り換え層で、原稿の一部だけクラウドに投げたい等のユースケース)。

### EmbeddingProvider

```rust
pub trait EmbeddingProvider: Send + Sync {
    async fn embed(&self, text: &str) -> Result<Vec<f32>>;
    async fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>>;
    fn dim(&self) -> usize;
}
```

リファレンス実装: `MockEmbeddingProvider` / `LMStudioEmbeddingProvider` / `OllamaEmbeddingProvider`

### Retriever

```rust
pub trait Retriever: Send + Sync {
    async fn retrieve(
        &self,
        query: &str,
        query_embedding: Option<&[f32]>,
        candidates: &[ChunkId],
        top_k: usize,
    ) -> Result<Vec<RetrievalHit>>;
}

pub struct RetrievalHit {
    pub chunk_id: ChunkId,
    pub score: f32,
    pub bm25_score: Option<f32>,
    pub cosine_score: Option<f32>,
}
```

デフォルト実装: `HybridRetriever` (BM25 via lindera + cosine 類似度)、`Bm25Retriever`, `CosineRetriever`

### RelevanceScorer

```rust
pub trait RelevanceScorer: Send + Sync {
    fn score(&self, chunk: &Chunk, ctx: &ScoringContext) -> f32;
}

pub struct ScoringContext {
    pub current_chunk_id: Option<ChunkId>,
    pub current_time: DateTime<Utc>,
    pub current_order: Option<i64>,
    pub current_location: Option<SourceLocation>,   // ★ FileProximityScorer 用
    pub retrieval_hit: Option<&RetrievalHit>,
}
```

同梱実装:

- `TemporalDecayScorer { half_life: Duration }` — つかさ向け (セッション時刻ベース)
- `ChapterOrderScorer { decay_per_chapter: f32 }` — つづり向け (章順距離ベース)
- **`FileProximityScorer { path_distance_weight: f32, depth_weight: f32 }`** ★新規 — つくも向け。ファイルパス距離・ディレクトリ深さ差・モジュール依存グラフ距離
- `NoDecayScorer` — 時間・距離無関係のケース
- `CompositeScorer(Vec<Box<dyn RelevanceScorer>>)` — 複数スコアの合成

### FileProximityScorer の詳細

```rust
pub struct FileProximityScorer {
    pub path_distance_weight: f32,
    pub depth_weight: f32,
    pub max_distance: f32,
}

impl RelevanceScorer for FileProximityScorer {
    fn score(&self, chunk: &Chunk, ctx: &ScoringContext) -> f32 {
        let (Some(current), Some(chunk_loc)) = (&ctx.current_location, &chunk.source_location)
            else { return 0.5 }; // ロケーション不明なら中立

        // 共通接頭辞長に基づく類似度
        let common_prefix = longest_common_prefix(&current.path, &chunk_loc.path);
        let path_score = common_prefix as f32 / current.path.len().max(chunk_loc.path.len()) as f32;

        // ディレクトリ深さ差
        let depth_diff = (dir_depth(&current.path) as i32 - dir_depth(&chunk_loc.path) as i32).abs() as f32;
        let depth_score = (1.0 - depth_diff / self.max_distance).max(0.0);

        path_score * self.path_distance_weight + depth_score * self.depth_weight
    }
}
```

パスベースの簡易スコアリングが初期実装。Phase 2 でモジュール依存グラフを組み込んだより高度なスコアリングを検討。

### EventDetector

```rust
pub trait EventDetector: Send + Sync {
    type Event;
    async fn detect(&self, chunk: &Chunk, new_turn: &serde_json::Value) -> Result<Vec<Self::Event>>;
}
```

同梱実装:

- `KeywordDetector` — 文字列完全一致、即応、コスト 0
- `EmbeddingSimilarityDetector` — top-K 意味類似
- `LLMClassifierDetector` — 軽量 LLM による yes/no 判定
- `CascadeDetector` — Keyword → Embedding → LLM の 3 段カスケード (chatstream 流)

---

## Context Compiler

```rust
pub struct ContextCompiler {
    storage: Arc<dyn StorageProvider>,
    retriever: Arc<dyn Retriever>,
    scorer: Arc<dyn RelevanceScorer>,
    embedding: Arc<dyn EmbeddingProvider>,
}

pub struct CompiledContext {
    pub resident: ResidentLayer,
    pub dynamic: DynamicLayer,
}

pub struct ResidentLayer {
    pub current_scene_summary: Option<String>,
    pub recent_turns: Vec<String>,
    #[cfg(feature = "creative")]
    pub active_characters: Vec<Character>,
    pub style_hint: Option<String>,
}

pub struct DynamicLayer {
    pub related_past_chunks: Vec<Chunk>,
    pub related_lore: Vec<LoreEntry>,
    pub unresolved_pending: Vec<PendingItem>,
    pub supporting_facts: Vec<Fact>,
}
```

製品側は CompiledContext を受け取り、最終プロンプトを組み立てる (system プロンプト、few-shot 整形、style 反映等は製品の責任)。

### コンテキストサイズ予算

Context Compiler はモデルの `context_window` を `ModelMetadata` から取得して、動的レイヤーのサイズを調整する。大まかな予算割当:

- 出力予約: `max_tokens` 指定分
- system プロンプト: 500 トークン
- 常駐レイヤー: 全体の 30%
- 動的レイヤー: 全体の 50%
- バッファ: 20%

製品側がこれを override 可能。MoE モデル (30B-A3B 等) は速度が速いため文脈を大きく取っても実用、dense 小型モデルはコンテキストを絞る方が安定する場合が多い。

---

## Alloy モデル戦略

`models/tsumugi-core.als` と `models/tsumugi-creative.als` に分けて記述。

### tsumugi-core.als の内容

- Chunk, Fact, LoreEntry, PendingItem の sig
- 参照整合性 (ChunkId → Chunk, FactId → Fact 等)
- 階層の非循環 (parent チェーンに自分を含まない)
- PendingItem のライフサイクル (introduced_at < resolved_at、resolved_at 存在時のみ resolved 扱い)
- Fact の supersession 関係 (superseded_by が循環しない)

### tsumugi-creative.als の内容

- Character, SceneView, StylePreset の sig
- Character の first_appearance が有効な Chunk を指す

### 生成物

- Rust: `tsumugi-core/src/domain/gen/` + `tsumugi-core/src/creative/gen/` (feature gate 適用)
- TypeScript: `tsumugi-ts/src/gen/` (将来)

### 検証

- `oxidtr check` を CI で実行、Alloy モデルと手書き実装のズレを検知
- `oxidtr generate` を pre-commit フックで実行、生成コードの更新漏れを防ぐ

---

## 上位製品ごとの利用例

### つかさ (creative feature、フル活用)

```rust
use tsumugi_core::{StorageProvider, InMemoryStorage, Character, Scene, ...};

let storage = InMemoryStorage::new();
let scorer = TemporalDecayScorer { half_life: Duration::days(30) };
let compiler = ContextCompiler::new(storage, retriever, scorer, embedding);

let context = compiler.compile(current_chunk_id, query).await?;
```

### つづり (creative feature、章順スコアリング)

```rust
let scorer = ChapterOrderScorer { decay_per_chapter: 0.1 };
// 以下同じ
```

### つくも (default features、ファイル近接スコアリング)

```rust
use tsumugi_core::{StorageProvider, InMemoryStorage, ...};
// Character / Scene は import しない (feature 無効)

let scorer = FileProximityScorer {
    path_distance_weight: 0.6,
    depth_weight: 0.4,
    max_distance: 5.0,
};
// 以下同じ
```

---

## chatstream との関係

両者は兄弟ミドルウェア。共通する設計思想を持つ独立実装。

| 観点 | chatstream | tsumugi |
|---|---|---|
| 主ターゲット | 音声 AI デバイス | 長期プロジェクト (汎用) |
| Turn 抽象 | 音声入出力ペア | 非依存 (`Chunk.items: Value`) |
| 話題検知 | 3 段カスケード | `EventDetector` trait (将来共通化) |
| ドメイン状態 | facts 中心 | Chunk / Fact / LoreEntry / PendingItem + creative 拡張 |
| Stage 3 UI | 音声制約 (選択肢 2 個) | テキスト制約ほぼなし |
| レイテンシ要件 | 厳しい | 緩い (数秒許容) |

将来の統合可能性として、`EventDetector` trait を chatstream 側にも輸入して話題検知を差し替え可能にする道筋がある。現時点では各自独立実装、設計の互換性を保つ方針。

---

## 未確定論点 (Phase 1 以降で詰める)

- `Chunk.items` の serialize 形式 (JSON? MessagePack?)
- `SpeechTraits` の拡張性 (struct のまま vs Map)
- `PendingItem.kind` の型安全化 (String vs enum)
- sqlite-vec 採用のタイミング
- `Retriever` の BM25/cosine 重み調整 (定数 vs 学習)
- Context Compiler の結果キャッシュ戦略
- `EventDetector::Event` の型パラメータ化の妥当性
- tsumugi-ts の実装時期
- `SourceLocation` の表現の抽象度 (ファイルパス文字列 / 構造化表現 / URI)
- 追加 feature flag (`coding`, `research`) の設計指針
- `ModelFamily` enum の粒度 (enum が長すぎる懸念、Other(String) を許容するか)
- `GrammarSpec::JsonSchema` から各ランタイム固有形式への変換の責務 (provider 側か compiler 側か)
- KV cache 量子化のランタイム横断 API
