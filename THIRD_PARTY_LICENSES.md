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

## LLM 候補 (Phase 4-α Step 1 smoke test 後に 1 つに確定)

### A. Qwen3.5-4B (text-only)

- 公式モデル: `Qwen/Qwen3.5-4B` (safetensors 配布、Apache 2.0)
- GGUF 配布: `unsloth/Qwen3.5-4B-GGUF` (community quantization)
  - 取得 quant: `Qwen3.5-4B-Q4_K_M.gguf`
  - revision SHA: TBD (Step 1 smoke 安定後に pin)
  - License: 公式モデルから継承する Apache 2.0
- License: **Apache 2.0** (HF model card で 2026-04-28 に確認)
- 備考: Multimodal VL モデルだが mmproj 非ロードで text-only 動作可能。
  Hybrid Gated DeltaNet + Gated Attention + sparse MoE のため
  llama.cpp は最新 master 系の build を pin する必要あり。
  公式 Qwen org からは GGUF 配布が無い (2026-04 時点)、`unsloth` の
  community 配布を使用している。Qwen 公式 GGUF が後日公開された場合は
  そちらに切り替える。

### B. Gemma 4 E4B-it

- HF: `google/gemma-4-e4b-it`
- GGUF (Q4_K_M / UD-Q4_K_XL): `unsloth/gemma-4-E4B-it-GGUF` または ggml-org 配布
- License: **Apache 2.0** (2026-03 に Gemma Terms から変更、Google Open Source Blog 確認)
- 備考: Day-0 llama.cpp 公式サポート、CPU 推論実績豊富。

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

## ランタイム / バイナリ

### llama.cpp

- リポジトリ: <https://github.com/ggml-org/llama.cpp>
- License: MIT
- 利用形態: CI で release バイナリを download、tag は smoke test 後に pin。

### ort (Rust ONNX Runtime バインディング)

- crate: `ort`
- License: MIT / Apache 2.0 dual
- 利用形態: `tsumugi-core` の `onnx` feature 経由で `OnnxEmbedding` から呼ぶ。
  Phase 4-α Step 1 では trait 面のみ先行追加、ort 統合は並行で進む。
