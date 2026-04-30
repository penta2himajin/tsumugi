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
3. **LLM**: CI 内推論を主軸、API フォールバックを補助軸とする。**主候補は 2 つを並列評価し Step 1 の smoke test 結果で確定する**: `Qwen3.5-4B-Instruct` Q4_K_M (Apache 2.0、262K context、Hybrid Gated DeltaNet、長文脈・推論性能で公式数値優位) と `Gemma 4 E4B-it` Q4_K_M (Apache 2.0、128K context、標準 Transformer + sliding window 512、llama.cpp Day-0 サポートで CI 安定性優位)。両者ともマルチモーダル VL モデルだが mmproj 非ロードで text-only 動作可能。43 ケース × 平均 200 出力トークン × 5–8 tok/s ≒ 15–30 分で完走する見込み。
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
| **MemoryAgentBench** | `Conflict_Resolution` 8 行 × `questions[0]` = 8 ケース | 8 | **CR (supersession 直接検証)** | Mayu 差別化軸 | MIT |
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

| モデル | パラメータ | License | コンテキスト | Q4_K_M サイズ | アーキテクチャ | 多言語 | 備考 |
|---|---|---|---|---|---|---|---|
| **Qwen3.5-4B-Instruct** | 5B 実測 (公称 4B) | **Apache 2.0** (確認済み) | **262K** native (1M YaRN) | ~3 GB | Hybrid Gated DeltaNet + MoE + Vision Encoder | 201 | **主候補 A**: 性能ベンチ公式数値優位 (GPQA 76.2%、LongBench v2 50.0、AA-LCR 57.0)、長文脈 retrieval に構造的優位。**llama.cpp 最新ビルド必須**、DeltaNet CPU kernel 最適化途上 |
| **Gemma 4 E4B-it** | ~4B effective | **Apache 2.0** (2026-03 化) | 128K | ~3 GB (Q8_0 で ~5 GB 推奨) | 標準 Transformer + Hybrid Attention (sliding window 512 + global) | 140+ | **主候補 B**: llama.cpp Day-0 公式サポート、CPU 推論実績豊富で CI 安定性が高い。GPQA 42.5-58.6% で Qwen3.5 に劣後だが Math/Coding は強い。**Long-context retrieval (RULER NIAH-S 32K+) は sliding window 制約で性能未知数** (Google が E4B/E2B の MRCR 系スコアを公開していない) |
| Qwen3.5-0.8B | ~0.6B | **Apache 2.0** (確認済み) | 262K | ~0.5 GB | Hybrid DeltaNet + Vision | 201 | 主候補 A の下振れ用 |
| Gemma 4 E2B-it | ~2B effective | **Apache 2.0** | 128K | ~1.5 GB | 標準 Transformer + sliding window | 140+ | 主候補 B の下振れ用 |
| Qwen3.5-2B | 2B | **Apache 2.0** (確認済み) | 262K | ~1.5 GB | Hybrid DeltaNet + Vision | 201 | **公式が「プロトタイピング/研究用途」と明示**、思考ループ報告あり。CI ベンチ主軸には不適 |
| Phi-4-Mini | 3.8B | MIT | 128K | ~2.5 GB | 標準 Transformer | 英語強 | 多言語が弱め。本計画では棄却 |
| Qwen2.5-3B-Instruct | 3B | (Apache 2.0 / Qwen Research 混在) | 32K | ~1.9 GB | 標準 Transformer | 多言語 | Qwen2.5 系はライセンス境界要確認。Qwen3.5 / Gemma 4 系で十分なため棄却 |
| Qwen3-4B / Qwen3-1.7B | 4B / 1.7B | **Apache 2.0** | 32K | ~2.5 / ~1.1 GB | 標準 Transformer | 119 | **棄却**: 同 4B クラス内で Qwen3.5-4B (262K, GPQA +20pt) と Gemma 4 E4B (128K, multimodal, Day-0 サポート) に対する性能差が世代的に明確。fallback としても残す価値が低い |

### 選択

