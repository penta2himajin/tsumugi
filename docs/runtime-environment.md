# つむぎ — ランタイム環境ガイド

つむぎを使う上位製品 (つかさ / つづり / つくも) の共通リファレンスとして、ハードウェア帯域別の推奨モデルと設定を整理する。製品固有の推奨は各製品の `docs/tech-architecture.md` を参照。

詳細な調査背景は `docs/research/2026-04-model-landscape.md` を参照。

## 設計哲学

つむぎを利用する製品は**最小構成で動くこと**を絶対基準とする。推奨 / 快適層への拡張はユーザーが自発的に選ぶものであり、MVP の成立条件ではない。

一方で、MoE アーキテクチャの普及と量子化技術の進展により、「最小構成」の定義が 2025 年と比べて大きく変わっている。MoE で 30B クラスを 16GB VRAM で動かせるようになったため、中央値ユーザーの体験は大幅に改善している。

## ハードウェア 3 段階の定義

### 最小構成 (Minimum)

- 統合 GPU / 古めの dGPU / 8GB RAM ノート PC
- AIのべりすと ボイジャー (年 ¥10,800) や Cursor (月 $20) 等のサブスクを払っていない層
- Apple Silicon M1 (8GB Unified Memory) 含む
- **製品は必ずこの層で機能する**のが条件

### 推奨構成 (Recommended)

- RTX 3060 (12GB VRAM) / RTX 5070 (12GB) / M1 Pro 16GB / M2 16GB
- 買い切り AI ツールに ¥8,800〜14,800 払える層
- MoE モデルの恩恵を受ける帯域

### 快適構成 (Comfortable)

- RTX 3090 / 4090 (24GB VRAM) / M3 Max / M4 Max (36-64GB)
- プロ作家 / 配信者 / ヘビー同人作家
- 上位版の訴求対象

## ハードウェア帯域別推奨モデル

### 最小構成 (統合 GPU / 8GB RAM)

| 用途 | 推奨モデル | Q | 実効メモリ |
|---|---|---|---|
| 日本語汎用 | Llama-3-ELYZA-JP-8B | Q3_K_M | 約 4 GB |
| 日本語汎用 | Qwen3 Swallow 8B | Q3_K_M | 約 4 GB |
| 日本語軽量 | llm-jp-3.1-1.8b-instruct4 | Q5_K_M | 約 1.3 GB |
| 多言語軽量 | Gemma 4 E4B (QAT) | Q5_K_M | 約 3 GB |
| 多言語軽量 | Qwen 3.5 4B | Q5_K_M | 約 3 GB |
| コード生成 | Phi-4 Mini | Q5_K_M | 約 2.5 GB |
| コード生成 | Qwen2.5-Coder 3B | Q5_K_M | 約 2 GB |

注意点:
- Q3_K_M は品質劣化が顕著で、文体・台詞生成には厳しい
- 日本語用途は Q5_K_M を下限にすべき (英語に比べ量子化耐性が低い)
- 1-bit / Ternary Bonsai は日本語未対応のため、現時点では選択肢にならない

### 推奨構成 (RTX 3060 12GB / M1 Pro 16GB)

| 用途 | 推奨モデル | Q | 実効メモリ |
|---|---|---|---|
| 日本語汎用 | **Qwen3 Swallow 8B (Unsloth Dynamic)** | Q5_K_M | 約 6 GB |
| 日本語汎用 | Llama-3.1-Swallow 8B | Q5_K_M | 約 6 GB |
| 日本語強化 | Qwen3 Swallow 30B-A3B (MoE) | Q4_K_M | 約 16 GB |
| 多言語 | Gemma 3 12B | Q4_K_M | 約 7 GB |
| コード生成 | Qwen2.5-Coder 7B | Q5_K_M | 約 5 GB |
| コード生成 | Qwen3-Coder 30B-A3B (MoE) | Q4_K_M | 約 16 GB |

MoE の恩恵 (この帯域の核心):
- Qwen3-30B-A3B: 16GB VRAM で 20-27B dense 相当の品質、3B dense 相当の速度
- Gemma 4 26B A4B: 同様に 14GB で動く

### 快適構成 (RTX 3090 24GB / M4 Max 36GB+)

| 用途 | 推奨モデル | Q | 実効メモリ |
|---|---|---|---|
| 日本語最強 | **Qwen3 Swallow 32B** | Q4_K_M | 約 18 GB |
| 日本語強化 | GPT-OSS Swallow 20B | MXFP4 | 約 10 GB |
| 日本語強化 | ABEJA-Qwen2.5-32b-Japanese | Q4_K_M | 約 18 GB |
| 多言語 | Gemma 4 31B | Q4_K_M | 約 20 GB |
| 多言語 | Qwen 3.5 35B-A3B (MoE) | Q4_K_M | 約 18 GB |
| コード生成 | Qwen3-Coder 30B-A3B | Q5_K_M | 約 22 GB |

