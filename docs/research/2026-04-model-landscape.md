# 2026-04 オープンモデル状況調査メモ

このドキュメントは、つかさ/つづり/つくもの MVP 準備段階 (2026-04-22) で行ったオープンモデル状況の調査内容を生データとして残したもの。後続の判断で見返す目的。

## 主要モデルファミリー (2026-04 時点)

### 汎用オープンモデル

| ファミリー | 2026-04 最新 | 注目点 | ライセンス |
|---|---|---|---|
| Qwen 3.5 / 3.6 (Alibaba) | 0.8B / 4B / 9B / 14B / 27B / 35B-A3B / 122B-A10B / 397B | MoE 多用、1M トークン文脈 (3.6 Plus)、2026-02〜03 リリース | Apache 2.0 |
| Gemma 4 (Google) | 2B / E4B / 26B A4B / 31B dense | 256K 文脈、140 言語対応、31B が Claude Sonnet 4.5 並のベンチ、2026-04-02 リリース | Apache 2.0 |
| GLM-5 / 5.1 (Zhipu AI) | reasoning 版がオープン最強 (BenchLM 85) | 中国勢の台頭 | - |
| Kimi K2.5 / K2.6 (Moonshot) | オープン重量級の品質リーダー | LMArena 上位 | - |
| DeepSeek V3.2 | 685B | まだ競争力あるがリーダーではない | - |
| Llama 4 Scout / Maverick (Meta) | 109B / 400B MoE | Scout は 10M 文脈、多モーダルネイティブ | Llama License |
| Phi-4 / Phi-4 Mini (Microsoft) | 14B / 3.8B | 高速、コーディング特化 | MIT |
| GPT-OSS (OpenAI) | 20B / 120B | MXFP4 ネイティブ配布 | Apache 2.0 |
| Mistral Small 4 | 24B | ヨーロッパ勢代表 | Apache 2.0 |
| Devstral-2 (Mistral) | 123B | コーディング特化、Vibe CLI 同梱 | Apache 2.0 |

### 日本語特化 / 日本語強化

| モデル | 基盤 | サイズ | 特徴 | ライセンス |
|---|---|---|---|---|
| Qwen3 Swallow (Science Tokyo + AIST) | Qwen3 | 8B / 30B-A3B / 32B | 2026-02 時点で同サイズクラス日本語 SOTA、reasoning 対応 | Apache 2.0 |
| GPT-OSS Swallow | GPT-OSS | 20B / 120B | 日英両対応、2026 | Apache 2.0 |
| ELYZA-Diffusion-1.0-Dream-7B | Dream (7B) | 7B | 拡散ベースの日本語 LLM、2026 | Apache 2.0 |
| ELYZA-Thinking-1.0-Qwen-32B | Qwen 2.5 32B | 32B | reasoning | Apache 2.0 |
| ELYZA-Shortcut-1.0-Qwen-32B | Qwen 2.5 32B | 32B | 通常指示 | Apache 2.0 |
| ABEJA-Qwen2.5-32b-Japanese-v1.0 | Qwen 2.5 32B | 32B | 継続事前学習 + SFT + DPO | Apache 2.0 |
| Qwen2.5 Bakeneko 32B | Qwen2.5 32B | 32B | rinna 系、DeepSeek-R1 distill 版もあり | Apache 2.0 |
| Llama-3-ELYZA-JP-8B | Llama 3 8B | 8B | 16k DL、根強い人気 | Llama 3 License |
| Llama-3.1-Swallow-8B-v0.5 | Llama 3.1 8B | 8B | 日本語 MT-Bench SOTA | Llama 3.1 License |
| llm-jp-3.1-1.8b-instruct4 | 独自 | 1.8B | NII 製、超軽量 | Apache 2.0 |

### コーディング特化

| モデル | サイズ | 特徴 |
|---|---|---|
| Qwen2.5-Coder 7B | 7B | 88.4% HumanEval (GPT-4 超え)、定番 |
| Qwen2.5-Coder 32B | 32B | SWE-Bench Verified 69.6%、Claude 3.5 Sonnet 相当 |
| Qwen3-Coder 30B-A3B | 30B MoE | 24GB GPU で最良のコーディング MoE |
| Qwen3-Coder-Next 80B A3B | 80B MoE | SWE-bench Verified 70.6%、64GB MacBook で Sonnet 4.5 級 |
| Phi-4 Mini | 3.8B | 高速、軽量コード補完 |
| Devstral-2 | 123B | Mistral 専用コーディング |
| GLM-4.7 Flash | - | オープンコーディング最強 (LiveCodeBench 89%) |

## Bonsai ファミリー (ネイティブ低 bit モデル)

### deepgrove の Bonsai (2025-03)

- 0.5B パラメータのテルナリー重み (BitNet b1.58 スタイル)
- Llama アーキ + Mistral トークナイザー、Danube 3 ベース
- DCLM-Pro + Fineweb-Edu で 5B トークン未満で訓練
- 英語専用、instruction tuning なし
- Apache 2.0

