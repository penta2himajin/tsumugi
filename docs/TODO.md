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
- [x] **`Chunk.source_location` 実装判断: B 案 (`SourceLocationValue` sum 型) 確定** (2026-04-23) — 詳細は `tech-architecture.md` §Phase 1 型定義時に決める実装判断
- [x] **ワークスペース skeleton** (2026-04-23)
  - [x] root `Cargo.toml` (workspace、共有依存定義)
  - [x] `rust-toolchain.toml` (stable + rustfmt + clippy)
  - [x] `tsumugi-core/` の Cargo.toml + src/ 雛形 (lib.rs, domain.rs, traits.rs, creative.rs)
  - [x] `creative` feature flag 設定 (`#[cfg(feature = "creative")]` で creative モジュール gate)
  - [x] `tsumugi-cli/` の Cargo.toml + main.rs 雛形
  - [x] `tsumugi-ts/` の package.json + tsconfig + src/index.ts 雛形
  - [x] `cargo check --all-features` が通ることを確認
- [x] **Alloy モデル multi-file 初版** (2026-04-23) — oxidtr multi-file 対応を活用
  - [x] `models/tsumugi.als` (main) — `module tsumugi`, `open tsumugi/{core,creative}`, クロスモジュール不変条件
  - [x] `models/tsumugi/core.als` — Chunk, Fact, PendingItem, SourceLocationValue (File + Custom), SummaryMethod, FactScope, FactOrigin, Priority の sig
  - [x] 階層非循環、親子逆関係、階層要約不変条件、要約メソッド整合 (Decision A)、Fact supersession 非循環
  - [x] `models/tsumugi/creative.als` — Character, SceneView, StylePreset, LoreEntry, Formality, PoV, Tense, LoreScope の sig
  - [x] oxidtr generate で Rust 出力確認 (`/tmp` で確認、生成物は未コミット)
- [x] **Alloy 警告の棚卸しと対応** (2026-04-23) — 36 件 → 4 件 (false positive) まで削減
  - [x] `edited_by_user` / `auto_update_locked` を Alloy から除去し Rust 側の runtime flag として扱う判断
  - [x] `UnconstrainedCardinality` 警告に対する tautology fact 追加 (oxidtr self-host 慣例)
  - [x] `UnreferencedSig` (SceneView / StylePreset / LoreEntry / LoreScope 変種) に対する `pred useX` マーク
  - [x] `UnhandledResponsePattern` (File / Custom / GlobalScope / ChunkLocalScope) に対する `pred useX` マーク
  - [x] `UnconstrainedTransitivity` (superseded_by) に対する直接 fact 追加
  - [x] 残る `MissingInverse` (PendingItem.expected_resolution_chunk / resolved_at × Chunk.pending、計 4 件) は設計上の reference-only 関係で ownership link でないため false positive として受容、.als 内に rationale 記載
- [x] **oxidtr 生成物の tsumugi-core への配置設計** (2026-04-23)
  - [x] 生成先: `tsumugi-core/src/gen/` に確定
  - [x] lib.rs で `#[path = "gen/tsumugi"] pub(crate) mod tsumugi { ... }` により型サブツリーのみを wire (scaffolding は未使用)
  - [x] `creative` feature gate は `pub mod creative;` のモジュール全体 gate で実装
  - [x] gen/ の型サブツリーはコミット (build-without-oxidtr を優先、IDE 互換性)
  - [x] 生成 scaffolding (helpers / operations / newtypes / fixtures / tests / 最上位 mod.rs) は `.gitignore`
  - [x] `scripts/regen.sh` で再生成 (oxidtr repo パス `--` / `OXIDTR_HOME` / デフォルト `../oxidtr`)
- [x] **Alloy モデル 2 版** (2026-04-23)
  - [x] PendingItem ライフサイクル不変条件 (`happens_before` 部分順序 + `resolved_at` / `expected_resolution_chunk` が `introduced_at` 以降)
  - [x] LoreEntry.scope Conditional 非空制約 (Rust 側 `ConditionalScope` newtype で enforce、rationale を `.als` に記載)
  - [x] `oxidtr check` を CI に組み込む判断: regen して `tsumugi-core/src/gen/` の diff を検知する形式を採用 (`.github/workflows/ci.yml` §`alloy-drift-check`)
  - [ ] oxidtr scaffolding の再評価 (helpers の transitive closure walker、fixtures 等を選択的に wire するか — Phase 2 保留)

