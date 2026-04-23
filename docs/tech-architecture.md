# つむぎ — 技術アーキテクチャ

## ツールチェーン

- 言語: Rust (コア) + TypeScript (SDK)
- パッケージマネージャ: cargo, bun
- ドメインモデル: [oxidtr](https://github.com/penta2himajin/oxidtr) (Alloy → Rust / TypeScript 型・テスト・不変条件)
- テスト: Rust の `#[test]` + 結合テスト、TypeScript の vitest (SDK 段階)
- 日本語トークナイザ: lindera (BM25 用)
- ベクトル検索: 初期は InMemory、将来 sqlite-vec 検討

推論ランタイム (上位製品での想定): Ollama (Apple Silicon で MLX バックエンド、2026-03〜) / LM Studio / llama.cpp。`LLMProvider` trait は OpenAI 互換 API をまず実装し、Ollama / LM Studio の両方を単一実装でカバーする。

ハードウェア帯域別の推奨モデルは `docs/runtime-environment.md` を参照。調査背景は `docs/research/2026-04-model-landscape.md` と `docs/research/context-management-survey.md` を参照。

## ワークスペース構成

```
tsumugi/
├── tsumugi-core/        # コアライブラリ (Rust crate)
│   ├── src/
│   │   ├── domain/      # ドメイン非依存の型 (Chunk, Fact, PendingItem, SourceLocation)
│   │   ├── creative/    # 創作拡張 (Character, Scene, StylePreset, LoreEntry) ★ feature = "creative"
│   │   ├── traits/      # 9 種の trait
│   │   ├── retriever/   # BM25 + cosine hybrid
│   │   ├── scorer/      # RelevanceScorer 実装群
│   │   ├── detector/    # EventDetector 実装群
│   │   ├── classifier/  # QueryClassifier 実装群 ★新
│   │   ├── compressor/  # PromptCompressor 実装群 ★新
│   │   ├── summarizer/  # Summarizer 実装群 ★新
│   │   └── compiler/    # Context Compiler
│   └── tests/           # 結合テスト (小説・TRPG・ツクールシナリオ各 1 本)
├── tsumugi-cli/         # 開発・検証用 REPL (Rust binary)
├── tsumugi-ts/          # TypeScript SDK (Phase 3 以降)
└── models/              # Alloy ソース (oxidtr 入力、multi-file 形式)
    ├── tsumugi.als      # main (module tsumugi; open tsumugi/{core,creative})
    └── tsumugi/
        ├── core.als     # module tsumugi/core
        └── creative.als # module tsumugi/creative (open tsumugi/core)
```

### Feature flag 方針

`tsumugi-core` の `Cargo.toml`:

```toml
[features]
default = []
creative = []
```

- `default`: ドメイン非依存コアのみ (つくもが使用)
- `creative`: Character / SceneView / StylePreset / **LoreEntry** を有効化 (つかさ・つづりが使用)

上位製品での依存指定例:

```toml
# つかさ / つづり
tsumugi-core = { path = "../tsumugi/tsumugi-core", features = ["creative"] }

# つくも
tsumugi-core = { path = "../tsumugi/tsumugi-core" }
```

別クレートに分けるより**軽量**で、コア側の機能追加が creative 側に自然に波及する。将来別ドメイン拡張が必要になれば、同じパターンで `coding`, `research` 等の feature を追加。

> **`creative` は暫定名**: 現状は創作 3 製品で共有される抽象の集合。将来 `story` 等への改名可能性あり (concept.md の設計決定表参照)。

---

## 処理パス概観

汎用コンテキストエンジンとしての 3 つの独立した処理パスを明示する。

### パス 1: 入力 → 保存

製品のドメインイベント (Turn / Commit / Paragraph 等) を受け取り、Chunk として保存する。

```
[Product Domain Event]
        ↓ serialize
[Value (JSON)]                       ← tsumugi との境界
        ↓
[Product が判断]
  ・新規 Chunk か、既存 Chunk への追記か
  ・親 Chunk は何か (階層構造は製品が知る)
        ↓
[Core: Chunk CRUD via StorageProvider]
        ↓
[EventDetector (trait): 事象検出]    ← Keyword → Embedding → LLM の cascade
        ↓
[Product: Event を受けて副次更新]
  ・Fact 作成/supersede
  ・PendingItem 作成/resolve
  ・(creative) LoreEntry 追加
        ↓
[Core: 索引更新 (非同期可)]
  ・Keyword index (lindera + BM25)
  ・Embedding index
```

**責任分担**: Chunk の階層構造維持と参照整合性は core、Fact / PendingItem の具体的な抽出ロジックは製品が `EventDetector` 経由で実装する。

### パス 2: 保存 → 選択的投入 (主処理パス)

現在文脈を与えて `CompiledContext` を組み立てる。**Tier 0-1 で完結する** のが中核設計。

```
[Product: 「今ここ」を指定]
  ・current_chunk_id, current_time, current_location, optional query
        ↓
[Tier 0 決定論フィルタ]              ← 階層走査、時間窓、superseded 除外
        ↓ (候補集合)
[Tier 0-1 QueryClassifier]           ← regex → (将来 BERT)、検索戦略切替
        ↓
[Tier 0-1 Retriever]                 ← BM25 + cosine ハイブリッド
        ↓ (RetrievalHit 群)
[Tier 0-1 RelevanceScorer]           ← TemporalDecay / ChapterOrder / FileProximity / NoDecay
        ↓ (スコア付き候補)
[Tier 2 PromptCompressor (任意)]     ← LLMLingua-2 / Selective Context、Phase 2〜
        ↓
[ContextCompiler: 層別組み立て]
  ・ResidentLayer (常駐)
  ・DynamicLayer  (動的)
        ↓
[CompiledContext] ────→ [Product: 最終プロンプト組み立てと LLM 呼び出し]
```

**重要**: このパスに `LLMProvider` は登場しない。最終生成は製品の責務。Tier 2-3 の `PromptCompressor` / `Summarizer` は内部で LLM を使うことがあるが、主処理パス (Tier 0-1) はすべて LLM 非依存で動く。

### パス 3: 要約 (非同期別パス)

主処理パスとは切り離された要約更新サイクル。

```
[トリガー]
  ・親 Chunk の children 数が閾値超過
  ・手動 (ユーザ編集 UI)
  ・schedule (バックグラウンド)
        ↓
[Summarizer (trait)]
  ・SummaryMethod 選択 (LlmFull / LlmLingua2 / SelectiveContext / ExtractiveBM25 / UserManual)
  ・SummaryLevel は u32 (0 = Raw、正数が抽象度)
        ↓
[Chunk.summary / summary_level / summary_method 更新]
  ・auto_update_locked=true なら skip
  ・edited_by_user=true は保護
```

選択的投入側は「Chunk.summary が適切に更新されている」前提で動作する。

---

## 4-tier 処理階層

| Tier | 粒度 | コスト目安 | 該当コンポーネント (例) |
|---|---|---|---|
| Tier 0 | 決定論 | μs 〜 ms | 正規表現、完全一致、BM25、階層走査、時間窓、supersession フィルタ |
| Tier 1 | CPU 軽量 | 数 ms | 小型 embedding (MiniLM / BGE-small)、BERT 分類器、IKE 二値化 |
| Tier 2 | GPU 中量 | 数十 ms | LLMLingua-2 圧縮、embedding 再ランク、軽量 LLM yes/no |
| Tier 3 | LLM フル | 数百 ms〜 | 階層要約生成、最終裁定抽出、最終生成 |

**帰結**:

- `Retriever` / `Scorer` / `QueryClassifier` の主系列は Tier 0-1
- `PromptCompressor` / `Summarizer` は Tier 2-3
- `EventDetector` は cascade で Tier 0 → 3 を段階評価
- `LLMProvider` は core の主処理パスから外れ、製品の最終生成と、一部 trait 実装 (Summarizer / LLMClassifierDetector 等) の内部呼び出しに限定

---

## 核心抽象 (Rust 型)

### コア (ドメイン非依存、default features)

#### Chunk

```rust
pub struct Chunk {
    pub id: ChunkId,
    pub text: String,                      // 正規化表示用テキスト
    pub items: Vec<serde_json::Value>,     // ドメイン Turn の serialize (summary_level != 0 では空)
    pub summary: String,
    pub keywords: HashSet<Keyword>,
    pub facts: Vec<FactId>,
    pub pending: Vec<PendingItemId>,
    pub parent: Option<ChunkId>,
    pub children: Vec<ChunkId>,
    pub metadata: serde_json::Map<String, Value>,
    pub last_active_at: DateTime<Utc>,
    pub order_in_parent: i64,
    pub source_location: Option<SourceLocationValue>,       // ★ sum 型 (B 案、2026-04-23 確定)

    // ★ 階層要約関連 (調査書 §5 統合)
    pub summary_level: u32,                // 0 = Raw (葉), 1+ は要約ノード、正数が高抽象度
    pub summary_method: SummaryMethod,
    pub edited_by_user: bool,
    pub auto_update_locked: bool,
}

pub enum SummaryMethod {
    LlmFull,              // Tier 3
    LlmLingua2,           // Tier 2
    SelectiveContext,     // Tier 2
    ExtractiveBM25,       // Tier 1
    UserManual,           // 人間が書いた
    None,                 // summary 未生成 (Raw 葉の初期状態)
}
```

**階層要約の不変条件** (Alloy で記述):

- `summary_level == 0` ⇒ `items` 非空 (生データの葉)
- `summary_level > 0` ⇒ `children` 非空 (要約ノードは子を持つ)
- 親子間で `parent.summary_level > child.summary_level` (抽象度は親が高い)
- `edited_by_user == true` かつ `auto_update_locked == true` は整合的 (手動更新を保護)

#### SourceLocation / SourceLocationValue

ファイルパス / URI / "session/3#scene2" 等の多様な表現に対応する。**2026-04-23 に B 案を確定** — `Chunk` は `SourceLocationValue` (sum 型) を保持し、`SourceLocation` trait は振る舞い (proximity 計算) の抽象として残す。値と trait を分離することで `Clone` / `Serialize` / `PartialEq` を自動導出でき、oxidtr / serde との境界がシンプルになる。

```rust
/// Chunk が保持する具体型。serialize 可能な sum 型。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "schema", rename_all = "kebab-case")]
pub enum SourceLocationValue {
    /// core 同梱の標準 variant。
    File(FileSourceLocation),
    /// 製品独自の SourceLocation。schema は識別子、payload は serialize 済みデータ。
    Custom { schema: String, payload: serde_json::Value },
}

/// 振る舞いの抽象。FileProximityScorer などがこの trait 越しに近接度を計算する。
pub trait SourceLocation: Send + Sync + Debug {
    fn schema(&self) -> &str;
    fn path(&self) -> &str;
    fn span(&self) -> Option<Range<usize>>;
    fn proximity(&self, other: &dyn SourceLocation) -> f32;
}

/// core 同梱の標準実装。ファイルシステム上のパスを扱う。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FileSourceLocation {
    pub path: String,
    pub span: Option<Range<usize>>,
}

impl SourceLocation for FileSourceLocation {
    fn schema(&self) -> &str { "file" }
    fn path(&self) -> &str { &self.path }
    fn span(&self) -> Option<Range<usize>> { self.span.clone() }
    fn proximity(&self, other: &dyn SourceLocation) -> f32 {
        if other.schema() != "file" { return 0.0; }
        // 共通接頭辞長 / ディレクトリ深さ差から近接度を計算
        /* ... */
    }
}

/// SourceLocationValue 自身も SourceLocation の振る舞いを提供 (variant にディスパッチ)。
impl SourceLocation for SourceLocationValue {
    fn schema(&self) -> &str {
        match self {
            SourceLocationValue::File(f) => f.schema(),
            SourceLocationValue::Custom { schema, .. } => schema,
        }
    }
    fn path(&self) -> &str { /* variant ごとに委譲 */ }
    fn span(&self) -> Option<Range<usize>> { /* variant ごとに委譲 */ }
    fn proximity(&self, other: &dyn SourceLocation) -> f32 {
        match self {
            SourceLocationValue::File(f) => f.proximity(other),
            // Custom の proximity は schema ごとの registry で解決 (Phase 2 以降で実装)
            SourceLocationValue::Custom { .. } => 0.0,
        }
    }
}

/// 製品独自実装は `SourceLocation` trait を実装しつつ、`Into<SourceLocationValue>` で保存形に変換する。
/// 例: TRPG の SessionLocation なら `From<SessionLocation> for SourceLocationValue` を実装して
/// `Custom { schema: "trpg-session", payload: serde_json::to_value(session_loc)? }` に畳む。
```

上位製品は必要に応じて独自実装を作れる (TRPG なら `SessionLocation`、ウェブなら `UriLocation` 等)。新しい schema の proximity 計算を core に載せたい場合は、Phase 2 以降で schema → proximity fn のレジストリを検討する。

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

#### SceneView (= 特殊化された Chunk ビュー)

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

#### LoreEntry (★ core から移設)

Lorebook 由来の keyword トリガー辞書。core からは外し、creative feature に配置する。

```rust
#[cfg(feature = "creative")]
pub struct LoreEntry {
    pub id: LoreEntryId,
    pub category: String,
    pub title: String,
    pub content: String,
    pub scope: LoreScope,
    pub keywords: Vec<Keyword>,
}

#[cfg(feature = "creative")]
pub enum LoreScope {
    Global,
    ChunkLocal(ChunkId),
    Conditional(String),
}
```

core の同等概念が必要になった場合は、`Chunk.metadata` や製品固有の feature で表現する (例: 将来の `coding` feature で `ArchitecturalDecision` を定義する等)。

---

## 核心 trait (9 種)

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

    async fn save_pending(&self, p: &PendingItem) -> Result<()>;
    async fn load_pending(&self, id: PendingItemId) -> Result<Option<PendingItem>>;
    async fn list_unresolved(&self) -> Result<Vec<PendingItem>>;

    #[cfg(feature = "creative")]
    async fn save_character(&self, c: &Character) -> Result<()>;
    #[cfg(feature = "creative")]
    async fn load_character(&self, id: CharacterId) -> Result<Option<Character>>;

    #[cfg(feature = "creative")]
    async fn save_lore_entry(&self, e: &LoreEntry) -> Result<()>;
    #[cfg(feature = "creative")]
    async fn load_lore_entry(&self, id: LoreEntryId) -> Result<Option<LoreEntry>>;
}
```

デフォルト実装: `InMemoryStorage` (Phase 1)、`SqliteStorage` (Phase 2)

### LLMProvider

LLM 呼び出しの抽象。OpenAI 互換 API を第一実装に据えることで、Ollama / LM Studio / llama.cpp server の**すべてを単一実装でカバー**する。

> core の主処理パス (Tier 0-1) からは LLMProvider は呼ばれない。最終生成は製品の責務であり、core 内部では `LLMClassifierDetector` / `Summarizer` 等の Tier 2-3 実装が任意で利用する。

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
    pub kv_cache_quantization: Option<KvCacheQuant>,
}

pub enum GrammarSpec {
    Gbnf(String),                          // llama.cpp GBNF (第一選択、移植性高)
    JsonSchema(serde_json::Value),         // JSON Schema → 各ランタイムで変換
    Regex(String),                         // 簡易制約
}
```

