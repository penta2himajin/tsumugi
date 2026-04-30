# つむぎ — TODO

進行中フェーズと未確定論点を集約する。完了済みフェーズの詳細履歴は git log を参照。

## 完了済みフェーズ (要約)

| フェーズ | 内容 | 完了日 |
|---|---|---|
| Phase 0 | 設計固め (concept / tech-architecture / 調査書統合 / Alloy multi-file モデル / ワークスペース skeleton) | 2026-04-23 |
| Phase 1 | コア実装 (9 trait + 各種実装 + InMemoryStorage + Mock providers + Context Compiler + 結合テスト) | 2026-04-23 |
| Phase 2 | 技術タスク (`SqliteStorage` / HTTP-backed providers / 日本語 tokenizer / 拡張 trait 群: `LlmSummarizer` / `HierarchicalSummarizer` / `LlmLinguaCompressor` / `SelectiveContextCompressor`) | 2026-04-23 |
| Phase 3 | TypeScript SDK (`tsumugi-ts/`、subpath export `tsumugi` / `tsumugi/tauri` / `tsumugi/gen`、Tauri IPC ヘルパー、20 vitest tests) + 拡張 trait (`BertClassifier` LLM-delegation 近似、`IkeEmbeddingProvider`) + CI / 再生成 | 2026-04-23 |
| 公開準備 | Apache-2.0 LICENSE + 関連製品依存の削除 + `creative` feature 廃止による汎用メモリレイヤー化 | 2026-04-29 |

未着手の Phase 1-3 持ち越し:

- [ ] oxidtr scaffolding の再評価 (helpers の transitive closure walker、fixtures 等を選択的に wire するか)
- [ ] BertClassifier の paper-exact 実装 (candle / ort 統合 + MiniLM / ModernBERT 重み配布)
- [ ] IkeEmbedding の `u64` bit packing 最適化 (retrieval hot path でメモリと SIMD を活用)
- [ ] Tauri プラグイン crate の追加 (現状はダウンストリームで `#[tauri::command]` を手動定義する前提)

---

## Phase 4-α: CI ベンチマーク評価統合 — **着手中 (2026-04-28)**

詳細計画は [`ci-benchmark-integration-plan.md`](./ci-benchmark-integration-plan.md)。
ベンチマーク 43 ケース (LongMemEval_oracle 30 + MemoryAgentBench CR 8 + RULER NIAH-S 5)
を nightly CI (新規 `bench.yml`) で回す。既存 `ci.yml` には触らない。

### Step 1: Runner skeleton + 主候補 smoke test — **完了**

- [x] `tsumugi-core` `onnx` feature 追加 + `OnnxEmbedding` trait 面 (実装は ort 統合と並行)
- [x] `benches/runner/` Cargo binary crate 作成 (workspace member、`tsumugi-core` 依存)
- [x] `benches/scripts/` 一式 (`install_llama_cpp` / `download_datasets` / `download_models` / `start_llama_server` / `wait_for_health`)
- [x] `THIRD_PARTY_LICENSES.md` 整備
- [x] `Suite::Health` (LLM 起動健全性 + 生成速度 + 簡易指示追従) + wiremock 単体テスト
- [x] `.github/workflows/bench.yml` (workflow_dispatch のみ、schedule 未設定)
- [x] **主候補 smoke test 実施**: Qwen3.5-4B (unsloth GGUF) で実機 smoke 成功 (`/health` ok まで 12 秒)
  - Gemma 4 E4B-it 並列評価は smoke 安定後に別 PR で再導入
  - 結果記録 (`benches/smoke-test-result.md` への正式記録) は Step 2 と並行

### Step 2: LongMemEval_oracle 動作確認 — **着手中**

