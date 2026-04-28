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

## Phase 2: 上位製品統合と調整 — **技術着地 (2026-04-23)**

### 技術タスク (実装完了)

- [x] `SqliteStorage` 実装 (sqlx ベース、`sqlite` feature) + CRUD 統合テスト
- [x] HTTP-backed provider wiring (`OpenAiCompatibleProvider` / `LmStudioEmbedding`、`network` feature、reqwest + wiremock テスト)
- [x] Japanese tokenizer: dict-free `JapaneseCharTokenizer` (script-run splitter + Han bi-gram)。lindera 統合は build-time 配布 dict の扱いが整った Phase 3 で再着手
- [x] `FileProximityScorer` の改良 (モジュール依存グラフベース) — 見送り理由: 実運用データ不在で最適化対象が不透明、ast-parser 基盤が必要で重量級。Phase 3 以降で上位製品の use case が出てから着手

### Phase 2 の拡張 trait 実装 (調査書 §8 の段階 2) — 完了

- [x] `LlmSummarizer` 実装 (Tier 3、`LLMProvider` 委譲)
- [x] `HierarchicalSummarizer` 実装 (level ごとに summarizer 切替 + `method_for` で実際に適用された method を報告)
- [x] `apply_summary_update` / `SummaryUpdate` / `SummaryUpdateOutcome` によるユーザー編集済み要約保護 (`edited_by_user` / `auto_update_locked` の 2 段ガード + `force_overwrite_user_edit` による明示リセット)
- [x] `LlmLinguaCompressor` 実装 (Tier 2 近似、LLM 委譲)。paper-exact の XLM-RoBERTa 分類器版は Phase 3+ で Rust ML runtime (ort / candle) と合わせて再検討
- [x] `SelectiveContextCompressor` 実装 (Tier 2 近似、sentence-level BM25 self-information)。paper-exact の decoder-LM log-prob 版は Phase 3+
- [x] Summarizer パイプライン全体の検証 (`tests/hierarchical_summary_pipeline.rs` — level-dispatch → 保護ガード → 強制リセット → lock 優先)

### 上位製品統合 (Phase 1 デリバラブル活用待ち)

- [ ] つかさで実戦投入、frictions を記録 — つかさ MVP 着手待ち
- [ ] 抽出した friction から API 改善 — 実データ必要
- [ ] つづり実装開始前に API 安定化 — つづり着手待ち
- [ ] つくも実装でのフィードバック反映 — つくも着手待ち

### 見送り / Phase 3+ へ

- [ ] sqlite-vec 統合検証 — crate ecosystem 成熟度待ち、`SqliteStorage` に後付けで追加可能な設計
- [ ] `FileProximityScorer` モジュール依存グラフ — ast-parser 基盤が必要、上位製品 (つくも) の実運用データで判断

## Phase 3: TypeScript SDK と拡張分類器 — **完了 (2026-04-23)**

### TS SDK (tsumugi-ts)

- [x] `tsumugi-ts/` 本体実装。subpath export `tsumugi` / `tsumugi/creative` / `tsumugi/tauri` / `tsumugi/gen`
- [x] oxidtr `--target ts` 生成型を `tsumugi-ts/src/gen/` に wire (models.ts + helpers.ts のみ、scaffolding は `.gitignore`)
- [x] hand-written runtime 型: ChunkId / FactId / ... newtype (branded string)、Chunk / Fact / PendingItem の runtime shape、SourceLocationValue (rich discriminated union)、SummaryMethod、FactScope / LoreScope
- [x] `creative` 拡張モジュール: Character / SceneView / StylePreset / LoreEntry + ConditionalScope (非空 newtype)
- [x] Tauri IPC ヘルパー: `createTauriClient(invoke)` でプラットフォームの `invoke` を受け取り型付き client を生成。`TSUMUGI_COMMANDS` に command 名定数を集約 (Rust 側 `#[tauri::command]` と 1:1)
- [x] vitest harness + 20 tests (domain + Tauri client mock invoke)

### 拡張 trait 実装

- [x] `BertClassifier` 実装: Phase 3 は LLM-delegation 近似版 (paper-exact の MiniLM / ModernBERT 推論は Rust ML runtime 導入後の Phase 4+)。`QueryClassifier` trait 面は同一、label set 互換なので後日差し替え可能
- [x] `IkeEmbeddingProvider` 実装 (`IkeEmbedding`): 任意の `EmbeddingProvider` をラップし ±1 に binarize。`EmbeddingVector::cosine` がそのまま Hamming-like スコアとして使える。Phase 4 で `u64` bit packing 最適化を検討

### CI / 再生成

- [x] `scripts/regen.sh` を Rust + TS 両方生成するよう拡張 (oxidtr `--target ts` を staging → models.ts/helpers.ts のみコピー、TS scaffolding は `.gitignore`)
- [x] `.github/workflows/ci.yml` に `tsumugi-ts` job 追加 (bun install + typecheck + vitest)、drift-check も TS gen/ を含む

