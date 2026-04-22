# つむぎ — 技術アーキテクチャ

## ツールチェーン

- 言語: Rust (コア) + TypeScript (SDK)
- パッケージマネージャ / 開発ランタイム: cargo, bun
- ドメインモデル定義: oxidtr (Alloy → Rust / TypeScript 型・テスト・不変条件)
- テスト: Rust の `#[test]` + インテグレーションテスト、TypeScript の vitest (SDK 段階)

## ワークスペース構成

```
tsumugi/
├── tsumugi-core/        # コアライブラリ (Rust crate)
├── tsumugi-cli/          # 開発・検証用REPL (Rust binary)
├── tsumugi-ts/           # TypeScript SDK (後日)
└── models/               # Alloy ソース (oxidtr 入力)
```

`tsumugi-server/` は当面作らない。上位製品 (つかさ / つづり / つくも) は Tauri アプリとしてコアを直接埋め込むため、独立サーバーは不要。

## ドメインモデル (概要)

以下は最終的に Alloy で記述される。現時点での想定構造:

```
Turn (sum type)
  ├── Dialogue   (speaker: CharacterId, text: String)
  ├── Narration  (text: String)
  ├── Action     (actor: CharacterId, verb: String, target?: EntityId)
  ├── Passage    (text: String, edit_history: Seq<Edit>)
  ├── Directive  (text: String)    # ユーザーから AI への指示
  └── Meta       (text: String)    # OOC / 注釈

Chunk
  ├── id: ChunkId
  ├── turns: Seq<Turn>
  ├── summary: String
  ├── keywords: Set<Keyword>
  ├── facts: Set<Fact>
  ├── pending: Set<PendingItem>
  ├── parent: Option<ChunkId>
  ├── children: Seq<ChunkId>
  └── last_active_at: DateTime

Character
  ├── id: CharacterId
  ├── name: String
  ├── sheet: Map<Attribute, Value>   # ドメイン固有 (HP, 性格等)
  └── style_examples: Seq<String>    # 口調のfew-shot 用

Scene
  ├── id: SceneId
  ├── chunk_ref: ChunkId
  ├── location: Option<LocationId>
  ├── participants: Set<CharacterId>
  └── time_marker: Option<String>

LoreEntry
  ├── id: LoreEntryId
  ├── category: LoreCategory (World / Faction / Item / History / ...)
  ├── title: String
  ├── content: String
  └── scope: LoreScope (Global / ChunkLocal { chunk_id })

Fact
  ├── key: String
  ├── value: String
  ├── scope: FactScope
  └── superseded: Option<String>
```

詳細な Alloy 定義は `models/tsumugi.als` に置く。

## Trait 抽象化

以下は実装差し替え可能にするため trait として定義する。

```rust
pub trait StorageProvider {
    async fn save_chunk(&mut self, chunk: &Chunk) -> Result<()>;
    async fn load_chunk(&self, id: ChunkId) -> Result<Option<Chunk>>;
    async fn save_character(&mut self, c: &Character) -> Result<()>;
    async fn load_character(&self, id: CharacterId) -> Result<Option<Character>>;
    // ... Scene, LoreEntry, Fact
}

pub trait EmbeddingProvider {
    async fn embed(&self, text: &str) -> Result<Vec<f32>>;
    fn dim(&self) -> usize;
}

pub trait LLMProvider {
    async fn complete(&self, req: LLMRequest) -> Result<LLMResponse>;
    async fn stream(&self, req: LLMRequest) -> Result<BoxStream<LLMChunk>>;
}
```

初期実装は `InMemoryStorage` + `MockEmbedding` + `LMStudioProvider` でテスト可能にする。

## Context Compiler

各ターンの LLM 呼び出しに渡すコンテキストは、以下の層で構成する。

### 常駐レイヤー (毎回含まれる)

- ルート状態要約 (現在の話題・シーン)
- 直近 N ターン (生)
- 現在シーン参加キャラクターの sheet (圧縮形式)

### 動的レイヤー (必要に応じて注入)

- 関連する過去 chunk の要約または生データ
- 関連する lore entry
- 未解決の pending plot

### 注入判定

- 直近参照 ... 常駐レイヤーでカバー
- 長距離参照 ... 話題／シーン切替検知で復元
- エンティティ参照 ... Character / Location 名の言及検出で sheet / lore を動的注入

## 話題／シーン切替検知

chatstream と同じ 3 段カスケード方式を基本とし、創作ドメインに合わせて閾値と Stage 3 UI を調整する。

- Stage 1: 埋め込み類似度 (低コスト、即決優先)
- Stage 2: 軽量 LLM 分類 (グレーゾーン)
- Stage 3: ユーザー確認 (テキスト UI では選択肢一覧提示で OK、音声と違い候補数制約なし)

## 形式仕様駆動の開発フロー

1. `models/tsumugi.als` にドメインモデルを Alloy で記述
2. `oxidtr generate models/tsumugi.als --target rust --output tsumugi-core/src/gen/` で骨格生成
3. `oxidtr generate models/tsumugi.als --target ts --output tsumugi-ts/src/gen/` で TS 型生成
4. 生成された型・不変条件を元にビジネスロジックを手書き
5. `oxidtr check` を CI に組み込み、モデル ⇔ 実装のズレを検知

## chatstream との棲み分け

| 観点 | chatstream | つむぎ |
|---|---|---|
| 主ターゲット | 音声 AI デバイス | テキスト創作 |
| Turn モデル | 対称な入出力ペア | 多型 (dialogue/narration/action/passage/...) |
| ドメイン状態 | facts のみ | Character / Scene / LoreEntry / PendingPlot 等を独立レイヤーで保持 |
| Stage 3 UI 制約 | 音声 (選択肢最大 2 つ) | テキスト (制約緩い) |
| レイテンシ要件 | 厳しい (即決率が生命線) | 相対的に緩い (数秒は許容) |

ただし**階層的コンテキスト管理**という中核技術は共通しており、将来的に chatstream の話題検知モジュールをつむぎに差し込めるよう、検知レイヤーは trait で抽象化する。

## 未確定論点

- Turn の表現: sum type (enum) か、`trait Turn` + Box<dyn Turn> か
- Character sheet の schema 固定度 (ドメインごとに違うので Map<Attribute, Value> で柔軟にするか、TypedBuilder で型付けするか)
- Scene と Chunk の関係 (1:1 か、Chunk が複数 Scene を持てるか)
- lore entry の embedding 戦略 (category ごとに別 index か、単一 index でフィルタか)
- RAG 層の設計 (BM25 + cosine のハイブリッドか、純 cosine か)

## 参考文献 / 先行研究

- Park et al. 2023, *Generative Agents: Interactive Simulacra of Human Behavior* — 記憶減衰、反省サイクル
- RAPTOR (ICLR 2024) — 階層的要約の retrieval
- VARS (arXiv 2603.20939) — preference memory cards
- CarMem (COLING 2025) — append/pass/update 操作
- ContextBranch (arXiv 2512.13914, 2025) — Git 的対話分岐
- Howard & Kahana 2002, TCM — 時間的文脈モデル
