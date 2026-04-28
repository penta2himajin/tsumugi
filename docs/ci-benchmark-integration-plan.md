# CI ベンチマーク評価統合 計画書

## ステータス

- **著者**: @penta2himajin
- **初版**: 2026-04-28
- **ステータス**: Draft (実装計画)
- **目的**: `docs/evaluation-datasets.md` で整理したベンチマークを GitHub Actions CI 上で機械的に回せる仕組みに落とし込む
- **関連**: [evaluation-datasets.md](./evaluation-datasets.md), [tech-architecture.md](./tech-architecture.md), [runtime-environment.md](./runtime-environment.md), [TODO.md](./TODO.md)

---

## サマリ (TL;DR)

1. **ベンチマーク**: フルセットは現実的でないため、ライセンスがクリーンで Mayu 差別化軸 (supersession / Conflict Resolution) と直結する **ミニマルサブセット** に絞る (LongMemEval_oracle 30 問 + MemoryAgentBench Conflict_Resolution 8 問 + RULER NIAH-S 5 ケース、合計 43 ケース)。
2. **埋め込みモデル**: CI 内推論前提。**第 1 候補: `multilingual-e5-small` (118M, MIT)**。理由は (1) Apache/MIT 系で Mayu OSS と整合、(2) 英語ベンチマークの主力かつ将来の日本語自作ベンチへの拡張パスが断たれない、(3) ONNX で 4 vCPU CPU 推論が 1 文 < 50ms。**第 2 候補: `bge-small-en-v1.5` (33M, MIT)** = 速度優先・英語特化サブセット用。
3. **LLM**: CI 内推論を主軸、API フォールバックを補助軸とする。**第 1 候補: `Qwen3-4B-Instruct` Q4_K_M (~2.5GB, Apache 2.0)** を `llama-server` (OpenAI 互換) で起動し、既存 `OpenAiCompatibleProvider` をそのまま叩く。**第 2 候補: `Gemma 4 E4B` (Apache 2.0, 多言語 140+)**。43 ケース × 平均 200 出力トークン × 5–8 tok/s ≒ 15–30 分で完走する見込み。
4. **CI 構成**: 既存 `.github/workflows/ci.yml` には触れず、新規 `.github/workflows/bench.yml` を作成し `schedule (nightly UTC 18:00) + workflow_dispatch` で起動。PR ではデフォルト回さない。
5. **段階実装**: TODO.md の Phase 4 に **Phase 4-α: CI ベンチマーク統合** として追加。Step 1 (runner と pinning) → Step 2 (LongMemEval_oracle 動作確認) → Step 3 (MemoryAgentBench CR + RULER NIAH-S 統合) → Step 4 (regression alert) の 4 段。

---

## CI 環境の前提

GitHub Actions の **公開リポジトリ向け標準ランナー** (x64 ubuntu-latest) を使う。Mayu (tsumugi) リポジトリは公開を前提とする (Phase 5 で公開判断と整合)。

| 項目 | 値 | 備考 |
|---|---|---|
| ランナーラベル | `ubuntu-latest` (= `ubuntu-24.04`) | x64 |
| vCPU | **4** | 公開リポジトリの x64 標準ランナー (2024-02 アップグレード後) |
| RAM | **16 GiB** | 同上 |
| ジョブタイムアウト | 6 時間 | デフォルト上限 |
| 利用料金 | 0 | 公開リポジトリは無料 |
| ストレージ | 約 14 GB free | モデル GGUF (~3 GB) と ONNX (~120 MB) は十分収まる |

**注**: 公開リポジトリの x64 は 2024-02 のアップグレードで 4 vCPU / 16 GiB に倍増している。プライベートリポジトリ (2 vCPU / 7 GiB) と混同しないこと。`ubuntu-24.04-arm` も公開リポジトリで 4 vCPU だが、ONNX や llama.cpp の x64 最適化を活かすため当面 x64 を主軸にする。

### キャッシュ戦略

- **Hugging Face モデル**: `actions/cache` で `~/.cache/huggingface/hub` を SHA キー化してキャッシュ。初回 cold は ~3-5 分、warm は ~10 秒。
- **Cargo / target**: 既存 `Swatinem/rust-cache@v2` の流用。
- **データセット**: HF dataset の Parquet をキャッシュ (LongMemEval ~50MB、MemoryAgentBench ~10MB、RULER は合成生成なのでキャッシュ不要)。

