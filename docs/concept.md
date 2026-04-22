# つむぎ — コンセプト資料

## 一言で

**「創作向け AI エージェントに長期一貫性を与えるための、ドメイン非依存ライブラリ。」**

---

## プロダクトビジョン

つむぎは、LLM API の前段に配置するコンテキスト管理ミドルウェアである。物語構造を持つ長期セッション（TRPG キャンペーン、長編小説、同一プロジェクトでの継続的な AI 対話）で、以下の課題を解決するライブラリを提供する:

- LLM のコンテキストウィンドウ制約を超えた**情報永続化**
- 話題・シーン切替の**自動検知**
- キャラクター / 世界観 / 伏線 / 裁定の**動的注入**
- 時系列 / 章順 / 文脈距離に応じた**関連度スコアリング**

つむぎは**製品ではなく、製品を支えるコアエンジン**である。最終利用者に直接売られるのではなく、上位製品 (つかさ / つづり / 将来的に他ドメイン) に組み込まれて機能を発揮する。

### なぜ今か

- ローカル LLM の実用品質到達により、買い切り × オフライン動作の創作 AI 市場が成立
- AIのべりすと / NovelCrafter / SillyTavern 等のツールが**静的な辞書管理**に留まり、動的なコンテキスト管理の空白が残る
- つかさ / つづりの bottom-up 分析から、両製品で共通する抽象が明確化した

---

## 設計原則

### 1. Bottom-up からの抽出

つむぎの設計は、**つかさ / つづりで実際に必要になった機能**から抽出された。推測で作られた抽象ではない。そのため:

- 使われない機能は含まない
- 上位製品の実需に裏打ちされた API のみを提供
- 将来の製品追加時は、その製品の要件から再抽出する

### 2. Turn 非依存のコア

Turn の具体型（Dialogue / Narration / Check / Edit 等）は製品ごとに大きく異なる。つむぎはこれらの**具体型を知らない**。

- `Chunk.text: String` — 正規化された表示用テキスト
- `Chunk.items: Vec<serde_json::Value>` — 製品ごとのドメイン型 (serialize 済み)

製品側が Turn 型を定義し、serialize してつむぎに渡す。つむぎは text を対象に検索・要約し、items は参照として保持するだけ。

### 3. Trait 駆動の拡張性

以下の 6 つの trait で主要な差し替え点を切る:

- `StorageProvider` — 永続化層 (in-memory / SQLite / 独自)
- `EmbeddingProvider` — 埋め込み生成 (API / ローカル / mock)
- `LLMProvider` — LLM 呼び出し (LM Studio / Ollama / クラウド)
- `Retriever` — 検索戦略 (BM25 + cosine ハイブリッドがデフォルト)
- `RelevanceScorer` — 関連度スコア (時間減衰 / 章順 / その他)
- `EventDetector` — 話題切替 / トリガー検知

各 trait には合理的なデフォルト実装を同梱し、製品は必要部分だけ差し替える。

### 4. Alloy による形式仕様 + oxidtr 生成

ドメインモデルは Alloy で記述し、oxidtr で Rust / TypeScript の型・テスト・不変条件を自動生成する。

Alloy に書くもの:
- 型構造 (newtype ID、enum、record フィールド)
- 参照整合性 (dangling ID 防止)
- 主要な不変条件 (階層の非循環、pending items の寿命)

Rust に書くもの:
- 業務ロジック
- 検索・スコアリング・プロンプト組み立て

### 5. 創作ファースト

既存の会話エージェントライブラリ (LangChain Memory 等) は「対話の履歴管理」を主目的とする。つむぎは**創作構造**を第一級で扱う:

- Character は voice_samples を持ち、few-shot 注入できる
- PendingItem は未解決を明示的に追跡できる
- LoreEntry は scope (Global / ChunkLocal) を持つ
- Scene は Chunk の特殊化として階層に組み込まれる

---

## 消費者 (上位製品)

### 現在の実需者