**主候補は 2 つを並列評価し、Step 1 の smoke test 結果で確定する**。両者とも Apache 2.0、~3 GB クラス Q4_K_M、Mayu の `network` feature 既存配線で叩ける点は共通。性能 vs CI 安定性のトレードオフが明確に分かれるため事前確定はせず、CI 環境での実測値で判定する。

#### 主候補 A: `Qwen3.5-4B-Instruct` Q4_K_M

- **強み**:
  - 性能ベンチが公式数値で優位 (GPQA Diamond 76.2% vs E4B 42.5-58.6%、LongBench v2 50.0、AA-LCR 57.0、IFEval 89.8)
  - 262K native context (1M YaRN 拡張可) で長文脈 retrieval (RULER NIAH-S 32K-64K) に構造的優位
  - 201 言語、思考モード/非思考モード切替
- **弱み・リスク**:
  1. **Multimodal VL モデル** (Image-Text-to-Text)。テキスト専用ベンチでは mmproj をロードしないことで text-only 動作可能 (llama.cpp で `--mmproj` フラグを付けない / vLLM では `--language-model-only`)。vision encoder 分のメモリオーバーヘッドはあるが運用上は無視できる範囲。
  2. **Hybrid Gated DeltaNet + Gated Attention + sparse MoE**。GGUF 配布元のモデルカードが「Ensure you are using the absolute latest version of llama.cpp to support these new operators」と明記。**llama.cpp は最新ビルド (2026-04 以降の master 系) を pin する必要あり**、release バイナリの古いタグでは起動失敗の可能性。
  3. **CPU での DeltaNet kernel 最適化状況が未確定**。第三者ベンチ記事で「同アーキの Qwen3.6 は llama.cpp build によって速度が大きく変わる」と報告。Step 1 で実測必須。
  4. **5B params 実測** (公称 4B は active param 表記)。Q4_K_M で ~3 GB。

#### 主候補 B: `Gemma 4 E4B-it` Q4_K_M (または UD-Q4_K_XL)

- **強み**:
  - **llama.cpp Day-0 公式サポート**、ggml-org が直接 GGUF 配布、Unsloth が UD-Q4_K_XL を維持。release バイナリで安定動作、CI 運用負荷が低い
  - 標準 Transformer + Hybrid Attention (sliding window 512 + final layer global) の枯れた構造で CPU 推論実績豊富
  - 2026-03 に Apache 2.0 化済み (旧 Gemma Terms から変更)、商用利用クリア
  - Math/Coding は強い (AIME 42.5%、LiveCodeBench v6 52.0%)、native audio 対応
- **弱み・リスク**:
  1. **Long-context retrieval が構造的に弱い可能性**。E2B/E4B は sliding window 512 トークン (26B/31B は 1024)、final layer のみ global。Google が **E4B/E2B の MRCR/RULER 系スコアを公式公開していない**事実は、small variants の long-context retrieval が弱いことを示唆 (推測、実測必須)。RULER NIAH-S 32K/64K で needle 取り損ねるリスクあり。
  2. **GPQA Diamond 42.5% (instruct) / 58.6% (HF eval)** で Qwen3.5-4B (76.2%) に大きく劣後。一般推論性能では明確に下。
  3. **Q4_K_M より Q8_0 (~5 GB) が公式推奨**: small models は量子化感度が高く Unsloth/Google 公式が Q8_0 を勧めている。Q4_K_M / UD-Q4_K_XL でも実用域だが品質低下リスクは Qwen3.5-4B より大きい。CI ストレージ・メモリ (16 GiB) は余裕があるので Q8_0 採用も可。

#### Smoke test の判定基準 (Step 1 で実施)

両モデルで以下を実測し、主候補を確定:

- **起動成功率**: GitHub Actions runner で 3 回連続成功するか
- **生成速度**: 4 vCPU で同一プロンプトに対する tok/s
- **短文 retrieval 精度** (RULER NIAH-S 4K, 16K): 各 8 ケース × 2 モデルで正答率
- **長文 retrieval 精度** (RULER NIAH-S 32K): 5 ケース × 2 モデルで正答率 — **ここが決定打**
- **指示追従性**: LongMemEval_oracle 5 問で「Final answer:」抽出が安定するか