---

## ベンチマークサブセットの選定

### 採用する 3 ベンチマーク

| ベンチマーク | サブセット | ケース数 | カバー軸 | 主用途 | License |
|---|---|---|---|---|---|
| **LongMemEval** | `longmemeval_oracle` を 30 問に絞り | 30 | IE, MR, KU, TR, ABS | 5 カテゴリ網羅、業界標準 | MIT |
| **MemoryAgentBench** | `Conflict_Resolution` 全 8 問 | 8 | **CR (supersession 直接検証)** | Mayu 差別化軸 | MIT |
| **RULER** | `niah_single_2` を seq_len ∈ {4K, 8K, 16K, 32K, 64K} で各 1 ケース | 5 | retrieval baseline | Tier 0 (BM25) baseline | Apache 2.0 |

合計 **43 ケース**。フルセット (LongMemEval 500 + MemoryAgentBench 146 + RULER 13 task) と比べて約 6%、CI で許容できる規模。

### サブセット選定基準

- **ライセンス**: 全て Apache/MIT 系。Mayu (今後 OSS 化の選択肢を残す) と整合。
- **対象言語**: 全て英語中心。CI 内推論モデルの選定もこれを前提にする。日本語自作ベンチは別ジョブ (`japanese-bench.yml`) として将来分離する。
- **機能カバー**: LongMemEval が基本 5 軸を、MemoryAgentBench が **Conflict Resolution = supersession** を、RULER が retrieval baseline をそれぞれ担う。重複が少なく合計でも軽量。
- **規模制御**: LongMemEval は `_oracle` サブセット (evidence のみ) を使うことで「retrieval 評価」と「memory 評価」を分離し、後者だけに絞れる。30 問は 6 question type × 平均 5 問の層化抽出で安定性を確保。
- **再現性**: 全データが Hugging Face / GitHub Releases から固定 commit hash で取得可能。

### 除外する判断

- **LongMemEval_S (115K tok/問)**: コンテキスト処理能力の評価には強いが、4B クラス LLM × 4 vCPU CPU では 1 問 5-10 分かかり 30 問完走で 2.5-5h。`_oracle` で代替。
- **MemoryAgentBench の他 split (Accurate_Retrieval / TTL / LRU)**: 110 + 22 + 6 問あり時間がかかる。CR が Mayu 最重要差別化軸なのでまず CR のみ。LRU は将来 weekly job に分離検討。
- **HotpotQA / NarrativeQA / MultiHop-RAG**: nightly スコープ外。weekly job (`bench-extended.yml`) で別途検討、本計画書では対象外。

---

## 埋め込みモデルの選定

### 選定基準

1. **ライセンス互換**: Apache 2.0 / MIT を必須。Gemma Terms 系や CC BY-NC 系は除外。
2. **CI 上での実行コスト**: 4 vCPU CPU 推論で 1 文 < 100ms、初回ロード < 30 秒。
3. **対象言語性能**: 採用ベンチマークが英語中心 → 英語精度を主軸、多言語対応は副次。
4. **Rust エコシステムとの親和性**: ONNX Runtime (ort crate) または GGUF (llama.cpp) で動くことを優先。`fastembed-rs` や `ort` から呼べるかを基準とする。

### 候補比較

| モデル | パラメータ | 次元 | License | 多言語 | MTEB (en) | 備考 |
|---|---|---|---|---|---|---|
| **multilingual-e5-small** | 118M | 384 | **MIT** | 100+ | 良好 | **第 1 候補**: 英語と日本語の両立、ONNX 配布あり |
| **bge-small-en-v1.5** | 33M | 384 | **MIT** | 英語のみ | 強い | **第 2 候補**: 速度最優先、英語サブセット用 |
| **Qwen3-Embedding-0.6B** | 600M | 1024 | **Apache 2.0** | 100+ | 強い | 大きすぎる、メモリ 1.2 GB |
| EmbeddingGemma-300M | 308M | 768 (MRL 128–768) | Gemma Terms | 100+ | SOTA <500M | ライセンスが Apache でない、Apache 2.0 OSS 化を狙う Mayu とは噛み合わない |
| ruri-v3-30m | 37M | 256 | Apache 2.0 | 日本語特化 | (英語は弱い) | 日本語自作ベンチで採用候補、英語 CI には不向き |
| jina-embeddings-v3 | 570M | 1024 | **CC BY-NC 4.0** | 100+ | 強い | **商用 NG で除外** |

