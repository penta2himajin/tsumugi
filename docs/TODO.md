# つむぎ — TODO

フェーズ別。つかさ MVP と同時並行での Phase 1 実装を想定。

## Phase 0: 設計固め (現在、完了間近)

- [x] bottom-up 抽出: つかさ・つづり・つくもの docs 深掘り完了
- [x] 10 論点への決着 (concept.md / tech-architecture.md に反映済み)
- [x] 汎用コンテキストエンジンへの再ポジショニング (2026-04-22)
- [x] `FileProximityScorer` の追加と `SourceLocation` 追加を反映
- [x] `creative` feature flag による分離方針を反映
- [x] **調査書 (`context-management-survey.md`) 統合** (2026-04-23)
  - [x] LoreEntry を core から creative feature へ移設
  - [x] Chunk に 4 フィールド追加 (`summary_level: u32`, `summary_method`, `edited_by_user`, `auto_update_locked`)
  - [x] SummaryLevel は u32 (0 = Raw、正数が抽象度) とする方針確定
  - [x] 階層要約は既存 Chunk 拡張で表現 (新規 `HierarchicalSummary` 型は作らない)
  - [x] trait を 6 種から 9 種へ拡張 (`QueryClassifier` / `PromptCompressor` / `Summarizer`)
  - [x] 4-tier 処理階層を設計原則に明文化
  - [x] `SourceLocation` を trait 化、core 標準実装 `FileSourceLocation` を同梱
  - [x] 入力→保存 / 選択的投入 / 要約非同期 の 3 処理パスを明文化
  - [x] `creative` は暫定名であり改名可能性を docs に記載
- [ ] Alloy モデル `models/tsumugi-core.als` 初版
  - [ ] Chunk, Fact, PendingItem の sig (LoreEntry は creative へ)
  - [ ] 参照整合性の述語
  - [ ] 階層非循環の不変条件
  - [ ] **階層要約の不変条件** (`summary_level == 0` ⇒ items 非空、`> 0` ⇒ children 非空、親 > 子)
  - [ ] PendingItem ライフサイクルの不変条件
  - [ ] Fact supersession の非循環
  - [ ] SourceLocation は抽象 sig として定義 (実装は Rust 側 trait)
- [ ] Alloy モデル `models/tsumugi-creative.als` 初版
  - [ ] Character, SceneView, StylePreset, **LoreEntry** の sig
  - [ ] Character の first_appearance 整合性
  - [ ] LoreEntry.scope Conditional の非空制約
- [ ] oxidtr 生成フロー動作確認
  - [ ] core と creative の分離生成
- [ ] ワークスペース skeleton
  - [ ] `tsumugi-core/` の Cargo.toml + src/ 雛形
  - [ ] `creative` feature flag の設定
  - [ ] `tsumugi-cli/` の Cargo.toml + main.rs 雛形
  - [ ] `tsumugi-ts/` の package.json + tsconfig 雛形

## Phase 1: コア実装 (つかさ MVP と並行)

### A. 型定義とストレージ

- [ ] `tsumugi-core/src/domain/` 手書き拡張 (Alloy 生成 + 追加ロジック)
- [ ] `tsumugi-core/src/creative/` 手書き拡張 (feature = "creative")
- [ ] `SourceLocation` trait 定義
- [ ] `FileSourceLocation` 標準実装 (core 同梱)
- [ ] `StorageProvider` trait 定義 (core / creative 分離、LoreEntry/Character メソッドは `#[cfg]`)
- [ ] `InMemoryStorage` 実装
- [ ] 結合テスト (save / load / delete / list、feature on/off 両方)

### B. Embedding / LLM

- [ ] `EmbeddingProvider` trait 定義
- [ ] `MockEmbedding` 実装
- [ ] `LMStudioEmbedding` 実装
- [ ] `LLMProvider` trait 定義
- [ ] `OpenAICompatibleProvider` 実装 (LM Studio / Ollama 両対応)
- [ ] `MockLLMProvider` 実装

### C. 検索とスコアリング

- [ ] `Retriever` trait 定義
- [ ] lindera による BM25 実装
- [ ] cosine 類似度実装
- [ ] `HybridRetriever` 実装
- [ ] `RelevanceScorer` trait 定義
- [ ] `TemporalDecayScorer` 実装
- [ ] `ChapterOrderScorer` 実装
- [ ] `FileProximityScorer` 実装 (`SourceLocation::proximity` 利用)
- [ ] `NoDecayScorer` 実装
- [ ] `CompositeScorer` 実装

### D. イベント検知

- [ ] `EventDetector` trait 定義
- [ ] `KeywordDetector` 実装 (Tier 0)
- [ ] `EmbeddingSimilarityDetector` 実装 (Tier 1)
- [ ] `LLMClassifierDetector` 実装 (Tier 2-3)
- [ ] `CascadeDetector` 実装

### E. ★新 3 trait (Phase 1 では最小実装)

- [ ] `QueryClassifier` trait 定義
- [ ] `RegexClassifier` 実装 (Tier 0、正規表現ベース) ※ 日本語パターンは要検証項目
- [ ] `PromptCompressor` trait 定義
- [ ] `TruncateCompressor` 実装 (Tier 0、単純截断)
- [ ] `Summarizer` trait 定義
- [ ] `ExtractiveBM25Summarizer` 実装 (Tier 1)