#### ModelMetadata

```rust
pub struct ModelMetadata {
    pub name: String,                      // "qwen3-swallow-8b"
    pub family: ModelFamily,
    pub parameters_total: u64,
    pub parameters_active: Option<u64>,    // MoE なら active、dense なら None
    pub quantization: QuantizationLevel,
    pub context_window: u32,
    pub supports_tools: bool,
    pub supports_json_mode: bool,
    pub supports_grammar: bool,
    pub language_focus: Vec<LanguageCode>,
}

pub enum QuantizationLevel {
    Fp16, Fp8, Int8, Int6, Int5, Int4, Int3, Int2,
    Ternary, OneBit, Unknown,
}

pub enum ModelFamily {
    Qwen3, Qwen35, Qwen36,
    Gemma3, Gemma4,
    Llama3, Llama4,
    Mistral, Mixtral,
    Phi4,
    Swallow, Elyza, Bonsai,
    GptOss, GlmV5, KimiK, DeepseekV3,
    Other(String),
}

pub enum KvCacheQuant {
    None, Q8, Q5, Q4, MlxTurboQuant,
}
```

#### リファレンス実装

- `OpenAICompatibleProvider` — Ollama / LM Studio / llama.cpp server を包括
- `MockLLMProvider` — テスト用、固定レスポンス
- (将来) `AnthropicProvider`, `CloudflareWorkersAIProvider` 等

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