- [x] LongMemEval HF dataset の Rust 側ローダー (`benches/runner/src/adapters/longmemeval.rs`、`xiaowu0162/longmemeval` datasets API から `longmemeval_oracle` を取得して JSON parse)
- [x] 30 問の層化抽出ロジック (6 question type × 5 問、seed 固定 FNV-1a)
- [x] 規則ベース primary metric (substring match)
- [x] `download_datasets.sh` を LongMemEval 取得用に実装
- [x] `bench.yml` に `oracle` suite option + dataset download step を追加
- [x] 単体テスト: stratified sample × 3、prompt builder × 1、wiremock 駆動 network test × 3
- [ ] **実機 smoke 実施**: `gh workflow run bench.yml -f suite=oracle` で完走確認
- [ ] LLM judge secondary metric (paper-exact 再現要時に別 PR)

### Step 3: MemoryAgentBench CR + RULER NIAH-S 統合 — **着手中**

- [x] **MemoryAgentBench Conflict_Resolution adapter** (PR ②、2026-04-30):
      `Suite::Cr` に配線、8 行 × `questions[0]` = 8 ケースを評価。各 row
      の context (273k-3.17M chars) は `Bm25Retriever` (chunk_size 1024 tok
      / top_k 10) で ~10K tok に圧縮、BM25 hit が top_k/2 未満になった
      場合は context 末尾 ~10K tok でフォールバック。`answers[i]: List[String]`
      の同義語候補は新設 `substring_match_any` でマッチ。`download_datasets.sh`
      で parquet → JSONL 変換 (pyarrow)。`CR_QUESTIONS_PER_ROW` / `CR_TOP_K`
      / `CR_CHUNK_CHARS` env で挙動 override 可
- [x] **RULER NIAH-S 合成生成 + adapter** (PR ①、2026-04-29):
      `Suite::Smoke` に配線、合成 haystack + needle 検出、`RULER_SEQ_LENGTHS`
      env で seq_len 群を上書き可能。CPU + 16K ctx 制約で default は
      {2K, 4K, 8K, 12K} の 4 ケース (公式 5 ケースの {4K, 8K, 16K, 32K, 64K}
      は GPU 環境 / 大 ctx 用)
- [x] **Tier ablation matrix** (PR ③、2026-04-30):
      `tier-0` / `tier-0-1` / `tier-0-1-2` / `full` の 4 構成。LLM 不使用
      ablation (`tier-0/0-1/0-1-2`) は retrieval recall (`substring_match[_any]`
      on retrieved chunks concat) で判定、`full` は既存 LLM answer match。
      `tier-0` は `Bm25Retriever`、`tier-0-1` は `HybridRetriever` +
      `MockEmbedding` (FNV-1a 64-dim、ONNX 実装まで暫定)、`tier-0-1-2` は
      `TruncateCompressor` (LLM 委譲版 `LlmLinguaCompressor` は ablation の
      LLM 不使用軸を破壊するため不採用)。各 adapter に
      `run_*_with_ablation(opts, ablation, dataset_path)` を追加し、
      `Suite::run` が `opts.ablations` を loop。`CaseMetric` に
      `retrieval_latency_ms` / `retrieved_chunks` / `retrieval_chars` /
      `compressed_chars` の Optional フィールド追加 (後方互換)。
      共通 utility は `benches/runner/src/adapters/common.rs`
- [x] `bench.yml` で `workflow_dispatch` から各 ablation を起動可能に
      (`--ablations <csv>` flag + `BENCH_ABLATIONS` env、未指定時は 4 構成全実行)

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

---

## Phase 4-γ: LLM 委譲撤去アーキテクチャ — **計画策定 (2026-04-30)**

詳細計画は [`llm-free-stack-plan.md`](./llm-free-stack-plan.md)。
`LLMProvider` 経由で LLM に委譲している 4 コンポーネント
(`LlmSummarizer` / `LlmLinguaCompressor` / `BertClassifier` /
`LLMClassifierDetector`) を encoder-only モデルベースの実装に置換し、
全パスを LLM 委譲なしで動作させる。Phase 4-α Step 3 で観測された
「retrieval recall (87.5%) ≫ LLM answer match (12.5%)」の構造的差を
受け、LLM-free 経路を堅牢化する。