### 選択

- **既定**: `intfloat/multilingual-e5-small` (HuggingFace、MIT)。
- **理由**: (1) MIT ライセンスで OSS 化の自由度を保つ、(2) 採用ベンチが英語中心の現状でも実用精度、(3) 将来の日本語自作ベンチで再利用可能 (JMTEB で Ruri-v3 系よりやや低いが破綻しない)、(4) ONNX 配布あり (`Xenova/multilingual-e5-small` で品質確認済み)、(5) 4 vCPU CPU で 1 文 ~30ms を見込める。
- **副候補**: `BAAI/bge-small-en-v1.5`。LongMemEval_oracle のような短文中心のジョブは bge-small-en の方が 3-5x 速い。コスト見積が増えた場合の最初の差し替え対象とする。
- **棄却**: EmbeddingGemma 300M は MTEB SOTA だが Gemma Terms (Apache 2.0 ではない) のため、Apache 2.0 化候補の Mayu リポジトリ内 CI で常用するのは避ける。Gemma 4 LLM の Apache 2.0 化と異なり、EmbeddingGemma は 2025-09 リリース時の Gemma Terms のまま。
- **棄却**: ruri-v3-30m は Apache 2.0 だが日本語特化 (英語 MTEB スコアは <60)、英語ベンチマークでは精度が出ない。日本語自作ベンチ用のサブ計画書 (将来) で採用予定。

### Rust 側統合方針

CI ジョブからは `tsumugi-core` の `EmbeddingProvider` 経由で叩く。具体的には:

- **既定**: `LmStudioEmbedding` + LM Studio をジョブ内で起動 → 工数大、却下。
- **代替案 A**: `OnnxEmbedding` provider (新規) を `tsumugi-core` に追加し、`ort` crate で ONNX 直叩き。**この案を採用**。`network` feature 同様に `onnx` feature を追加する。
- **代替案 B**: `llama-server --embedding` モードで GGUF 埋め込みを叩く。OpenAI 互換 API で済む反面、モデル選択肢が少ない (ruri / bge / e5 すべて GGUF 未配布の場合あり)。

`OnnxEmbedding` の追加は本計画書のスコープに含むが、トレイト面のみ Phase 4-α Step 1 で先行追加し、実装は CI 統合と並行で進める。

---

## LLM の選定

### 選定基準

1. **ライセンス**: Apache 2.0 を最優先。Gemma 4 (Apache 2.0 化済み) は許容、Gemma 3 以前 (Gemma Terms) は CI 用には避ける。
2. **CI 実行時間**: 4 vCPU CPU で生成 5-10 tok/s が現実的下限。43 問 × 平均 200 tok 出力 ≒ 8,600 tok。10 tok/s なら 14 分、5 tok/s なら 30 分。**目標: 30 分以内**。
3. **指示追従性**: ベンチマーク回答は「Final answer: X」形式の structured output が要る。GBNF / JSON Mode 対応があると安定する。
4. **コンテキスト長**: LongMemEval_oracle が evidence sessions のみなので 8K-16K あれば足りる。RULER NIAH-S も最大 64K。

### 候補比較

| モデル | パラメータ | License | コンテキスト | Q4_K_M サイズ | 多言語 | 備考 |
|---|---|---|---|---|---|---|
| **Qwen3-4B-Instruct** | 4B | **Apache 2.0** | 32K | ~2.5 GB | 119 | **第 1 候補**: 思考モード対応、CPU 実用域 |
| **Gemma 4 E4B-it** | ~4B effective | **Apache 2.0** (2026-04) | 128K | ~3 GB (Q4_K_M) | 140+ | **第 2 候補**: 長文脈と多言語に強い |
| Qwen3-1.7B-Instruct | 1.7B | **Apache 2.0** | 32K | ~1.1 GB | 119 | **下振れ用**: 4B が間に合わない場合 |
| Gemma 4 E2B-it | ~2B effective | **Apache 2.0** | 128K | ~1.5 GB | 140+ | E4B が間に合わない場合 |
| Qwen3.5-2B / 4B / 9B | 2B / 4B / 9B | (要確認、Apache 2.0 系の見込み) | 256K | (未測定) | 200+ | 2026-03-02 リリースで新しい。ライセンス文面を直接確認できるまで主候補にしない |
| Phi-4-Mini | 3.8B | MIT | 128K | ~2.5 GB | 英語強 | 多言語が弱め、副次候補 |
| Qwen2.5-3B-Instruct | 3B | Apache 2.0 (Research?) | 32K | ~1.9 GB | 多言語 | Qwen2.5 small は Qwen Research License の場合あり、要確認 |