### Phase 4+ への持ち越し

- [ ] BertClassifier の paper-exact 実装 (candle / ort 統合 + MiniLM / ModernBERT 重み配布)
- [ ] IkeEmbedding の `u64` bit packing 最適化 (retrieval hot path でメモリと SIMD を活用)
- [ ] Tauri プラグイン crate の追加 (現状は下流製品側で `#[tauri::command]` を手動定義する前提)

## Phase 4-α: CI ベンチマーク評価統合 — **着手中 (2026-04-28)**

詳細計画は [`ci-benchmark-integration-plan.md`](./ci-benchmark-integration-plan.md)。
ベンチマーク 43 ケース (LongMemEval_oracle 30 + MemoryAgentBench CR 8 + RULER NIAH-S 5)
を nightly CI (新規 `bench.yml`) で回す。既存 `ci.yml` には触らない。

### Step 1: Runner skeleton + 主候補 smoke test

- [x] `tsumugi-core` `onnx` feature 追加 + `OnnxEmbedding` trait 面 (PR #9, 2026-04-28、実装は ort 統合と並行)
- [x] `benches/runner/` Cargo binary crate 作成 (PR #9, workspace member、`tsumugi-core` 依存)
- [x] `benches/scripts/install_llama_cpp.sh` (release バイナリ取得、`LLAMA_CPP_TAG` env var で pin、デフォルト latest)
- [x] `benches/scripts/download_datasets.sh` / `download_models.sh` skeleton (PR #9 で追加、Step 2 で本格的にデータ取得を埋める)
- [x] `benches/scripts/start_llama_server.sh` / `wait_for_health.sh` (Qwen 専用に絞り込み、2026-04-28)
- [x] `THIRD_PARTY_LICENSES.md` 雛形 (PR #9, 両 LLM 候補 + e5-small + bge-small-en の attribution)
- [x] **v0 smoke 実装**: `Suite::Health` (LLM 起動健全性 + 生成速度 + 簡易指示追従の 2 probes × 3 trials)
  - `benches/runner/src/health.rs` + wiremock 駆動の単体テスト 3 本
  - `.github/workflows/bench.yml` (workflow_dispatch のみ、schedule 未設定)
- [ ] **主候補 smoke test 実施**: Qwen3.5-4B-Instruct を 4 vCPU runner で実機検証
  - 当面 Qwen のみ評価 (ユーザー方針、2026-04-28)。Gemma 4 E4B-it 並列評価は smoke 安定後に別 PR で再導入
  - 起動成功率 (3 回連続)、tok/s、簡易指示追従 (`Health` suite で計測)
  - RULER NIAH-S 4K/16K/32K と LongMemEval_oracle 5 問の指示追従評価は Step 2-3 で追加
  - 結果を `benches/smoke-test-result.md` に記録

### Step 2: LongMemEval_oracle 動作確認

- [ ] LongMemEval HF dataset の Rust 側ローダー (`benches/runner/src/adapters/longmemeval.rs`)
- [ ] 30 問の層化抽出ロジック (6 question type × 5 問、seed 固定)
- [ ] 規則ベース primary metric (substring match)
- [ ] LLM judge secondary metric (主候補 LLM、簡易 prompt)
- [ ] ローカルでの動作確認 (CI 投入前)

### Step 3: MemoryAgentBench CR + RULER NIAH-S 統合

- [ ] MemoryAgentBench Conflict_Resolution 8 問 adapter
- [ ] RULER NIAH-S 合成生成スクリプト統合 (5 seq_len)
- [ ] Tier ablation matrix (`tier-0` / `tier-0-1` / `tier-0-1-2` / `full` の 4 構成)
- [ ] `bench.yml` workflow 追加、`workflow_dispatch` のみで初回起動

### Step 4: nightly スケジュールと regression alert

- [ ] `benches/baseline.json` 初回 run の結果で生成
- [ ] `compare_baseline.sh` (>5% 低下で警告)
- [ ] `schedule` cron 有効化 (UTC 18:00)
- [ ] 1 週間 nightly 観測、不安定であれば調整

### Phase 4-β (後続検討、本フェーズのスコープ外)

- [ ] Weekly job (`bench-extended.yml`) で NarrativeQA / MultiHop-RAG / HotpotQA / MemoryAgentBench 残り 3 split
- [ ] Japanese 自作ベンチ (`japanese-bench.yml`) を ruri-v3-30m + Qwen3 Swallow 8B で構築
- [ ] API judge fallback (OpenAI / Anthropic) のオプション化
- [ ] 結果ダッシュボード (Cloudflare Pages 等) の構築

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

*最終更新: 2026-04-28*