### F. Context Compiler

- [ ] `CompiledContext` 型定義 (core / creative 分離)
- [ ] 常駐レイヤー構築
- [ ] 動的レイヤー構築 (`related_lore` は `#[cfg(feature = "creative")]`)
- [ ] Optional な `QueryClassifier` / `PromptCompressor` の組み込み
- [ ] 結合テスト: 小説シナリオ 1 本で context compile (creative feature)
- [ ] 結合テスト: コーディングシナリオ 1 本で context compile (core のみ)

### G. 結合テスト

- [ ] TRPG シナリオ (CoC ミニセッション 3 シーン) の end-to-end
- [ ] 小説シナリオ (短編 5 章分) の end-to-end
- [ ] ツクールシナリオ (MZ プロジェクトでの裁定記憶) の end-to-end
- [ ] つかさ / つづり / つくもからの依存導入確認 (`cargo tree`)
- [ ] feature flag の組み合わせ検証 (default / creative)

## Phase 2: 上位製品統合と調整 (つかさリリース後)

- [ ] つかさで実戦投入、frictions を記録
- [ ] 抽出した friction から API 改善
- [ ] つづり実装開始前に API 安定化
- [ ] `SqliteStorage` 実装 (sqlx ベース)
- [ ] sqlite-vec 統合検証
- [ ] つくも実装でのフィードバック反映
- [ ] `FileProximityScorer` の改良 (モジュール依存グラフベース)

### Phase 2 の拡張 trait 実装 (調査書 §8 の段階 2)

- [ ] `LlmLinguaCompressor` 実装 (Tier 2、LLMLingua-2)
- [ ] `SelectiveContextCompressor` 実装 (Tier 2)
- [ ] `LlmSummarizer` 実装 (Tier 3)
- [ ] `HierarchicalSummarizer` 実装 (level ごとに method 切替)
- [ ] Summarizer パイプライン全体の検証 (階層要約の更新タイミング含む)
- [ ] ユーザー編集済み要約保護の UX 検証 (`edited_by_user` / `auto_update_locked`)

## Phase 3: TypeScript SDK と拡張分類器

- [ ] `tsumugi-ts/` の実装開始
- [ ] oxidtr 生成型を活用した TS 側 API
- [ ] Tauri IPC での Rust ↔ TS 型整合性確認
- [ ] `BertClassifier` 実装 (Tier 1、MiniLM / ModernBERT ベース) ※ 調査書 §4.1 SelRoute 流
- [ ] `IkeEmbeddingProvider` 実装 (Tier 1、二値化 embedding) ※ 調査書 §4.3

## Phase 4: 拡張 (実需次第)

- [ ] chatstream との抽象共通化検討
- [ ] 追加 feature flag の設計 (`coding`, `research`, `business` 等、実需発生時のみ)
- [ ] Alloy 生成制約のさらなる追加
- [ ] パフォーマンス最適化 (embedding batch 化、検索インデックス)
- [ ] Decision-theoretic memory (調査書 §4.4 DAM) の実験

## Phase 5 (未定): 公開

- [ ] README / docs の英語版整備
- [ ] ライセンス決定
- [ ] crates.io 公開判断
- [ ] npm 公開判断 (tsumugi-ts)

---

## 未確定の大論点 (プロジェクト横断)

> tech-architecture.md の実装固有論点は同ファイル末尾に移動済み。ここにはフェーズ判断や方針系の大論点のみ集約。

### 設計方針系

- `creative` 命名の見直しタイミング (新ドメイン出現時に再検討)
- 追加 feature flag (`coding`, `research` 等) 追加の発動条件 (どの程度の実需があれば起動するか)
- LLM 自己編集メモリ (MemGPT / Letta 流) の是非 (現状見送り、今後の必要性判断)
- Event-Centric Memory (MAGMA / EverMemOS) のグラフ構造と Chunk 構造の両立可否

### フェーズ判断系

- sqlite-vec の採用タイミング (Phase 2 で性能評価後に決定)
- `Summarizer` の LLM コスト負担 (ローカル完結 / クラウド fallback の切替基準)
- chatstream との統合スコープと時期 (Phase 4 で再評価)

### 公開系

- 公開可否 (OSS / 商用) の判断
- 英語 docs の整備優先度
- npm / crates.io 公開判断

### 要実機検証項目

> 実機環境で実測して判断する項目。詳細は `runtime-environment.md` と `context-management-survey.md` §10 を参照。

- SelRoute 方式の日本語対応 (`RegexClassifier` / `BertClassifier` の日本語適性)
- LLMLingua-2 の日本語性能 (XLM-RoBERTa 実タスク品質)
- IKE 二値化 embedding の retrieval 精度
- 階層的要約の更新タイミング戦略 (差分更新 vs フル再生成)
- ユーザー編集済み要約と自動更新の競合 UX
- context clash (Microsoft/Salesforce 39% 低下) の創作系での再現
- Qwen3 Swallow の日本語品質
- MoE モデルの上位体験差 (30B-A3B 等)
- MacBook Air M1/M2 での動作閾値
- GBNF 制約下の Coder 生成精度

---

*最終更新: 2026-04-23*