### 選択

- **既定**: `Qwen3-4B-Instruct` Q4_K_M (Unsloth Dynamic 推奨)。理由は Apache 2.0、4B クラスで指示追従が安定、思考モード on/off で CI コスト調整可能、`network` feature 既存配線がそのまま使える。
- **副候補**: `Gemma 4 E4B-it` Q4_K_M。Apache 2.0 化済み、多言語に強く、将来の日本語自作ベンチへ流用可能。E4B の "effective" 表記は QAT 前提。
- **下振れ用**: `Qwen3-1.7B-Instruct`。CPU 推論が遅すぎた場合の差し替え。指示追従が落ちることは織り込み済み。
- **保留**: Qwen3.5 small (0.8B/2B/4B/9B) は 2026-03-02 リリースで魅力的だが、本書執筆時点でモデルカードのライセンス文面を直接確認できていない。Qwen3.5 27B 以下のモデルがすべて Apache 2.0 とアナウンスされた後に主候補へ昇格する余地あり。Qwen3.6 (27B 以上) は CI 対象外。
- **棄却**: Qwen2.5-3B は Qwen2.5 系列で Apache 2.0 と Qwen Research License が混在しており検証コスト過多。Qwen3 系で十分。

### llama.cpp サーバーモード経由

- ジョブ内で `apt install` 経由ではなく、`ggml-org/llama.cpp` の release バイナリを GitHub Releases からダウンロード (バージョン pin 必須、例: `b4500`)。
- 起動: `llama-server -hf Qwen/Qwen3-4B-Instruct-GGUF:Q4_K_M --port 8080 --ctx-size 16384 --threads 4`
- 既存 `OpenAiCompatibleProvider` から `http://localhost:8080/v1` を叩く。
- グラマー制約: `--grammar-file` で JSON 強制、回答抽出を安定化。

### API フォールバック (補助軸)

- CI 内推論で品質が安定しない / 時間切れの場合、**手動 dispatch + secret 設定時のみ** OpenAI / Anthropic API へフォールバックする経路を残す。
- 既定 (nightly schedule) では API は呼ばない。コスト爆発と external dependency を避けるため。
- 該当ジョブは `if: github.event_name == 'workflow_dispatch' && secrets.OPENAI_API_KEY != ''` で gate。

---

## 評価アーキテクチャ

### ディレクトリ構成

```
benches/
├── runner/                     # Rust binary crate (新規)
│   ├── Cargo.toml
│   ├── src/
│   │   ├── main.rs             # CLI: --suite longmemeval-oracle ...
│   │   ├── adapters/
│   │   │   ├── longmemeval.rs
│   │   │   ├── memoryagentbench.rs
│   │   │   └── ruler.rs
│   │   ├── metrics.rs
│   │   └── report.rs           # JSON Lines 出力
│   └── tests/
├── data/                       # .gitignore (download script 経由で取得)
├── results/                    # CI artifact のみ、commit しない
└── scripts/
    ├── download_datasets.sh
    ├── download_models.sh
    └── start_llama_server.sh
```

- **runner は Rust binary crate** とする。理由: (1) `tsumugi-core` の trait をそのまま叩ける、(2) Cargo workspace 内で型安全、(3) Phase 1 の `Bm25Retriever` / `CosineRetriever` / `HybridRetriever` を直接組み合わせて Tier 別 ablation を切れる。
- TypeScript 側 (`tsumugi-ts`) は CI ベンチ対象外 (Tauri IPC 経由の上位製品で別途評価)。
- `benches/data/` と `benches/results/` は `.gitignore`。データはライセンス継承 (CC BY-SA 4.0 等) を回避するため絶対にコミットしない。

### Tier 別 ablation の分離