### PrismML の Bonsai (2026-03-31)

Caltech 発、Khosla Ventures + Cerberus + Google が $16.25M 出資。

| モデル | サイズ | メモリ | 品質 |
|---|---|---|---|
| 1-bit Bonsai 8B | 8B | 1.15 GB | Llama 3 8B 相当、平均スコア 70.5 |
| 1-bit Bonsai 4B | 4B | 0.57 GB | M1 Air で 132 tok/s |
| 1-bit Bonsai 1.7B | 1.7B | 約 0.3 GB | ブラウザ WebGPU で動作 |
| Ternary Bonsai 8B | 8B | 1.75 GB | 平均 75.5、Qwen3 8B (16GB) の次点 |
| Ternary Bonsai 4B | 4B | 約 1 GB | M4 Pro で 82 tok/s |
| Ternary Bonsai 1.7B | 1.7B | 約 0.4 GB | |

特徴:
- FP16 比 14 倍メモリ効率、8 倍高速、5 倍エネルギー効率 (PrismML 公称)
- Apache 2.0、GGUF + MLX 両対応
- 事後量子化ではなく 1-bit/ternary ネイティブ訓練
- M4 Pro で Ternary Bonsai 8B 82 tok/s、iPhone 17 Pro Max で 27 tok/s
- llama.cpp の `Q1_0` で動作、MLX は `mlx-2bit`

### Bonsai の日本語適性

現状、Bonsai ファミリーはすべて英語中心。日本語評価は公開されていない。MVP では使えない。

ただし Kenya 自身の 2-bit / quaternary quantization 研究と方向性が一致しており、将来「日本語 Ternary Swallow」的なモデルが 2026 後半〜 2027 に登場する可能性は中程度。tsumugi の LLMProvider trait が低 bit 量子化モデルを透過的に扱えるよう設計しておく戦略的価値あり。

## 量子化スペクトラム (2026-04)

### bit 数と品質保持

| bit 数 | フォーマット例 | 品質保持 | 8B モデルの実効サイズ |
|---|---|---|---|
| 16-bit FP16/BF16 | safetensors native | 100% | 約 16 GB |
| 8-bit | Q8_0, INT8, W8A8 | ~99% | 約 8 GB |
| 6-bit | Q6_K | ~97% | 約 6 GB |
| 5-bit | Q5_K_M | ~95-96% | 約 5.5 GB |
| 4-bit | Q4_K_M, AWQ, GPTQ, MXFP4, MLX 4bit | 92-95% | 約 4.5 GB |
| 3-bit | Q3_K_M, IQ3_XXS, IQ3_M | 85-90% | 約 3.5 GB |
| 2-bit | Q2_K, IQ2_M, MLX 2bit | 70-80% | 約 2.5 GB |
| ternary (1.58 bit) | Ternary Bonsai native | 訓練次第、事後量子化では劣化 | 約 1.75 GB |
| 1-bit | 1-bit Bonsai native | 訓練次第 | 約 1.15 GB |

### 2026 年の新しい手法

- **QAT (Quantization-Aware Training)**: Gemma 3 / Gemma 4 は QAT 済み版を公式配布、同じ bit 数でも事後量子化より 2-5% 品質向上。Gemma 4 E4B の "effective 4B" は QAT 前提
- **Unsloth Dynamic Quantization**: imatrix キャリブレーションで重要重みの精度保持、4-bit で 95-97% 品質
- **MXFP4**: OpenAI の FP4 ネイティブ形式、GPT-OSS 20B/120B で採用、H100 以降の GPU で native 対応
- **ネイティブ低 bit 訓練**: BitNet b1.58 2B4T (Microsoft 2025-04)、1-bit/Ternary Bonsai (PrismML 2026-03)、事後量子化の限界を超える可能性
- **KV cache 量子化**: 重み量子化と独立、MLX の TurboQuant で 4.6x 圧縮、長文脈で VRAM 劇的節約
- **MoE + 量子化**: Qwen3 30B-A3B Q4 で 16GB、品質は 20-27B dense 相当、アクティブ 3.3B だから速度も速い

### 量子化による日本語品質の非対称劣化リスク

英語ベンチで Q4_K_M が 92% 保持でも、日本語生成では 85% くらいに落ちる可能性 (トークナイゼーション特性から)。日本語用途では Q5_K_M を推奨下限とすべきかもしれない。要実機検証。

## Apple Silicon / MLX の進化

- 2026-03: Ollama が Apple Silicon でメインバックエンドを MLX に切り替え、llama.cpp 比 20-30% 高速化
- M5 Neural Accelerators: FLUX 画像生成で M4 比 3.8x 高速
- M1/M2 16GB: 8B モデル (Llama-3-ELYZA-JP-8B、Qwen3 Swallow 8B) 快適
- M3 Pro 18-32GB: Qwen 3.5 9B で 25-35 tok/s
- M4 Max 36-48GB: Gemma 4 26B A4B で 30-45 tok/s
- MLX TurboQuant: KV cache 4.6x 圧縮、長文脈で大きな価値