pub struct ScoringContext<'a> {
    pub current_chunk_id: Option<ChunkId>,
    pub current_time: DateTime<Utc>,
    pub current_order: Option<i64>,
    pub current_location: Option<&'a SourceLocationValue>,   // B 案確定に伴い具体型に変更 (2026-04-23)
    pub retrieval_hit: Option<&'a RetrievalHit>,
}
```

同梱実装:

- `TemporalDecayScorer { half_life: Duration }` — つかさ向け (セッション時刻ベース)
- `ChapterOrderScorer { decay_per_chapter: f32 }` — つづり向け (章順距離ベース)
- `FileProximityScorer { path_distance_weight, depth_weight, max_distance }` — つくも向け、`SourceLocation::proximity` を利用
- `NoDecayScorer` — 時間・距離無関係のケース
- `CompositeScorer(Vec<Box<dyn RelevanceScorer>>)` — 複数スコアの合成

### EventDetector

```rust
pub trait EventDetector: Send + Sync {
    type Event;
    async fn detect(&self, chunk: &Chunk, new_turn: &serde_json::Value) -> Result<Vec<Self::Event>>;
}
```

同梱実装 (Tier 別):

- `KeywordDetector` (Tier 0) — 文字列完全一致、即応、コスト 0
- `EmbeddingSimilarityDetector` (Tier 1) — top-K 意味類似
- `LLMClassifierDetector` (Tier 2-3) — 軽量 LLM による yes/no 判定
- `CascadeDetector` — Keyword → Embedding → LLM の 3 段カスケード (chatstream 流)

### QueryClassifier ★新

クエリ種別を判定して Retriever / Scorer の構成を切り替える。SelRoute 流 (`context-management-survey.md` §4.1)。

```rust
pub trait QueryClassifier: Send + Sync {
    async fn classify(&self, query: &str) -> Result<QueryType>;
}