判定ロジック:
- A の RULER 32K 正答率が B より 20pt 以上高い → **A (Qwen3.5-4B) を主候補に確定**
- B の起動が安定し、A の起動が断続的に失敗する、または速度差が 2x 以上 → **B (Gemma 4 E4B) を主候補に確定**
- どちらも実用域で大差なし → **B を主候補** (CI 安定性重視、性能差は実用上無視できる範囲)
- どちらも実用に届かない → API フォールバックを検討、または Step 2 で Phi-4-Mini を再評価

確定した主候補のみで Step 2 以降を進める。負けた方は再評価候補として残し、新しい llama.cpp / モデルバージョンが出たら再ベンチする。

#### 下振れ用 (主候補が CPU で間に合わない場合)

- 主候補 A 確定時 → `Qwen3.5-0.8B` (Hybrid DeltaNet 系を維持、~0.5 GB)
- 主候補 B 確定時 → `Gemma 4 E2B-it` (標準 Transformer 系を維持、~1.5 GB)

#### 棄却

- **`Qwen3-4B` / `Qwen3-1.7B`**: 同 4B クラス内で Qwen3.5-4B / Gemma 4 E4B に対する性能差 (32K vs 262K/128K context、GPQA で 20pt 以上劣後など) が世代的に明確。fallback としても残す価値が低い。
- **`Qwen3.5-2B`**: Qwen 公式が「intended use cases are prototyping, task-specific fine-tuning, and other research or development purposes」と明示、「more prone to entering thinking loops」と記載。CI nightly の主軸には不適。
- **Qwen2.5-3B**: Qwen2.5 系列でライセンス境界が混在、Qwen3.5 / Gemma 4 系で十分。
- **Phi-4-Mini**: MIT で英語性能は強いが多言語弱め、対抗候補に劣後する場面が多い。再評価候補として保留。

### llama.cpp サーバーモード経由

- ジョブ内で `apt install` 経由ではなく、`ggml-org/llama.cpp` の release バイナリを GitHub Releases からダウンロード。**バージョン pin 必須**:
  - **Qwen3.5 (主候補 A) を使う場合**: 最新の master 系 build を pin (2026-04 以降の `b6000+` 系を想定、Step 1 の smoke test で具体的タグを確定する)。Hybrid Gated DeltaNet operator のサポートが新しいため、古い release では起動失敗の可能性が高い。
  - **Gemma 4 E4B (主候補 B) を使う場合**: ggml-org / Unsloth が Day-0 GGUF を配布しており、`b4500` 等の比較的古い安定タグでも動く。最新タグでも問題なし。両モデルを smoke test で並列評価する間は A の要件を優先 (新しめの master 系を pin)。
- 起動例 (Qwen3.5-4B、text-only モード、mmproj 非ロード):
  ```bash
  llama-server -hf Qwen/Qwen3.5-4B:Q4_K_M --port 8080 --ctx-size 16384 --threads 4
  # mmproj を渡さないことで vision encoder ロードを回避し、純テキスト推論で動かす
  ```
- 起動例 (Gemma 4 E4B、Unsloth UD-Q4_K_XL、text-only モード):
  ```bash
  llama-server -hf unsloth/gemma-4-E4B-it-GGUF:UD-Q4_K_XL --port 8080 --ctx-size 16384 --threads 4
  # mmproj を渡さないことで vision encoder ロードを回避
  # 品質重視なら Q8_0 (~5 GB) を選択、CI ストレージ・メモリは余裕あり
  ```
- 既存 `OpenAiCompatibleProvider` から `http://localhost:8080/v1` を叩く。
- グラマー制約: `--grammar-file` で JSON 強制、回答抽出を安定化。
- **CPU 推論速度の実測がリスク (主に Qwen3.5 側)**: Qwen3.5 の Hybrid DeltaNet kernel は llama.cpp の CPU パスでまだ最適化が進行中との第三者報告あり。Step 1 の smoke test で Qwen3.5-4B と Gemma 4 E4B の生成速度 (tok/s) を実測。Qwen3.5 が著しく遅い、または起動が不安定なら主候補 B (Gemma 4 E4B) を確定。

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