## MoE アーキテクチャの影響

MoE が最小構成の定義を書き換えた。

例: Qwen3-30B-A3B
- 総 30.5B パラメータ、アクティブ 3.3B (128 エキスパートから 8 個選択)
- 推論速度は 3B dense 相当 (高速)
- 品質は 14-20B dense 相当
- Q4 量子化で VRAM 16-17GB で動く

例: Gemma 4 26B A4B
- 総 26B、アクティブ 4B
- Q5 で 8GB VRAM で動く
- 品質は 20-27B dense 相当

これで「RTX 3060 (12GB) ユーザーが 30B 級品質を 14B 級速度で得られる」時代になった。

## 推論ランタイム比較

| ランタイム | 適性 | 推奨度 |
|---|---|---|
| Ollama | 最も普及、Apple Silicon で MLX 自動適用 | 一般ユーザー向け推奨 |
| LM Studio | GUI 付き、Windows 層で人気 | ノンコーダー層に最適 |
| llama.cpp | 直接統合、細かい制御 | ヘビーユーザー向け |
| MLX (Apple) | Apple Silicon 専用、最高性能 | Mac 上級者向け |
| vLLM | サーバー用途、高スループット | つむぎ製品群では不要 |

OpenAI 互換 API (LM Studio / Ollama) に対応すれば、ユーザーはどちらでも使える。これが LLMProvider trait の第一実装の方針。

## 構造化出力の選択肢

つくもの EventCommand 生成には必須。

- **GBNF (llama.cpp)**: 文法制約で形式保証、llama.cpp / Ollama / LM Studio で利用可能
- **XGrammar**: 高速な文法制約、vLLM 等で採用
- **Outlines**: Python ライブラリ、正規表現 / JSON Schema / pydantic 対応
- **JSON Mode** (各社 API): OpenAI 互換 API ならネイティブ対応
- **Tool calling / Function calling**: Qwen3, Gemma 4, Llama 4 すべてネイティブサポート

GBNF が最も移植性高い (llama.cpp ベースのすべての runtime で動く)。MVP の第一選択。

## 確度・不確実性の評価

### 確度が高い情報

- 主要モデルファミリーの存在と基本スペック (公式リリース済み)
- 量子化スペクトラム (標準化された指標)
- Apple MLX の Ollama 統合 (2026-03 発表済み)
- MoE 効率性 (Qwen3-30B-A3B の仕様)
- Bonsai の数値スペック (PrismML 公称値)

### 確度が低い / 要実機検証

- Ternary Bonsai 8B / 1-bit Bonsai 8B の実タスク品質 (ベンチと実感のギャップ)
- 日本語量子化時の品質劣化率 (英語ベンチからの外挿)
- Qwen3 Swallow 8B の AIのべりすと代替としての文体品質
- ELYZA-Diffusion-7B の拡散モデル特性
- Unsloth Dynamic Quantization の客観的優位性 (独立ベンチ乏しい)
- R-18 対応 uncensored 系 fine-tune の品質とライセンス
- 日本語 ternary モデル登場時期 (2026 後半〜 2027 の予測は中程度確度)

### MVP で検証すべき項目

1. Qwen3 Swallow 8B Q5_K_M で AIのべりすと相当の小説品質が出るか (つづり)
2. Qwen3 Swallow 8B Q5_K_M で CoC NPC 台詞の口調一貫性が保てるか (つかさ)
3. Qwen2.5-Coder 7B Q5_K_M で RPGツクール MZ EventCommand 生成が実用品質か (つくも)
4. Qwen3-30B-A3B Q4 で上位体験が 8B と有意差を生むか (全製品)
5. MacBook Air M1/M2 16GB で Qwen3 Swallow 8B が快適に動くか (つづり主ターゲット)
6. Apple MLX 経由での Gemma 4 / Qwen 3.5 の対応状況 (Mac ユーザー向け)
7. GBNF 制約下での Qwen2.5-Coder 7B の JSON + JavaScript 生成精度 (つくも)

## 次ステップへの示唆

### tsumugi 側で対応すべき設計上の考慮

- LLMProvider trait は量子化レベル・MoE 構成・KV cache 設定を model metadata で受け取れる
- OpenAI 互換 API を第一実装に (LM Studio / Ollama を両方カバー)
- Grammar-constrained generation (GBNF) を統一的に扱えるインターフェイス
- 将来的な 1-bit / ternary ネイティブモデル対応を設計視野に

### 各製品側で対応すべき UX

- 最小 / 推奨 / 快適の 3 段階ハードウェア要件を明示
- 推奨モデル (Unsloth Dynamic Quantization 版を推奨) とインストール手順
- KV cache 量子化設定の exposure (長文脈で効く)
- ユーザー環境ベンチマーク (起動時に推奨モデル自動選定)

---

*調査日: 2026-04-22。情報は急速に変化する。6 ヶ月ごとに再調査推奨。*