pub enum QueryType {
    Temporal,        // 時間的 ("最近の〜", "前の章で〜")
    Semantic,        // 意味的 ("〜に似た〜")
    Factual,         // 事実参照 ("〜の値は?")
    Comparative,     // 比較 ("A vs B")
    MultiHop,        // 多段推論
    Other,
}
```

同梱実装:

- `RegexClassifier` (Tier 0) — Phase 1 実装
- (将来) `BertClassifier` (Tier 1) — Phase 3 で MiniLM / ModernBERT ベース

### PromptCompressor ★新

プロンプト圧縮。LLMLingua-2 / Selective Context / truncation 等 (`context-management-survey.md` §4.2)。

```rust
pub trait PromptCompressor: Send + Sync {
    async fn compress(&self, text: &str, target_ratio: f32) -> Result<String>;
}
```

同梱実装:

- `TruncateCompressor` (Tier 0) — 単純截断、Phase 1 最小実装
- (将来) `LlmLinguaCompressor` (Tier 2) — Phase 2
- (将来) `SelectiveContextCompressor` (Tier 2) — Phase 2

### Summarizer ★新

階層要約の生成。RAPTOR 流 (`context-management-survey.md` §5)。

```rust
pub trait Summarizer: Send + Sync {
    /// 子 Chunk 群を要約して新しい要約ノード用のテキストと method を返す。
    async fn summarize(
        &self,
        chunks: &[Chunk],
        target_level: u32,
    ) -> Result<SummarizerOutput>;
}