| 製品 | 用途 | つむぎの利用度 |
|---|---|---|
| [つかさ](https://github.com/penta2himajin/tsukasa) | TRPG GM 補助 | フル活用 (階層 / 話題検知 / Fact / Character / LoreEntry / PendingItem) |
| [つづり](https://github.com/penta2himajin/tsuzuri) | 小説執筆補助 | 7 割活用 (recency 不要、章順 Scorer 差し替え) |

### 将来検討

- [つくも](https://github.com/penta2himajin/tsukumo): 軽量利用 (Fact + LoreEntry のみ。階層 / 検知不要)
- [chatstream](https://github.com/penta2himajin/chatstream): 音声 AI 向け、将来的に抽象共通化検討

### 明示的に対象外

- polarist-ai: 異なるドメイン (セッションレス SaaS)、chatstream ベースで構築済み

---

## 他ライブラリとの位置付け

### SillyTavern / AIのべりすと の Lorebook / Codex

- 静的辞書、ユーザーが手動管理
- トリガー語で注入されるが、コンテキスト枠を圧迫

**つむぎとの差**: 動的階層、距離スコアリング、EventDetector による自動検知

### LangChain Memory / Mem0

- 会話履歴管理中心、Buffer / Summary / Hybrid
- 対話エージェント前提、創作構造の概念なし

**つむぎとの差**: 創作ドメイン (Character / Scene / LoreEntry / PendingPlot) を第一級

### RAPTOR / VARS / CarMem

- 学術研究、階層要約 / preference memory / append-update 操作
- それぞれ部分的な機能

**つむぎとの差**: 実装された Rust ライブラリ、上位製品で使用可能、日本語特化

### chatstream

- 音声 AI 向け、話題検知 3 段カスケード
- 同じ設計思想 (階層、trait 抽象、oxidtr 駆動) を共有する**兄弟プロジェクト**

**つむぎとの差**: 創作ドメイン (text)、chatstream は対話ドメイン (voice)。将来的に共通基盤化を検討

---

## 設計決定の記録

つかさ / つづりの bottom-up 分析から導出された 10 論点の決着:

| 論点 | 決定 | 理由 |
|---|---|---|
| Turn 表現 | **Turn 非依存**、`Chunk.items: Vec<Value>` に製品が serialize | ドメイン Turn 型が製品ごとに根本的に違う |
| Character sheet | **`sheet: Map<String, Value>`** の自由形式、共通フィールドのみ型付け | CoC SAN vs 小説 backstory は共通化できない |
| Scene と Chunk | **Scene = Chunk + metadata タグ**、階層は単一機構 | 両製品とも 3 層の階層を持ち、Scene を別 entity にすると重複 |
| LoreEntry embedding | **`EmbeddingStore` trait**、デフォルトは長さで判定 | 短い固有名詞は不要、長い世界観説明は必要 |
| RAG hybrid | **BM25 + cosine**、日本語は lindera | 両方の強みが必要、`Retriever` trait で差し替え |
| RelevanceScorer | **trait 化**、`TemporalDecay` / `ChapterOrder` / `NoDecay` 同梱 | つかさ・つづり・つくもで減衰モデルが質的に異なる |
| EventDetector | **trait 化**、3 段カスケードを `CascadeDetector` で chain | つかさのトリガー・つづりの伏線活性化・chatstream 話題切替の共通構造 |
| chatstream 統合 | **当面独立**、抽象を互換に保つ | 早期統合は設計を縛る、将来統合の余地は残す |
| Alloy 粒度 | **中間**: 型 + 参照整合 + 主要ライフサイクル | 完全制約は重い、型のみでは価値薄い |
| 配布形態 | **`core` + `cli` + `ts` の 3 クレート**、`kv` サブクレート分離は保留 | つくも方針が固まってから再検討 |

---

## 現在のフェーズ

Phase 0: 設計固め (bottom-up 抽出完了、Alloy モデル初版着手中)

次フェーズの入り口:
- Alloy モデル `models/tsumugi.als` 初版作成
- oxidtr 生成パイプライン動作確認
- `StorageProvider` / `EmbeddingProvider` / `LLMProvider` の trait 定義
- in-memory 実装と結合テスト

---

*最終更新: 2026-04-22*