各ベンチで **同一のテストケースを Tier 構成違いで複数回回す** ことを必須仕様とする。

| ablation | 構成 | 用途 |
|---|---|---|
| `tier-0` | BM25 のみ (`Bm25Retriever`) | LLM 不使用 baseline |
| `tier-0-1` | BM25 + cosine semantic (`HybridRetriever`、embedding は Phase 4-α Step 3 では `MockEmbedding` (FNV-1a 64-dim、deterministic)、Step 4+ で `OnnxEmbedding` (multilingual-e5-small) に置換) | semantic 効果の単独測定 |
| `tier-0-1-2` | + `TruncateCompressor` (Tier 0-2 の非 LLM 圧縮、`CompressionHint` で whitespace token budget を指定) | budget 制約下の圧縮効果測定 |
| `full` | LLM full prompt (BM25 で context を ~10K tok に縮めて LLM に投げる) | 全 Tier 投入 |

各 ablation でメトリクス (accuracy / latency / retrieval-side metrics) を記録し、`results/<run_id>/<bench>/<ablation>.jsonl` に出力する。

#### 評価指標の決定 (Step 3 PR ③、2026-04-30)

- LLM 不使用 ablation (`tier-0` / `tier-0-1` / `tier-0-1-2`): **retrieval recall** = `substring_match[_any](concat(retrieved_chunks_top_k), expected_answers)`。retrieve した chunk 群の concatenation (tier-0-1-2 では `TruncateCompressor` 適用後) が期待回答を substring として含むかで判定する
- `full`: 既存通り **answer match** = LLM 生成出力 (`response.text` または `reasoning_content`) に対する `substring_match[_any]`

#### 設計判断と注記 (Step 3 PR ③、2026-04-30)

