# Third-Party Licenses

このリポジトリ自体は `MIT OR Apache-2.0` で配布されるが、CI ベンチマーク
ジョブ (`docs/ci-benchmark-integration-plan.md`) は以下の第三者アセットを
**実行時に download** する。本ファイルはその attribution と revision を
集約する。**本リポジトリにはこれらのアセット (重み・データ) を同梱しない**。

ライセンスが事後に変更された場合は、ここに記録した revision SHA を pin
し続ける運用とする。詳細は計画書 §「ライセンス整合チェック」/「リスクと
対策 §4」参照。

最終更新: 2026-04-28 (Phase 4-α Step 1 雛形)

---

## ベンチマークデータセット

### LongMemEval (Wu et al., ICLR 2025)

- リポジトリ: <https://github.com/xiaowu0162/LongMemEval>
- HF dataset: TBD (Step 2 で revision SHA pin)
- License: MIT
- 利用形態: CI 実行時 download、`_oracle` サブセット 30 問のみ評価。
  生データは artifact に含めない (metric / score のみ保存)。

### MemoryAgentBench (Hu et al., ICLR 2026)

- リポジトリ: TBD
- HF dataset: TBD (Step 3 で revision SHA pin)
- License: MIT
- 利用形態: CI 実行時 download、`Conflict_Resolution` split 全 8 問のみ評価。

### RULER (Hsieh et al., 2024)

- リポジトリ: <https://github.com/NVIDIA/RULER>
- License: Apache 2.0
- 利用形態: 合成生成スクリプトのみ呼び出し、データ非配布。
  `niah_single_2` を seq_len ∈ {4K, 8K, 16K, 32K, 64K} で各 1 ケース、計 5 ケース。

---

## ~~LLM~~ — **削除済 (2026-04)**

tsumugi は 2026-04 に autoregressive LLM 呼び出しを完全撤去し、
encoder-only スタックに確定した。`Qwen3.5-4B` / `unsloth/Qwen3.5-4B-GGUF`
/ `google/gemma-4-e4b-it` 等の LLM weights、および llama.cpp ランタイムは
本リポジトリの依存対象外となった。詳細は `docs/llm-free-stack-plan.md`
§ 5.3。

下流製品が LLM を使う場合、その製品自身が任意の LLM ランタイム
(Ollama / LM Studio / llama.cpp / OpenAI API / Anthropic API 等) と
ライセンスを管理する。tsumugi はそのブリッジを提供しない。

---

## 埋め込みモデル

### intfloat/multilingual-e5-small (第 1 候補)

- HF: `intfloat/multilingual-e5-small`
- ONNX: `Xenova/multilingual-e5-small` (品質確認済み)
- License: **MIT**
- 備考: 118M params, 384 dim, 100+ 言語。CI 4 vCPU で ~30ms/文。

### BAAI/bge-small-en-v1.5 (第 2 候補、英語特化サブセット用)

- HF: `BAAI/bge-small-en-v1.5`
- License: **MIT**
- 備考: 33M params, 384 dim, 英語のみ。第 1 候補で時間が増えた場合の差し替え対象。

---

## Encoder-only LLM-free スタック (Phase 4-γ)

### microsoft/llmlingua-2-bert-base-multilingual-cased-meetingbank (PromptCompressor)

- HF: `microsoft/llmlingua-2-bert-base-multilingual-cased-meetingbank`
- ONNX: HF Hub にネイティブ ONNX 配布なし。CI 内で `optimum-cli export onnx`
  経由で都度 export し、`~/.cache/tsumugi/llmlingua2-mbert/` に出力する
  (`benches/scripts/download_llmlingua2.sh`)。export 結果は actions/cache
  でキャッシュ。
- License: **Apache-2.0** (HF model card で確認、上流訓練データ MeetingBank
  は CC BY-NC-ND 4.0 だが Microsoft が weights を Apache-2.0 で再配布)
- 備考: 110M params (mBERT-base 多言語)。per-token binary classifier
  (keep / discard)。CPU で ~0.8-1.5 s / 10K-tok prompt。tsumugi はデータ
  セット (MeetingBank) を再配布せず、Microsoft 発行の weights のみ HF
  経由で取得して export する。詳細は `docs/llm-free-stack-plan.md` § 8.2。