### ハイエンド (64GB Mac / マルチ GPU)

| 用途 | 推奨モデル | Q | 実効メモリ |
|---|---|---|---|
| 日英両対応 | GPT-OSS Swallow 120B | MXFP4 | 約 60 GB |
| 多言語 | Qwen 3.5 122B-A10B (MoE) | Q4_K_M | 約 68 GB |
| コード生成 | Qwen3-Coder-Next 80B A3B | Q4_K_M | 約 42 GB |

## 量子化レベル選択指針

### 汎用指針

1. **Q5_K_M を日本語用途の推奨下限**とする (英語 Q4_K_M 相当の品質)
2. **Q4_K_M** は多言語・コード生成用途で実用スイートスポット
3. **Q3_K_M** は 4GB GPU 等の制約下でのみ選択、品質劣化は許容前提
4. **Q2_K / IQ2** は非推奨、緊急時のみ

### Unsloth Dynamic Quantization の優先

Hugging Face で Unsloth が配布している imatrix キャリブレーション済み GGUF を第一選択とする。同じ Q4_K_M でも 2-3% 品質向上。

### QAT 済みモデルの優先

Gemma 3 / Gemma 4 は QAT 済み版を配布しており、事後量子化比で 2-5% 品質向上。E4B 表記は QAT 前提の "effective" パラメータ数。

### KV cache 量子化

- MLX の TurboQuant: 4.6x 圧縮、長文脈で効く
- llama.cpp の `--cache-type-k q4_0 --cache-type-v q4_0`: KV キャッシュ Q4 化
- つむぎの Context Compiler は大量コンテキストを渡すため、KV cache 量子化への対応は**長期プロジェクトの長文生成で実効的**

## 推論ランタイムの選択

| ランタイム | 推奨ユーザー層 | 強み |
|---|---|---|
| **Ollama** | 一般ユーザー (デフォルト推奨) | 最も普及、Apple Silicon で MLX 自動適用 (2026-03〜) |
| **LM Studio** | ノンコーダー / つくも層 | GUI 付き、Windows 層で人気 |
| llama.cpp | ヘビーユーザー / 組込み | 直接統合、細かい制御 |
| MLX (直接) | Mac 上級者 | Apple Silicon 専用、最高性能 |

OpenAI 互換 API を実装すれば Ollama / LM Studio のどちらでも動く。これを **`LLMProvider` trait の第一実装**にする。

## 構造化出力の選択

つくもの EventCommand 生成等、JSON 形式保証が必要な用途:

- **GBNF (llama.cpp)**: 最も移植性が高い、Ollama / LM Studio で動く。**第一選択**
- **JSON Mode** (各社 API): OpenAI 互換 API ならネイティブ
- **Tool calling / Function calling**: Qwen3 / Gemma 4 / Llama 4 すべて対応

## 将来検討 (設計視野だけ残す)

### 1-bit / Ternary ネイティブモデル

- PrismML の Bonsai ファミリー (2026-03 リリース) は英語専用だが、8B クラスを 1-2 GB で動かせる
- Kenya の 2-bit / quaternary quantization 研究と方向性が一致
- 2026 後半〜 2027 に日本語 ternary 8B が登場する可能性は中程度
- 登場した時点で即座に対応できるよう、`LLMProvider` trait は量子化形式を抽象化

### モバイル動作

- iPhone 17 Pro Max で Ternary Bonsai 8B が 27 tok/s
- つむぎ製品群のモバイル版は MVP 対象外だが、Phase 4 以降の拡張候補
- WebGPU (Bonsai 1.7B がブラウザ動作) も将来の埋め込み配布候補

### Apple M5 Neural Accelerators

- M5 (2025 末) 以降で MLX が M4 比 3.8x 高速化
- Mac ユーザーの体験は急速に向上、Apple Silicon 特化の価値は増している

## 要実機検証項目 (MVP Phase 0)

1. Qwen3 Swallow 8B Q5_K_M が AIのべりすと代替として文体品質で通用するか (つづり)
2. Qwen3 Swallow 8B Q5_K_M が CoC NPC 台詞の口調一貫性を保てるか (つかさ)
3. Qwen2.5-Coder 7B Q5_K_M が RPGツクール MZ EventCommand 生成で実用品質か (つくも)
4. Qwen3-30B-A3B Q4 と Qwen3 Swallow 8B Q5_K_M で上位体験が有意差を生むか (全製品)
5. MacBook Air M1/M2 16GB で Qwen3 Swallow 8B が快適に動くか (つづり主ターゲット)
6. GBNF 制約下での Qwen2.5-Coder 7B の JSON + JavaScript 生成精度 (つくも)

## メタデータ

- 調査日: 2026-04-22
- 想定次回レビュー: 2026-10 前後 (半年ごと)
- 情報が急速に変化する領域のため、MVP 実装直前にも再確認推奨