## Phase 1: コア実装 (つかさ MVP と並行) — **完了 (2026-04-23)**

### A. 型定義とストレージ

- [x] `tsumugi-core/src/domain/` 手書き拡張 (Alloy 生成 + 追加ロジック)
- [x] `tsumugi-core/src/creative/` 手書き拡張 (feature = "creative")
- [x] `SourceLocationValue` enum 定義 (core 同梱、`File` + `Custom { schema, payload }`)
- [x] `SourceLocation` trait 定義 (振る舞いの抽象、proximity 等)
- [x] `FileSourceLocation` 標準実装 (core 同梱)
- [x] `impl SourceLocation for SourceLocationValue` (variant ディスパッチ)
- [x] `StorageProvider` trait 定義 (core / creative 分離、LoreEntry/Character メソッドは `#[cfg]`)
- [x] `InMemoryStorage` 実装
- [x] 結合テスト (save / load / delete / list、feature on/off 両方)

### B. Embedding / LLM

- [x] `EmbeddingProvider` trait 定義
- [x] `MockEmbedding` 実装 (FNV-1a → L2 正規化、決定的)
- [x] `LMStudioEmbedding` **stub** (Phase 1 は trait 面のみ、HTTP 配線は Phase 2)
- [x] `LLMProvider` trait 定義 (`ModelMetadata` / `GrammarSpec` 込み)
- [x] `OpenAICompatibleProvider` **stub** (Phase 1 は trait 面のみ、HTTP 配線は Phase 2)
- [x] `MockLLMProvider` 実装 (prefix echo、決定的)

### C. 検索とスコアリング

- [x] `Retriever` trait 定義
- [x] BM25 実装 (`Bm25Retriever` + pluggable `Tokenizer` trait、`WhitespaceTokenizer` 同梱、lindera 組み込みは Phase 2)
- [x] cosine 類似度実装 (`EmbeddingVector::cosine` + `CosineRetriever`)
- [x] `HybridRetriever` 実装 (スコア正規化後の重み付き合成)
- [x] `RelevanceScorer` trait 定義
- [x] `TemporalDecayScorer` 実装
- [x] `ChapterOrderScorer` 実装
- [x] `FileProximityScorer` 実装 (`SourceLocation::proximity` 利用)
- [x] `NoDecayScorer` 実装
- [x] `CompositeScorer` 実装

### D. イベント検知

- [x] `EventDetector` trait 定義 (`type Event`)
- [x] `KeywordDetector` 実装 (Tier 0)
- [x] `EmbeddingSimilarityDetector` 実装 (Tier 1)
- [x] `LLMClassifierDetector` 実装 (Tier 2-3、MockLLMProvider で plumbing 検証)
- [x] `CascadeDetector` 実装 (short-circuit)

### E. ★新 3 trait

- [x] `QueryClassifier` trait 定義
- [x] `RegexClassifier` 実装 (Tier 0、`regex` crate)
- [x] `PromptCompressor` trait 定義 (`CompressionHint`)
- [x] `TruncateCompressor` 実装 (Tier 0)
- [x] `Summarizer` trait 定義
- [x] `ExtractiveBM25Summarizer` 実装 (Tier 1、日本語/英語両対応の sentence splitter)

### F. Context Compiler

- [x] `CompiledContext` 型定義 (core / creative 分離、`related_lore` は `#[cfg(feature = "creative")]`)
- [x] 常駐レイヤー構築 (current chunk → parent chain)
- [x] 動的レイヤー構築 (retrieve → rescore → top-k)
- [x] Optional な `QueryClassifier` / `PromptCompressor` の組み込み
- [x] 結合テスト: 小説シナリオで context compile + summarize (`tests/novel_scenario.rs`)
- [x] 結合テスト: コーディングシナリオで context compile (`tests/coding_scenario.rs`)

### G. 結合テスト

- [x] 小説シナリオ (4 章、Character + LoreEntry + ChapterOrderScorer) end-to-end
- [x] コーディングシナリオ (6 ファイル、FileProximityScorer + Hybrid 検索) end-to-end
- [x] feature flag の組み合わせ検証 (default / creative): `cargo test` / `cargo test --all-features` 両方 55 test 通過
- [ ] TRPG シナリオ (つかさ MVP と並行実装、Phase 2 で回収)
- [ ] ツクールシナリオ (つくも実装時、Phase 2 で回収)
- [ ] つかさ / つづり / つくもからの依存導入確認 (`cargo tree`) — 下流 3 製品の着手時

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