### MoritzLaurer/mDeBERTa-v3-base-xnli-multilingual-nli-2mil7 (EventDetector default)

- HF: `MoritzLaurer/mDeBERTa-v3-base-xnli-multilingual-nli-2mil7`
- ONNX: HF Hub に Optimum 経由の ONNX 同梱は無いため、必要時に
  `optimum-cli export onnx --task text-classification` で export
  (`docs/llm-free-stack-plan.md` § 5.2 (4))。
- License: **MIT**
- 備考: ~278M params、mDeBERTa-v3-base ベースの 100+ 言語対応 NLI
  分類器 (3-class: entailment / neutral / contradiction)。tsumugi では
  `NliZeroShotDetector` の default モデル。XNLI 15 言語平均 ~80%
  accuracy、MNLI 85.7% / ANLI 53.7%。日本語は XNLI test split に含まれ
  ないため、エンコーダ事前学習 (CC-100) と多言語 NLI fine-tune からの
  cross-lingual transfer に依存する経験的開放問題。

### MoritzLaurer/DeBERTa-v3-base-mnli-fever-anli (EventDetector English-only swap)

- HF: `MoritzLaurer/DeBERTa-v3-base-mnli-fever-anli`
- License: **MIT**
- 備考: ~184M params、DeBERTa-v3-base ベース英語専用 NLI 分類器。
  英語ベンチマーク特化運用で `NliZeroShotDetector::new` に渡し直す
  diff は constructor 引数のみ。MNLI ~90% / ANLI ~50%。多言語不要
  ユースケースで latency / 精度トレードを取る場合の選択肢。

### sshleifer/distilbart-cnn-6-6 (Summarizer default)

- HF: `sshleifer/distilbart-cnn-6-6`
- ONNX: HF Hub にネイティブ ONNX 配布なし。CI 内で
  `optimum-cli export onnx --task text2text-generation-with-past` 経由で 3 ONNX
  graph (encoder / decoder / decoder_with_past) を都度 export し、
  `~/.cache/tsumugi/distilbart-cnn-6-6/` に出力する
  (`benches/scripts/download_distilbart.sh`)。export 結果は
  `actions/cache` でキャッシュ。
- License: **Apache-2.0** (HF model card で確認、上流 BART-large-CNN
  も Apache-2.0)
- 備考: 230M params (BART-large から distilled、6 encoder + 6 decoder
  layers)。CNN/DailyMail ニュース要約で fine-tune。CPU で 1K-tok 入力に
  対し ~1 秒で 80-150 tok の要約を greedy 生成。tsumugi では
  `DistilBartSummarizer` の default モデル、`SummaryMethod::DistilBart`
  variant に対応。日本語要約には不向き (英語ニュースのみ訓練)、
  日本語向けには `ku-nlp/bart-base-japanese` 等への切替が必要だが
  ONNX export 経路は別途検証要。

---

## ランタイム / バイナリ

### ort (Rust ONNX Runtime バインディング)

- crate: `ort` (=2.0.0-rc.10)
- License: MIT / Apache 2.0 dual
- 利用形態: `tsumugi-core` の `onnx` feature 経由で `OnnxEmbedding` /
  `LlmLingua2Compressor` / `SetFitClassifier` / `NliZeroShotDetector` /
  `DistilBartSummarizer` がすべての ONNX 推論を ort で実行。
  `download-binaries` feature で ONNX Runtime バイナリを実行時 download。

### tokenizers (Hugging Face Rust tokenizers)

- crate: `tokenizers` (0.21)
- License: Apache 2.0
- 利用形態: ort と組み合わせて `onnx` feature 配下の各 encoder-only impl
  で BERT/XLM-R/SentencePiece tokenizer を実行。

### llama.cpp — **削除済 (2026-04)**

LLM 撤去と同時に CI からも除外。bench は `llama-server` を起動せず、
encoder-only ONNX 推論のみで完結する。