- **`tier-0-1` の embedding 選択**: 当初は `multilingual-e5-small` ONNX を予定していたが、`tsumugi_core::providers::OnnxEmbedding` は trait 面のみで実装が未完了 (PR #9 skeleton)。LLM ローカル server に embedding endpoint を併設する案は CI runner (7GB RAM) では LLM 4B + embed model 同時 OOM リスクがある。Phase 4-α Step 3 では `MockEmbedding` (deterministic、軽量) で代替し、Phase 4-α Step 4 以降で `OnnxEmbedding` に切替える方針。`tier-0-1` の数値は再計測対象。
- **`tier-0-1-2` の compressor 選択**: 当初予定の `LlmLinguaCompressor` は tsumugi 現実装では LLM 委譲版 (Phase 2 完了)。これを採用すると `tier-0-1-2` も LLM を呼ぶことになり「`tier-0` から `full` までの LLM 不使用 → 使用」軸が連続性を失う。Phase 4-α Step 3 では `TruncateCompressor` (head + " … " + tail tokens、LLM 不使用、Tier 0-2 範囲) を採用し、paper-exact な LLMLingua-2 ML 実装は Phase 3+ の課題として保留する。
- **`full` の Tier 3 構成**: 既存 adapter は LLM full prompt のみで `LlmSummarizer` を呼ばない。Phase 4-α Step 3 範囲では **既存 `full` の動作を保持**し、`+ LlmSummarizer` 統合は scope 外とする。

#### 実装インターフェース

- 各 adapter は `run_*_with_ablation(opts: &SuiteRunOptions, ablation: Ablation, dataset_path: &Path) -> SectionReport` を提供。`Suite::run` が `opts.ablations` を loop して 4 sections を返す
- ablation 選択: `--ablations <csv>` CLI flag > `BENCH_ABLATIONS` env > default (4 ablation 全部)
- 共通 retrieval / compression utility は `benches/runner/src/adapters/common.rs`

### メトリクス

- LongMemEval: 公式 metric (LLM-as-Judge with GPT-4o → CI では judge LLM を Qwen3-4B にダウンサイズ + 規則ベース併用)
- MemoryAgentBench: 公式 exact match / fuzzy match。CR は `answers[i]: List[String]` の同義語候補リストを持つので、`metrics::substring_match_any` (any-match) で判定する
- RULER: 公式 string match (NIAH 系は完全一致)

LLM-as-Judge は判定モデルの差で揺れるため、**規則ベース primary + LLM judge secondary** とし、不一致時は両方記録する。後で paper-exact 再現が必要になったら API judge に差し替える。

#### `CaseMetric` schema (Step 3 PR ③ 拡張)

`benches/runner/src/metrics.rs::CaseMetric` は jsonl の 1 行に対応。以下の Optional フィールドが追加された (既存 `full` jsonl との後方互換維持。`Option::is_none` のフィールドは serialize でスキップ):

- `retrieval_latency_ms: Option<u64>` — retrieval (BM25 / Hybrid) 段の wall time。LLM 不使用 ablation (`tier-0` / `tier-0-1` / `tier-0-1-2`) で記録
- `retrieved_chunks: Option<usize>` — retrieve した chunk 数 (top_k で打ち切り後)
- `retrieval_chars: Option<usize>` — retrieved chunks の concatenation の文字数 (compression ratio 計算の分母)
- `compressed_chars: Option<usize>` — `TruncateCompressor` 適用後の文字数。`tier-0-1-2` のみ Some

`metrics::compression_ratio(original, compressed) -> f64` で圧縮率を計算 (集計時に jsonl から後段で算出)。

### MemoryAgentBench CR の context truncation (Step 3 PR ② 実装決定、2026-04-30)

CR の各 row の `context` は 273k-3.17M chars (約 70K-800K tokens) と llama-server `--ctx-size 16384` を大きく超えるため、adapter 側で context 圧縮が必要。実装決定:

- **戦略**: `tsumugi_core::retriever::Bm25Retriever` で chunk_size 1024 tok (≒ 4096 chars) / top_k 10 の retrieval を実施し ~10K tok 程度に圧縮
- **フォールバック**: BM25 hit が `top_k/2` 未満の場合、context 末尾 ~10K tokens を採用 (CR の supersession 仮説と整合: 新しい事実は document 末尾近辺に集中する傾向)
- **chars↔tokens 換算**: 保守的に 4 chars/token を仮定 (英語 ASCII で 3.5-4.5 chars/tok の安全側)
- **Tier 0 ablation との関係**: adapter 内の BM25 は **prompt budget 制約から来る前処理**であり、Tier 0 (LLM 不使用 baseline) ablation とは別概念。Tier 0 は Step 3 PR ③ (Tier ablation matrix) で別出力ファイルとして追加する
- **「全 8 問」の解釈**: CR split は 8 行 × 60-100 QA/行という構造。本フェーズでは **8 行 × `questions[0]` = 8 ケース**を評価対象とする (最初の代表 QA を deterministic に採用)。`CR_QUESTIONS_PER_ROW` env で 1..N に拡張可能

### parquet 取扱い

`download_datasets.sh` は LongMemEval (JSON) は raw 取得、MemoryAgentBench (parquet) は `pyarrow` 経由で JSONL 変換した結果を `benches/data/memoryagentbench_cr.jsonl` に置く。Rust adapter は両者とも JSONL/JSON 経路で読む (`serde_json`)。

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
| Qwen3.5-4B (主候補 A) | Apache 2.0 (HF model card 確認、2026-04-28) | 重みは HF Hub から download、Mayu に同梱しない | ✅ |
| Gemma 4 E4B (主候補 B) | Apache 2.0 (2026-03 化、HF blog / 公式 model card 確認) | 同上 | ✅ |
| multilingual-e5-small | MIT | ONNX 重みを HF から download | ✅ |
| bge-small-en-v1.5 | MIT | 同上 | ✅ |
| llama.cpp (バイナリ) | MIT | release バイナリを download | ✅ |
| ort crate | MIT/Apache 2.0 dual | dependency | ✅ |

`THIRD_PARTY_LICENSES.md` を作成し、ベンチマークデータと推論モデルの双方の attribution を集約する (evaluation-datasets.md でも提案済みの方針)。

---

## 段階的実装計画

### Phase 4-α (本計画書のスコープ)

#### Step 1: Runner skeleton + 主候補 smoke test (1-2 週間)

- [ ] `benches/runner/` Cargo binary crate 作成、`tsumugi-core` 依存追加
- [ ] `OnnxEmbedding` trait 実装 (`tsumugi-core` `onnx` feature 追加)、ort crate 統合
- [ ] `benches/scripts/install_llama_cpp.sh` (バイナリ release pin、Qwen3.5 が動く master 系を選択)
- [ ] `benches/scripts/download_datasets.sh` / `download_models.sh` (HF revision pin、Qwen3.5-4B + Gemma 4 E4B 両方)
- [ ] `benches/scripts/start_llama_server.sh` / `wait_for_health.sh` (モデル切替対応)
- [ ] **主候補 smoke test**: Qwen3.5-4B と Gemma 4 E4B を 4 vCPU GitHub Actions runner で並列評価
  - 起動成功率 (3 回連続)、tok/s、RULER NIAH-S 4K/16K/32K 正答率、LongMemEval_oracle 5 問の指示追従
  - 結果を `benches/smoke-test-result.md` に記録 (commit して PR レビュー対象に)
  - 上記「選択」セクションの判定ロジックに従い主候補を確定
- [ ] `THIRD_PARTY_LICENSES.md` 雛形 (Qwen3.5-4B + Gemma 4 E4B 両方の attribution を含む)

#### Step 2: LongMemEval_oracle 動作確認 (1-2 週間)

- [ ] LongMemEval HF dataset の Rust 側ローダー (`benches/runner/src/adapters/longmemeval.rs`)
- [ ] 30 問の層化抽出ロジック (6 question type × 5 問、seed 固定)
- [ ] 規則ベース primary metric (substring match)
- [ ] LLM judge secondary metric (Qwen3-4B 使用、簡易 prompt)
- [ ] ローカルでの動作確認 (CI 投入前)

#### Step 3: MemoryAgentBench CR + RULER NIAH-S 統合

- [x] **RULER NIAH-S 合成生成スクリプト統合** (PR ①、2026-04-29、`Suite::Smoke`、CPU smoke 用に default 4 ケース {2K/4K/8K/12K})
- [x] **MemoryAgentBench Conflict_Resolution adapter** (PR ②、2026-04-30、`Suite::Cr`、8 行 × `questions[0]`、Bm25Retriever で context 圧縮、`substring_match_any` で同義語マッチ)
- [ ] Tier ablation matrix の実装 (4 構成、PR ③ 予定)
- [x] `bench.yml` workflow を追加し、`workflow_dispatch` のみで初回起動 (本 PR で `cr` / `all` suite も配線済み)

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
  1. 主候補 A (Qwen3.5-4B) なら → `Qwen3.5-0.8B` にダウンサイズ / 主候補 B (Gemma 4 E4B) なら → `Gemma 4 E2B-it` にダウンサイズ
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
- Qwen3.5 model cards on Hugging Face (`Qwen/Qwen3.5-4B`, `Qwen/Qwen3.5-2B`、2026-04-28 直接確認): License: apache-2.0 を明示、Hybrid Gated DeltaNet + Gated Attention + sparse MoE + Vision Encoder アーキテクチャ
- AaryanK/Qwen3.5-{0.8B,2B}-GGUF: GGUF 配布あり、ただし「Ensure you are using the absolute latest version of llama.cpp」と明記
- Hugging Face: `intfloat/multilingual-e5-small` (MIT), `BAAI/bge-small-en-v1.5` (MIT)
- LongMemEval (Wu et al., ICLR 2025), MemoryAgentBench (Hu et al., ICLR 2026), RULER (Hsieh et al., 2024)

---

*最終更新: 2026-04-28*