pub struct SummarizerOutput {
    pub summary: String,
    pub method: SummaryMethod,
}
```

同梱実装:

- `ExtractiveBM25Summarizer` (Tier 1) — Phase 2 最小実装
- (将来) `LlmSummarizer` (Tier 3) — Phase 2
- (将来) `HierarchicalSummarizer` — 複数 Summarizer を組み合わせて level ごとに手法を切替 (Phase 2-3)

---

## Context Compiler

```rust
pub struct ContextCompiler {
    storage: Arc<dyn StorageProvider>,
    retriever: Arc<dyn Retriever>,
    scorer: Arc<dyn RelevanceScorer>,
    embedding: Arc<dyn EmbeddingProvider>,
    classifier: Option<Arc<dyn QueryClassifier>>,    // 任意、Phase 1 で追加
    compressor: Option<Arc<dyn PromptCompressor>>,   // 任意、Phase 2 で追加
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
    pub unresolved_pending: Vec<PendingItem>,
    pub supporting_facts: Vec<Fact>,
    #[cfg(feature = "creative")]
    pub related_lore: Vec<LoreEntry>,
}
```

製品側は `CompiledContext` を受け取り、最終プロンプトを組み立てる (system プロンプト、few-shot 整形、style 反映等は製品の責任)。

### コンテキストサイズ予算

Context Compiler はモデルの `context_window` を `ModelMetadata` から取得して、動的レイヤーのサイズを調整する。大まかな予算割当:

- 出力予約: `max_tokens` 指定分
- system プロンプト: 500 トークン
- 常駐レイヤー: 全体の 30%
- 動的レイヤー: 全体の 50%
- バッファ: 20%

製品側がこれを override 可能。

---

## Alloy モデル戦略

`models/tsumugi.als` (main) から `models/tsumugi/{core,creative}.als` を `open` で取り込む multi-file 形式。oxidtr の multi-file 対応 (parse_from_path が `open` を辿って transitive に解決) を利用し、Rust backend は module ごとに 1 ファイルを生成する。

### models/tsumugi.als (main)

- `module tsumugi` 宣言
- `open tsumugi/core` / `open tsumugi/creative`
- クロスモジュール不変条件 (creative ↔ core の参照整合性など)

### models/tsumugi/core.als の内容

- Chunk, Fact, PendingItem の sig
- SourceLocation は sum sig として定義 (`FileSourceLocation` + `CustomSourceLocation { schema, payload }`)。Rust 側では `SourceLocationValue` enum に対応 (B 案、2026-04-23 確定)
- 参照整合性 (ChunkId → Chunk, FactId → Fact 等)
- 階層の非循環 (parent チェーンに自分を含まない)
- **階層要約の不変条件**:
  - `summary_level = 0` ⇒ `items` 非空
  - `summary_level > 0` ⇒ `children` 非空
  - 親子間で親の `summary_level` > 子の `summary_level`
- PendingItem のライフサイクル (introduced_at ≤ resolved_at、resolved_at 存在時のみ resolved 扱い)
- Fact の supersession 関係 (superseded_by が循環しない)

### models/tsumugi/creative.als の内容

- Character, SceneView, StylePreset, **LoreEntry** の sig
- Character の first_appearance が有効な Chunk を指す
- LoreEntry.scope の Conditional 文字列は非空

### 生成物

- Rust: `tsumugi-core/src/gen/` に `tsumugi/{core,creative}/` サブツリーを展開。lib.rs は `#[path = "gen/tsumugi"] pub(crate) mod tsumugi { pub mod core; pub mod creative; }` で型サブツリーのみを取り込む。`creative` feature は `creative.rs` 経由の re-export レイヤーで gate する (モジュール全体 gate、生成コード自体は常時コンパイル)
- oxidtr が生成する scaffolding (`helpers.rs` / `operations.rs` / `newtypes.rs` / `fixtures.rs` / `tests.rs` / 最上位 `mod.rs`) は Phase 0 時点では未採用のため `.gitignore` で除外 (一部は `todo!()` / コード生成の既知の不具合あり、Phase 1 で選択的に wire 予定)
- TypeScript: `tsumugi-ts/src/gen/` (将来、Phase 3+)