各ベンチで **同一のテストケースを Tier 構成違いで複数回回す** ことを必須仕様とする。これは Mayu の価格設計 (monetization-strategy.md の Tier 別ablation の根拠) に直結する。

| ablation | 構成 | 用途 |
|---|---|---|
| `tier-0` | BM25 のみ (`Bm25Retriever` + `NoDecayScorer`) | LLM 不使用 baseline |
| `tier-0-1` | BM25 + cosine semantic (`HybridRetriever`) | semantic 効果の単独測定 |
| `tier-0-1-2` | + `LlmLinguaCompressor` (Tier 2) | 圧縮の効果測定 |
| `full` | + `LlmSummarizer` (Tier 3) | 全 Tier 投入 |

各 ablation でメトリクス (accuracy / F1 / latency / token cost) を記録し、`results/<run_id>/<bench>/<ablation>.jsonl` に出力する。

### メトリクス

- LongMemEval: 公式 metric (LLM-as-Judge with GPT-4o → CI では judge LLM を Qwen3-4B にダウンサイズ + 規則ベース併用)
- MemoryAgentBench: 公式 exact match / fuzzy match
- RULER: 公式 string match (NIAH 系は完全一致)

LLM-as-Judge は判定モデルの差で揺れるため、**規則ベース primary + LLM judge secondary** とし、不一致時は両方記録する。後で paper-exact 再現が必要になったら API judge に差し替える。

---

## ワークフロー設計

### `bench.yml` (新規) のスケルトン

```yaml
name: Bench (nightly)

on:
  schedule:
    - cron: '0 18 * * *'   # UTC 18:00 = JST 03:00
  workflow_dispatch:
    inputs:
      suite:
        description: 'Benchmark suite'
        required: true
        type: choice
        options:
          - smoke         # NIAH-S 5 問のみ、~5 分
          - oracle        # LongMemEval_oracle 30 問
          - cr            # MemoryAgentBench CR 8 問
          - all           # 上記すべて、~30-40 分
        default: all
      llm_backend:
        description: 'LLM backend'
        required: true
        type: choice
        options:
          - llama-cpp     # 既定、CI 内
          - openai        # API フォールバック (要 secret)
        default: llama-cpp

jobs:
  bench:
    name: 'bench (${{ inputs.suite || ''all'' }})'
    runs-on: ubuntu-latest
    timeout-minutes: 60
    permissions:
      contents: read
    steps:
      - uses: actions/checkout@v4

      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2

      - name: Cache Hugging Face hub
        uses: actions/cache@v4
        with:
          path: ~/.cache/huggingface/hub
          key: hf-${{ runner.os }}-bench-v1-${{ hashFiles('benches/scripts/download_models.sh') }}

      - name: Install llama.cpp release
        run: ./benches/scripts/install_llama_cpp.sh  # version pin

      - name: Download datasets
        run: ./benches/scripts/download_datasets.sh

      - name: Download models (LLM + embedding)
        run: ./benches/scripts/download_models.sh

      - name: Build runner
        run: cargo build --release --bin tsumugi-bench --features "network,onnx"

      - name: Start llama-server
        run: ./benches/scripts/start_llama_server.sh &

      - name: Wait for llama-server health
        run: ./benches/scripts/wait_for_health.sh http://localhost:8080/health

      - name: Run benchmarks
        env:
          OPENAI_API_BASE: http://localhost:8080/v1
          OPENAI_API_KEY: dummy
        run: |
          cargo run --release --bin tsumugi-bench --features "network,onnx" -- \
            --suite ${{ inputs.suite || 'all' }} \
            --output benches/results/${{ github.run_id }}

      - name: Upload results
        if: always()
        uses: actions/upload-artifact@v4
        with:
          name: bench-results-${{ github.run_id }}
          path: benches/results/

      - name: Compare with baseline
        if: github.event_name == 'schedule'
        run: ./benches/scripts/compare_baseline.sh
```

### Regression alert

- ベースラインスコアを `benches/baseline.json` にコミット (commit のたびに更新せず、明示更新だけ)。
- `compare_baseline.sh` でメトリクス低下 > 5% を検出したら `gh issue create` で Issue を起票。
- 当面は警告のみ、`exit 1` で fail させるのは安定するまで保留。

---

## コスト見積