採用アーキテクチャ:

| trait | 採用モデル | パラメータ | 既存 LLM 委譲版 |
|---|---|---|---|
| `Summarizer` | DistilBART-CNN-6-6 | 230M | `LlmSummarizer` (並列維持) |
| `PromptCompressor` | LLMLingua-2-mBERT-base | 110M | `LlmLinguaCompressor` (並列維持、要リネーム) |
| `QueryClassifier` | SetFit + MiniLM-L6-v2 | 22M | `BertClassifier` (並列維持) |
| `EventDetector` | GLiNER2 (`fastino/gliner2-base-v1`) | ~200M | `LLMClassifierDetector` (並列維持) |

実装順序 (各 1 PR):

- [ ] `ort` 統合 + `OnnxEmbedding` 本実装 (`tier-0-1` の `MockEmbedding`
      置換)
- [ ] LLMLingua-2-mBERT 実装 (`PromptCompressor`)
- [ ] SetFit + MiniLM 実装 (`QueryClassifier`)
- [ ] GLiNER2 実装 (`EventDetector`)
- [ ] DistilBART 実装 (`Summarizer`)

要検討事項 (本フェーズ着手前):

- [x] 採用モデル 4 つのライセンス確認 — 全て Apache-2.0、blocker なし
      (詳細は `llm-free-stack-plan.md` § 8)
- [ ] `THIRD_PARTY_LICENSES.md` 更新 (実装着手時に各モデル impl PR と並行)
- [ ] GLiNER2 (`fastino/gliner2-base-v1`) の ONNX export + Rust 推論実機検証
- [ ] 多言語性能 (特に日本語) のリスク評価

---

## Phase 4: 拡張 (実需次第)

- [ ] 追加機能の feature flag 設計 (実需発生時のみ起動)
- [ ] Alloy 生成制約のさらなる追加
- [ ] パフォーマンス最適化 (embedding batch 化、検索インデックス)
- [ ] Decision-theoretic memory (調査書 §4.4 DAM) の実験

## Phase 5 (未定): 公開

- [x] LICENSE ファイル (Apache-2.0) 配置
- [ ] README / docs の英語版整備
- [ ] crates.io 公開判断
- [ ] npm 公開判断 (tsumugi-ts)

---

## 未確定の大論点 (プロジェクト横断)

### 設計方針系

- LLM 自己編集メモリ (MemGPT / Letta 流) の是非 (現状見送り、今後の必要性判断)
- Event-Centric Memory (MAGMA / EverMemOS) のグラフ構造と Chunk 構造の両立可否

### フェーズ判断系

- sqlite-vec の採用タイミング (Phase 2 で性能評価後に決定)
- `Summarizer` の LLM コスト負担 (ローカル完結 / クラウド fallback の切替基準)

### 公開系

- 英語 docs の整備優先度
- crates.io / npm 公開判断のタイミング

### 要実機検証項目

> 実機環境で実測して判断する項目。詳細は `runtime-environment.md` と `context-management-survey.md` §10 を参照。

- SelRoute 方式の日本語対応 (`RegexClassifier` / `BertClassifier` の日本語適性)
- LLMLingua-2 の日本語性能 (XLM-RoBERTa 実タスク品質)
- IKE 二値化 embedding の retrieval 精度
- 階層的要約の更新タイミング戦略 (差分更新 vs フル再生成)
- ユーザー編集済み要約と自動更新の競合 UX
- context clash (Microsoft/Salesforce 39% 低下) の長期プロジェクトでの再現
- Qwen3 Swallow の日本語品質
- MoE モデルの上位体験差 (30B-A3B 等)
- MacBook Air M1/M2 での動作閾値
- GBNF 制約下の Coder 生成精度

---

*最終更新: 2026-04-30*