### 再生成

`scripts/regen.sh` で再生成する:

```bash
# デフォルト (oxidtr は ../oxidtr にクローン済みとして扱う)
scripts/regen.sh

# 明示
scripts/regen.sh /path/to/oxidtr
OXIDTR_HOME=/path/to/oxidtr scripts/regen.sh
```

スクリプトは oxidtr を release ビルドしてから generate を実行し、最後に `cargo check --all-features` で生成コードがコンパイルすることを確認する。

### 検証

- `oxidtr check` を CI で実行、Alloy モデルと生成コードのズレを検知 (Phase 1 以降で CI ワイヤリング)
- `scripts/regen.sh` を手動 (または pre-commit) で実行、`models/` 変更後に gen/ を同期させる

### 警告方針 (2026-04-23 棚卸し)

oxidtr の警告 36 件を以下の方針で精査し、4 件の false positive まで削減:

- **UnconstrainedSelfRef** (`edited_by_user` / `auto_update_locked`): Alloy から除去し Rust 側の runtime flag として扱う (構造的不変条件でなく UX フラグのため)
- **UnconstrainedCardinality** (`children` / `items` / `facts` / `pending` / `participants`): tsumugi はスケール非依存のため `#x.field = #x.field` の tautology fact で silence (oxidtr self-host 慣例)
- **UnreferencedSig** (SceneView / StylePreset / LoreEntry / LoreScope 変種): Rust 側で context compiler 経由で利用されるスタンドアロン sig のため `pred useX[x: X] { x = x }` でマーク
- **UnhandledResponsePattern** (data-carrying 変種 `File` / `Custom` / `GlobalScope` / `ChunkLocalScope`): 同上 `pred useX` でマーク
- **UnconstrainedTransitivity** (`^superseded_by`): 直接 fact `SupersededByDirect` で silence
- **残る 4 件の MissingInverse** (`PendingItem.expected_resolution_chunk` / `resolved_at` × `Chunk.pending`): 設計上 reference-only (ownership link ではない) のため false positive として受容、`core.als` にコメントで rationale 記載