CPU 推論前提の概算 (4 vCPU、Qwen3-4B Q4_K_M、~7 tok/s)。

| 工程 | 時間 |
|---|---|
| llama.cpp バイナリ取得 + モデル DL (cold) | 5 分 |
| モデルウォームアップ | 30 秒 |
| LongMemEval_oracle 30 問 (out 200 tok 平均) | ~14 分 |
| MemoryAgentBench CR 8 問 (out 150 tok) | ~3 分 |
| RULER NIAH-S 5 ケース (out 50 tok) | ~1 分 |
| Tier ablation (Tier 0 / 0-1 / 0-1-2 / full の 4 通り、LLM が走るのは full のみ) | x1.3 (Tier 0/0-1/0-1-2 は LLM 軽量) |
| **合計 (warm cache)** | **~25-30 分** |
| 合計 (cold cache) | ~35-40 分 |

警戒域は 50 分。それを超え始めたら Qwen3-1.7B / Gemma 4 E2B へダウンサイズ。

---

## ライセンス整合チェック

| アセット | License | Mayu 内利用形態 | 整合性 |
|---|---|---|---|
| LongMemEval | MIT | CI 実行時 download、評価結果のみ保持 | ✅ |
| MemoryAgentBench | MIT | 同上 | ✅ |
| RULER | Apache 2.0 | 合成生成スクリプトのみ呼び出し、データ非配布 | ✅ |
| Qwen3-4B | Apache 2.0 | 重みは HF Hub から download、Mayu に同梱しない | ✅ |
| Gemma 4 E4B | Apache 2.0 (2026-03) | 同上 | ✅ |
| multilingual-e5-small | MIT | ONNX 重みを HF から download | ✅ |
| bge-small-en-v1.5 | MIT | 同上 | ✅ |
| llama.cpp (バイナリ) | MIT | release バイナリを download | ✅ |
| ort crate | MIT/Apache 2.0 dual | dependency | ✅ |

`THIRD_PARTY_LICENSES.md` を作成し、ベンチマークデータと推論モデルの双方の attribution を集約する (evaluation-datasets.md でも提案済みの方針)。

---

## 段階的実装計画

### Phase 4-α (本計画書のスコープ)

#### Step 1: Runner skeleton と pinning (1 週間)

- [ ] `benches/runner/` Cargo binary crate 作成、`tsumugi-core` 依存追加
- [ ] `OnnxEmbedding` trait 実装 (`tsumugi-core` `onnx` feature 追加)、ort crate 統合
- [ ] `benches/scripts/install_llama_cpp.sh` (バイナリ release pin)
- [ ] `benches/scripts/download_datasets.sh` / `download_models.sh` (HF revision pin)
- [ ] `benches/scripts/start_llama_server.sh` / `wait_for_health.sh`
- [ ] `THIRD_PARTY_LICENSES.md` 雛形

#### Step 2: LongMemEval_oracle 動作確認 (1-2 週間)

- [ ] LongMemEval HF dataset の Rust 側ローダー (`benches/runner/src/adapters/longmemeval.rs`)
- [ ] 30 問の層化抽出ロジック (6 question type × 5 問、seed 固定)
- [ ] 規則ベース primary metric (substring match)
- [ ] LLM judge secondary metric (Qwen3-4B 使用、簡易 prompt)
- [ ] ローカルでの動作確認 (CI 投入前)

#### Step 3: MemoryAgentBench CR + RULER NIAH-S 統合 (1 週間)

- [ ] MemoryAgentBench Conflict_Resolution 8 問の adapter
- [ ] RULER NIAH-S の合成生成スクリプト統合 (5 seq_len)
- [ ] Tier ablation matrix の実装 (4 構成)
- [ ] `bench.yml` workflow を追加し、`workflow_dispatch` のみで初回起動

#### Step 4: nightly スケジュールと regression alert (1 週間)

- [ ] `benches/baseline.json` を初回 run の結果で生成
- [ ] `compare_baseline.sh` (>5% 低下で警告)
- [ ] `schedule` cron を有効化 (UTC 18:00)
- [ ] 1 週間 nightly 観測、不安定であれば調整

### Phase 4-β (本計画書のスコープ外、後続検討)

- Weekly job (`bench-extended.yml`) で NarrativeQA / MultiHop-RAG / HotpotQA / MemoryAgentBench 残り 3 split を回す
- Japanese 自作ベンチ (`japanese-bench.yml`) を ruri-v3-30m + Qwen3 Swallow 8B で構築
- API judge fallback (OpenAI / Anthropic) のオプション化
- 結果ダッシュボード (Cloudflare Pages 等) の構築

---

## リスクと対策

### 1. CI 実行時間の上振れ

- **症状**: nightly が 50 分超で常態化
- **対策 (上から順に試す)**:
  1. Qwen3-4B → Qwen3-1.7B にダウンサイズ
  2. ablation を full のみに削減
  3. LongMemEval_oracle 30 問 → 18 問に削減 (6 type × 3 問)
  4. 最終手段: nightly でなく weekly 化

### 2. CPU 推論の品質劣化

- **症状**: 4B Q4_K_M で指示追従が落ち、回答抽出が不安定
- **対策**: GBNF で回答フォーマット強制、`--grammar-file final_answer.gbnf` 投入。それでも不安定なら `tier-0` (BM25 のみ、LLM 不使用) ablation のみ nightly に残し、LLM 評価は workflow_dispatch にする。

### 3. データセット可用性

- **症状**: HF からの download が rate limit / モデル削除
- **対策**: HF revision SHA を pin。actions/cache でモデル / データを CI 内に滞留させる。年 1 回 manual で revision 更新。

### 4. ライセンス変更

- **症状**: モデル / データセットのライセンスが事後に変更
- **対策**: `THIRD_PARTY_LICENSES.md` で revision SHA + license snapshot を記録。ライセンスが商用 NG / 改変禁止に変更された場合は revision を fix し続け、新 revision には移行しない。

### 5. LLM judge の判定揺れ

- **症状**: 同じ回答でも LLM judge が日によって異なる判定
- **対策**: 規則ベース primary。LLM judge は secondary 記録のみで alert に使わない。`temperature=0` 強制、`seed` 固定、`top_p=1`。

### 6. HotpotQA 等の CC BY-SA 継承リスク

- **症状**: CI artifact (results/ ディレクトリ) に raw data が混入し、再配布扱いになる懸念
- **対策**: 本計画書の nightly では HotpotQA を含まない。Phase 4-β で含める場合、artifact から raw text を除外し metric / score のみ保存するスクリプトを介す。

### 7. 公開前の評価結果リーク

- **対策**: artifacts は private retention (90 日)、Issue / PR comment への自動投稿はオフ。公開判断 (Phase 5) まで内部利用に限定。

---

## 既存 ci.yml との関係

既存 `.github/workflows/ci.yml` (test matrix + tsumugi-ts + drift-check) には**一切手を入れない**。`bench.yml` はファイル単位で完全独立。

- 既存 CI: PR / push 時に毎回走る、軽量 (test + clippy + fmt)
- 新規 bench.yml: nightly + manual のみ、重量 (LLM 推論あり)

---

## 関連ドキュメント

- [evaluation-datasets.md](./evaluation-datasets.md): ベンチマーク選定とライセンス調査の根拠
- [tech-architecture.md](./tech-architecture.md): Tier 階層と trait 構造
- [runtime-environment.md](./runtime-environment.md): エンドユーザー向けランタイム (CI と区別)
- [TODO.md](./TODO.md): 全体フェーズ管理 (本計画書は Phase 4-α として追加)

---

## 参考文献 / 一次情報

- GitHub Blog (2024-02): "GitHub-hosted runners: Double the power for open source" — 公開リポジトリの x64 ubuntu-latest が 4 vCPU / 16 GiB に倍増
- GitHub Docs: "GitHub-hosted runners reference" — 標準ランナー仕様
- Google Open Source Blog (2026-03): "Gemma 4: Expanding the Gemmaverse with Apache 2.0" — Gemma 4 の Apache 2.0 化
- Qwen3 Blog (2025-04): Apache 2.0 ライセンスでの 0.6B-32B + MoE 公開
- Hugging Face: `intfloat/multilingual-e5-small` (MIT), `BAAI/bge-small-en-v1.5` (MIT)
- LongMemEval (Wu et al., ICLR 2025), MemoryAgentBench (Hu et al., ICLR 2026), RULER (Hsieh et al., 2024)

---

*最終更新: 2026-04-28*