---

## 上位製品ごとの利用例

### つかさ (creative feature、フル活用)

```rust
use tsumugi_core::{StorageProvider, InMemoryStorage, Character, Scene, ...};

let storage = InMemoryStorage::new();
let scorer = TemporalDecayScorer { half_life: Duration::days(30) };
let classifier = RegexClassifier::default();
let compiler = ContextCompiler::new(storage, retriever, scorer, embedding)
    .with_classifier(classifier);

let context = compiler.compile(current_chunk_id, query).await?;
```

### つづり (creative feature、章順スコアリング)

```rust
let scorer = ChapterOrderScorer { decay_per_chapter: 0.1 };
// 以下同じ
```

### つくも (default features、ファイル近接スコアリング)

```rust
use tsumugi_core::{StorageProvider, InMemoryStorage, FileSourceLocation, ...};
// Character / Scene / LoreEntry は import しない (feature 無効)

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
| ドメイン状態 | facts 中心 | Chunk / Fact / PendingItem + creative 拡張 |
| Stage 3 UI | 音声制約 (選択肢 2 個) | テキスト制約ほぼなし |
| レイテンシ要件 | 厳しい | 緩い (数秒許容) |

将来の統合可能性として、`EventDetector` trait を chatstream 側にも輸入して話題検知を差し替え可能にする道筋がある。現時点では各自独立実装、設計の互換性を保つ方針。

---

## 未確定論点 (Phase 1 以降で詰める)

> 注記: 以下は tech-architecture 固有の実装判断に限定する。プロジェクト全体の大論点は `TODO.md` §未確定の大論点 に集約。

- `Chunk.items` の serialize 形式 (JSON? MessagePack?)
- `SpeechTraits` の拡張性 (struct のまま vs Map)
- `PendingItem.kind` の型安全化 (String vs enum)
- `Retriever` の BM25/cosine 重み調整 (定数 vs 学習)
- Context Compiler の結果キャッシュ戦略
- `EventDetector::Event` の型パラメータ化の妥当性
- `ModelFamily` enum の粒度 (enum が長すぎる懸念、`Other(String)` を許容するか)
- `GrammarSpec::JsonSchema` から各ランタイム固有形式への変換の責務 (provider 側か compiler 側か)
- KV cache 量子化のランタイム横断 API
- `SourceLocation::proximity` のシグネチャ (現状 f32 だが、異種比較の戻し方を検討)
- `Summarizer::summarize` で複数 method を試す場合の合成ルール

### Phase 1 型定義時に決める実装判断 (2026-04-23 追加)

docs 整理中に浮上した、実装着手時点で具体判断が必要な論点:

- **`Chunk.source_location` の持ち方** ✅ **確定 (2026-04-23): B 案採用**
  - 採用: `Option<SourceLocationValue>` (sum 型)。`File(FileSourceLocation)` と `Custom { schema, payload }` の 2 variants。`SourceLocation` trait は振る舞いの抽象として存続し、`impl SourceLocation for SourceLocationValue` で variant へディスパッチする。
  - 決定根拠: (1) `Clone` / `Serialize` / `PartialEq` が自動導出可能で oxidtr / serde との相性が最良、(2) FileProximityScorer の hot path がゼロコスト、(3) 将来 C 案 (ID lookup) が必要になっても段階的移行が可能。
  - 不採用: A 案 (dyn trait 保持 + 手動 serialize) は拡張性は高いが実装コストが見合わない。typetag による自動 serialize は WASM / tsumugi-ts target で registration が効かないリスクがあり見送り。C 案 (ID lookup) は SourceLocation 自体のライフサイクル (リネーム追従など) が主要 use case でないため時期尚早。
  - 残課題: `Custom` variant の proximity 計算をどう拡張可能にするか (schema → proximity fn のレジストリ) は Phase 2 以降で検討。
- **`SummaryMethod::None` と `summary_level == 0` の整合**: Raw 葉 (`summary_level == 0`) は「まだ要約されていない」状態であり `summary_method == None` が自然だが、ランタイムで強制する方法が 2 通りある:
  - **A. Alloy 不変条件 + ランタイム assert**: `summary_level = 0 iff summary_method = None` を Alloy で宣言、Rust 側は `Chunk::new_raw` / `Chunk::new_summary` のコンストラクタで担保。
  - **B. 型レベルで分離**: `enum ChunkBody { Raw { items: Vec<Value> }, Summarized { level: NonZeroU32, method: SummaryMethod } }` のように代数的データ型で不正状態を排除。構造は堅いが、`Chunk` 全体が 2 variants に割れ、Retriever / Scorer のパターンマッチが増える。

  **A を既定** とし、B は Phase 2 以降の不正状態が実際に頻発した場合に昇格を検討する。
- **`ScoringContext<'a>` のライフタイム設計**: B 案確定 (上記) により `current_location: Option<&'a SourceLocationValue>` を暫定採用。dyn trait object ではなく具体 enum の参照となるため、非同期 Scorer 境界 (`Send + 'static`) での扱いは dyn 版より素直。ただし下記は依然として Phase 1 結合テストで実測:
  - `CompositeScorer(Vec<Box<dyn RelevanceScorer>>)` で `ctx` を fan-out する際のライフタイム制約
  - 将来の非同期 Scorer (`AsyncRelevanceScorer` 等) で await を跨ぐ必要が出た場合、`Option<SourceLocationValue>` (owned) または `Option<Arc<SourceLocationValue>>` への切り替えを検討
  - 参照で押し通せない場合は owned variant に切り替える (`SourceLocationValue` は `Clone` 可能なのでコストは許容範囲)

---

*最終更新: 2026-04-23*
