# LLM 委譲撤去アーキテクチャ計画

tsumugi-core は `LLMProvider` trait を介して 4 箇所で LLM (自己回帰生成モデル) に
委譲している。本書はこの 4 箇所を **encoder-only モデル / 古典統計手法** で
段階的に置換するための計画文書。

「LLM 不使用」とは GPT/Claude/Llama のような自己回帰生成モデルを呼ばないこと
を指す。BERT/MiniLM/ModernBERT/XLM-RoBERTa/DistilBERT 等の encoder-only
モデルや、TF-IDF/BM25 のような古典統計手法は本書の文脈で「LLM 不使用」に
含む。DistilBART のような encoder-decoder (生成系) は本来 "LLM 不使用" の
範疇からは外れるが、tsumugi における「`LLMProvider` 経由の `complete()`
呼び出しを撤去する」という運用上の目的に対しては機能的等価とみなす
(任意の LLM provider に依存せず、固定された軽量モデルでタスクを完結できる)。

詳細な経緯と評価基盤は
[`ci-benchmark-integration-plan.md`](./ci-benchmark-integration-plan.md)
の Tier ablation matrix と接続する。

## 1. 背景と動機

Phase 4-α Step 3 PR ③ (Tier ablation matrix) によって以下が観測された:

- LLM 不使用 ablation (`tier-0` / `tier-0-1` / `tier-0-1-2`) は CR 8 ケースで
  **per case ~10-50 ms** の retrieval recall path として完走する
- LLM ablation (`full`) は同 8 ケースで **per case ~700 sec** (Qwen3.5-4B
  Q4_K_M @ 4 vCPU CPU) かかり、accuracy も 1/8 (12.5%) と低い
- BM25 retrieval recall は 7/8 (87.5%) であり、**ボトルネックは retrieval
  ではなく LLM** であることが定量的に示された

この結果は「Mayu の価格設計上 LLM-free な Tier 0-2 の経路を堅牢にしたい」
という `monetization-strategy.md` の方針と整合する。一方で現状の Tier
2/3 実装は `LlmLinguaCompressor` / `LlmSummarizer` / `BertClassifier` /
`LLMClassifierDetector` がいずれも `LLMProvider` 経由で外部 LLM に依存
しており、ローカルで CPU 数十-数百 ms で完結する選択肢になっていない。

本計画ではこの 4 コンポーネントを encoder-only モデルベースの実装に
置換し、**全パスを LLM 委譲なしで動作させる** ことを目指す。既存 LLM
委譲版は `Summarizer` / `PromptCompressor` / `QueryClassifier` /
`EventDetector` の代替実装として残し、deprecate しない (オプション)。

## 2. 現状の LLM 依存ポイント

`grep -r "\.complete(" tsumugi-core/src/` で確認できる 4 箇所
(テスト除く):

| trait | 実装 | tier | ファイル | 役割 |
|---|---|---|---|---|
| `Summarizer` | `LlmSummarizer` | 3 | `summarizer/llm.rs:48-62` | chunk を 2-4 文に要約 (固有名詞・因果順序保持) |
| `PromptCompressor` | `LlmLinguaCompressor` | 2 | `compressor/llm_lingua.rs:49-68` | prompt を `target_budget_tokens` に圧縮 (entity/数値/日付/quote 保持) |
| `QueryClassifier` | `BertClassifier` | 3 (stub) | `classifier/bert_classifier.rs:82-97` | query を {Literal, Narrative, Analytical, Unknown} に分類 |
| `EventDetector` | `LLMClassifierDetector` | 2/3 | `detector/llm_classifier.rs:43-74` | chunk + new turn の per-label yes/no |

これらは全て `Arc<dyn LLMProvider>` を保持し、prompt template を render
して `complete()` を呼ぶ構造。同一 trait に **LLM 不使用の代替実装** が
既に存在する (`ExtractiveBM25Summarizer` / `TruncateCompressor` /
`SelectiveContextCompressor` / `RegexClassifier` / `KeywordDetector` /
`EmbeddingDetector`) ため、新実装は同 trait の追加 impl として導入する
だけで API 変更は不要。

## 3. 関連研究サーベイ

各コンポーネントについて非 LLM 代替手法を新旧網羅的に整理する。性能評価
は相対 (★1-5)、CPU 速度は 4 vCPU + 7GB RAM の CI runner を想定した推定値
(実機未測定、要検証)。Pareto frontier 上の手法のみ抜粋し、前述
[`research`](./research/) ディレクトリ配下に詳細を将来配置する余地を残す。

### 3.1 Abstractive Summarization

性能指標: ROUGE-1/2/L (CNN-DM 系) と factual consistency。

| 手法 | パラメータ | 性能 | CPU 速度 (1K tok chunk) | 種別 |
|---|---|---|---|---|
| PEGASUS-X (Phang 2022) | 568M | ★★★★★ | ~3-5 s | encoder-decoder |
| MatchSum (Zhong 2020) | 110M Siamese × N候補 | ★★★★★ | ~1-2 s × N | extractive |
| **DistilBART-CNN-6-6** (Shleifer 2020) | 230M | ★★★★ | ~0.8-1.5 s | encoder-decoder |
| BERTSum / PreSumm (Liu 2019) | 110M | ★★★★ | ~0.3-0.5 s | extractive |
| FLAN-T5-base (Chung 2022) | 250M | ★★★ | ~1-2 s | encoder-decoder |
| SBERT centroid + MMR | 22-110M | ★★★ | ~50-100 ms | embedding-based |
| LexRank (Erkan 2004) | 0 | ★★ | ~5-10 ms | graph-based |
| TextRank (Mihalcea 2004) | 0 | ★★ | ~5-10 ms | graph-based |

### 3.2 Prompt Compression

性能指標: 同 budget での downstream task (QA/Summarization) accuracy 保持率。

| 手法 | パラメータ | 性能 | CPU 速度 (10K tok prompt) | 種別 |
|---|---|---|---|---|
| LLMLingua-2 XLM-R-large (Pan 2024) | 560M | ★★★★★ | ~3-5 s | per-token classifier |
| LongLLMLingua (Jiang 2024) | GPT-2-tiny + iter | ★★★★★ | ~5-10 s | iterative (AR LM 利用) |
| **LLMLingua-2 mBERT-base** (Pan 2024) | 110M | ★★★★ | ~0.8-1.5 s | per-token classifier |
| RECOMP-extractive (Xu 2024) | dual SBERT 22-110M | ★★★★ | ~100-300 ms | dual encoder (query-dep) |
| Selective Context (Li 2023) | GPT-2-small (124M) | ★★★ | ~1-2 s | self-information (AR LM 利用) |
| Entity-keep + IDF span pruning | 0 | ★★★ | ~10-30 ms | classical hybrid |
| TruncateCompressor (現行 Tier 0) | 0 | ★ | ~1 ms | head + tail |

### 3.3 Query Classification

設定: 4 ラベル × 限定訓練データ (8-100 examples/class)。性能指標は few-shot
accuracy。

| 手法 | パラメータ | 性能 | CPU 速度 (1 query) | 種別 |
|---|---|---|---|---|
| SetFit + ModernBERT-embed (Wasserblat 2024) | 150M | ★★★★★ | ~30-50 ms | contrastive few-shot |
| ModernBERT fine-tuned (Warner 2024) | 150M | ★★★★★ | ~30-50 ms | classification head (要十分データ) |
| **SetFit + MiniLM-L6** (Tunstall 2022) | 22M | ★★★★ | ~5-10 ms | contrastive few-shot |
| DeBERTa-v3-MNLI zero-shot (Laurer 2024) | 184M | ★★★★ | ~80-150 ms (4 ラベル) | NLI entailment |
| FastFit (Yehudai 2024) | 22-110M | ★★★★ | ~10-20 ms | batch contrastive |
| SBERT + logistic probe | 22-150M | ★★★ | ~5-10 ms | encoder + linear |
| fastText (Joulin 2017) | 0 (subword hash) | ★★ | ~0.5-1 ms | subword bag |
| TF-IDF + Naive Bayes / SVM | 0 | ★★ | ~0.1-0.5 ms | classical |

### 3.4 Per-Label Binary Event Detection

性能指標: chunk (~1K tok) + 新 turn → 各ラベル yes/no の F1。N = ラベル数。

| 手法 | パラメータ | 性能 | CPU 速度 (chunk あたり) | 種別 |
|---|---|---|---|---|
| **GLiNER2** (2025) | ~200M | ★★★★★ | ~200-400 ms (全ラベル 1 pass) | span-classification (zero/few-shot) |
| GLiNER (Zaratiana 2024) | ~200M | ★★★★★ | ~150-300 ms (全ラベル 1 pass) | span-classification |
| DeBERTa-v3-MNLI zero-shot | 184M | ★★★★ | ~80-150 ms × N | NLI entailment |
| SetFit-binary fan-out | 22-150M (共有) | ★★★★ | encoder 1 pass + N logistic | few-shot |
| TARS (Halder 2020) | 110M | ★★★★ | ~50-100 ms × N | label-conditioned binary |
| Cross-encoder MS-MARCO MiniLM | 22M | ★★★ | ~10-20 ms × N | cross-encoder |
| SBERT bi-encoder cosine (現行 Tier 1) | 22-110M | ★★ | ~5-15 ms × N | bi-encoder |


## 4. 採用アーキテクチャ

各コンポーネントについて以下の構成を採用する。選定基準は **(a) Pareto
frontier 上にあり、(b) HF Hub に weights が公開されており、(c) ONNX export
の道筋が確立しており、(d) CI runner (4 vCPU + 7GB RAM) で動作可能** な
ものとした。

### 4.1 Abstractive Summarization → DistilBART-CNN-6-6

| 項目 | 値 |
|---|---|
| モデル | `sshleifer/distilbart-cnn-6-6` (HF Hub) |
| パラメータ | 230M |
| 推定 CPU 速度 | ~0.8-1.5 s / 1K tok chunk |
| 種別 | encoder-decoder (BART distilled) |
| 訓練データ | CNN/DM (英語ニュース要約) |
| 新 impl 名 (案) | `DistilBartSummarizer` |

**選定理由**:

- abstractive 系で速度/精度の sweet spot (DistilBART-CNN-12-6 が ~98%
  ROUGE 保持で 2x 高速、6-6 はそれをさらに半分の decoder layer に圧縮)
- `rust-bert` に BART pipeline が同梱されており参考実装が存在
- HF Optimum で ONNX export 経路が確立、`ort` で int8 量子化推論が可能

**留意点**:

- 訓練データが英語ニュースに偏っているため、日本語チャンクに対する
  品質は要実機検証。日本語の比重が大きい運用では Japanese-tuned BART
  (例: `ku-nlp/bart-base-japanese`) との比較が必要
- encoder-decoder 系であり generation loop が必要 → ort の場合は手書き、
  candle-transformers の場合は BART 既存実装を流用可能
- Tier 1 の `ExtractiveBM25Summarizer` で済むケースが多い場合、Tier 3
  化する意義は限定的。`HierarchicalSummarizer` の上位レベル要約用と
  位置づけるのが妥当

### 4.2 Prompt Compression → LLMLingua-2-mBERT

| 項目 | 値 |
|---|---|
| モデル | `microsoft/llmlingua-2-bert-base-multilingual-cased-meetingbank` (HF Hub) |
| パラメータ | 110M (mBERT-base 多言語) |
| 推定 CPU 速度 | ~0.8-1.5 s / 10K tok prompt |
| 種別 | per-token binary classifier (keep/discard) |
| 訓練データ | GPT-4 distillation on MeetingBank |
| 新 impl 名 (案) | `LLMLinguaCompressor` (※ 既存名から `Llm` を外す or `LLMLingua2EncoderCompressor` 等で新設) |

**選定理由**:

- LLMLingua-2 (Pan et al., ACL Findings 2024) の paper-exact 実装が
  目標であり、これは tsumugi 設計時から計画書に記載済み (`tech-architecture.md`)
- mBERT-base 版は XLM-R-large (560M) 版に対し速度 5x、精度差 ~2 pt のみ
- 多言語対応 (mBERT は 104 言語の Wikipedia で事前学習) のため日本語
  prompt の圧縮にも適用可能
- 出力が plain text (per-token mask 後の concatenation) のため tsumugi
  の `PromptCompressor` trait が要求する `String` インターフェースと
  完全互換

**留意点**:

- 訓練分布 (MeetingBank = 会議書き起こし) と実運用分布の乖離による
  精度低下があり得る。実機で `tier-0-1-2` の retrieval recall vs
  圧縮後 recall を計測する必要あり
- 110M は CPU で 1 秒台、Truncate (1 ms) と比較すると 1000 倍遅い。
  ablation matrix で Truncate との性能差が圧縮率の差を上回るか実機検証
- 既存 `LlmLinguaCompressor` (LLM 委譲版) は名称が衝突するため、新 impl
  はリネームを伴う。詳細は §5

### 4.3 Query Classification → SetFit + MiniLM

| 項目 | 値 |
|---|---|
| モデル | `sentence-transformers/all-MiniLM-L6-v2` + linear head |
| パラメータ | 22M (encoder) + 4 行 logistic regression matrix |
| 推定 CPU 速度 | ~5-10 ms / query |
| 種別 | contrastive few-shot fine-tuning + linear classifier |
| 訓練データ | tsumugi 側で 4 ラベル × 8-100 examples を用意 |
| 新 impl 名 (案) | `SetFitClassifier` (or `MiniLMClassifier`) |

**選定理由**:

- 4 ラベル固定 + 限定訓練データという tsumugi 設定における Pareto
  frontier の最良点 (8-32 examples/class で full fine-tune と同等の
  accuracy)
- MiniLM-L6-v2 (22M) は CPU で 5-10 ms、本コンポーネントで最重要な
  「query routing 経路に hot-path として乗せる」要件を満たす
- 訓練は Python の `setfit` ライブラリで offline に行い、ONNX export して
  Rust 推論のみ実装。tsumugi-core 側に PyTorch 依存は持ち込まない
- 既存 `BertClassifier` (LLM 委譲版) の docstring に「Phase 4+ で MiniLM /
  ModernBERT 統合予定」とあり、本選定はその実現に該当

**留意点**:

- 訓練データを誰がどのように用意するかが要決定事項。tsumugi-core は
  汎用フレームワークなので、ラベル自体 ({Literal, Narrative, Analytical,
  Unknown}) が tsumugi のデフォルトとして適切かも要再検討
- MiniLM-L6-v2 は英語中心 (multilingual ではない)。日本語クエリには
  multilingual sentence-transformers (例: `paraphrase-multilingual-MiniLM-L12-v2`)
  への切替が必要
- 推論時の linear head (4 ラベル × 384 dim) は Rust で手書き可能、または
  `linfa-logistic` で読み込む

### 4.4 Per-Label Binary Event Detection → GLiNER2

| 項目 | 値 |
|---|---|
| モデル | `fastino/gliner2-base-v1` (HF Hub、公開済。large/multi バリアントあり) |
| パラメータ | ~200M (DeBERTa-v3-base ベース) |
| 推定 CPU 速度 | ~200-400 ms / chunk (全ラベル 1 pass) |
| 種別 | span-classification (zero-shot で任意ラベル可) |
| 訓練データ | 実データ 135,698 件 (ニュース/Wikipedia/法律/ArXiv/PubMed) + GPT-4o 合成 118,636 件 |
| 新 impl 名 (案) | `GLiNER2Detector` |

**選定理由**:

- ラベル数 N が増えても **1 forward pass で全ラベル同時処理** できる
  (DeBERTa-MNLI の N pass や SetFit-binary 各ラベル別個推論に対する
  決定的優位)
- zero-shot で動作 (ラベル名テキストを prompt として渡す) のため、
  運用時に新ラベルを追加する際に再訓練不要
- DeBERTa-v3-base (~184M) ベースで CPU で 200-400 ms 級、現実的な
  Tier 2 レイテンシ
- GLiNER (NAACL 2024) の発展系で classification + relation extraction が
  追加され、cascade detector の上流タスクへの拡張余地あり

**留意点**:

- GLiNER2 は 2025 年公開 (arXiv 2507.18546、fastino-ai/GLiNER2)。ONNX
  export パスや Rust 推論サポートは GLiNER v1 系では確認済 (`gliner` の
  ort 推論例)、GLiNER2 はまだ未確認
- span を返すモデルなので「chunk 全体の yes/no」を取るには「span が 1 つ
  でも検出されたら yes」とラップする必要あり (実装は単純)
- 訓練分布は英語中心だが多言語版 (`fastino/gliner2-multi-v1`) が公開済。
  日本語 chunk 適用時は multi 版を採用し、実機品質を要検証
- 訓練データに GPT-4o 合成データが含まれる。OpenAI ToU 上の論点は
  上流 (fastino) が Apache-2.0 で公開している時点で完結している扱い
  (§ 8 ライセンス確認の項参照)

## 5. 実装計画

### 5.1 ML runtime レイヤーの選定

選択肢: `ort` (ONNX Runtime Rust binding) / `candle-transformers` (HF 純
Rust) / `rust-bert` (libtorch 経由) / `tch-rs`。

採用: **`ort` を主、`candle-transformers` を従** とする 2-tier 戦略。

理由:

- `ort` は HF Optimum で ONNX export した weights をそのまま読めるため
  4 モデル全てに対応可能。production-ready で量子化サポートも豊富
- `candle-transformers` は pure Rust で BART/BERT/XLM-R/T5/ModernBERT を
  ネイティブサポート。`ort` でカバーできないモデル (GLiNER2 が export
  困難なケース等) のフォールバック経路として確保
- `rust-bert` は libtorch (>500MB バイナリ) を引きずるため CI 環境への
  影響が大きい。pipeline 既製品の利便性は認めるが採用しない
- `tch-rs` は libtorch 直叩き、production 向けでは `rust-bert` と同様の
  懸念

`tsumugi-core/src/providers/onnx_embedding.rs` に既に `OnnxEmbedding` の
trait skeleton があるため、`ort` 統合の入口は確保済み。本計画の sunk cost
は **`ort` を実依存に追加する 1 回のみ**。

### 5.2 実装順序

依存関係と影響範囲を考慮した順序:

1. **`ort` 統合と `OnnxEmbedding` 実装** — **完了 (2026-04-30)**
   - `tsumugi-core` の Cargo features に `onnx` を追加 (skeleton 既存)
   - `OnnxEmbedding::embed` 本実装: ort 2.0.0-rc.10 + tokenizers 0.21
     + ndarray 0.16 を `onnx` feature gate に追加。`tokio::sync::OnceCell`
     による lazy session init、`spawn_blocking` で blocking inference を
     async ランタイムから分離、mean-pool over attention mask + L2 normalize
     の e5 規約パイプライン。token_type_ids は graph に存在する場合のみ
     zero-tensor を渡す多言語 BERT/XLM-R 互換
   - benches/runner で `tier-0-1` の embedding 選択を env で切替:
     `TSUMUGI_E5_MODEL_PATH` / `TSUMUGI_E5_TOKENIZER_PATH` 両方が
     設定されていれば OnnxEmbedding (default 384-dim)、未設定時は
     MockEmbedding に fallback。`resolve_e5_paths.sh` で HF cache から
     path を抽出し `$GITHUB_ENV` に投入、`bench.yml` は
     `--features network,onnx` でビルド
   - **CR/Oracle/RULER の再計測**: `bench.yml` workflow_dispatch で
     `--ablations tier-0-1` 等を流して取得 (本 PR では実機 dispatch 未実施)
   - **規模**: tsumugi-core +~250 行 / benches/runner +~30 行 /
     scripts +~70 行 / bench.yml +~10 行
2. **LLMLingua-2-mBERT 実装** (`PromptCompressor`)
   - `tsumugi-core/src/compressor/llm_lingua_v2.rs` 新設
   - mBERT-base モデルで per-token classification、threshold で keep/discard
   - 既存 `LlmLinguaCompressor` (LLM 委譲版) をリネーム
     (`LlmDelegationCompressor` 等) して衝突回避、新 impl が
     `LlmLinguaCompressor` を継承
   - benches で `tier-0-1-2` を Truncate から LLMLingua-2-mBERT に置換、
     圧縮率と recall の関係を観測
   - **規模**: 1 PR、~400-600 行
3. **SetFit + MiniLM 実装** (`QueryClassifier`)
   - `tsumugi-core/src/classifier/setfit_classifier.rs` 新設
   - 訓練データ生成 (offline、別リポ or `models/training/` 配下) は本計画の
     scope 外、別タスクとして切り出す
   - 新 impl は ONNX 推論 + 4 行の logistic head 行列を読む
   - **規模**: 1 PR、~300-500 行 + 訓練データ用意
4. **GLiNER2 実装** (`EventDetector`)
   - `tsumugi-core/src/detector/gliner2_detector.rs` 新設
   - HF 公開状況とライセンスの最終確認後に着手
   - span を「any-span → yes」でラップ
   - **規模**: 1 PR、~400-600 行
5. **DistilBART 実装** (`Summarizer`)
   - `tsumugi-core/src/summarizer/distilbart.rs` 新設
   - encoder-decoder の generation loop を ort または candle で実装
   - `HierarchicalSummarizer` の上位レベル用に統合
   - **規模**: 1 PR、~500-800 行 (最も重い)

**順序の根拠**: (1) で `ort` 統合の検証が完了し、その上で軽量な classifier
系 (3, 4) → 圧縮系 (2) → 重い generation 系 (5) の順で段階的に進める。
`ort` 統合段階で問題があれば早期に `candle-transformers` への切替判断が
可能。

### 5.3 既存 LLM 委譲版の扱い

既存 4 つの LLM 委譲実装は **削除せず、選択肢として残す**:

- `LlmSummarizer` → 任意 LLM provider 経由で要約したいユースケース用
- `LlmLinguaCompressor` → LLM 圧縮を試したいベンチマーク用
- `BertClassifier` → 命名は誤解を招くので `LlmRoutedClassifier` 等への
  リネームを検討
- `LLMClassifierDetector` → cascade の最終段として利用継続可能

新 impl 着地後、CLAUDE.md / `tech-architecture.md` の「LLM is isolated
to Tier 2-3」記述を更新し、新 encoder-only 実装が default Tier 2 となり、
LLM 委譲は明示オプトイン経路となる旨を反映する。


## 6. 評価方針

新 impl の評価は既存
[`ci-benchmark-integration-plan.md`](./ci-benchmark-integration-plan.md)
の Tier ablation matrix を拡張する形で実施する。具体的には:

- **`tier-0-1` の embedding 置換**: 現行 `MockEmbedding` (FNV-1a 64-dim,
  deterministic) を `OnnxEmbedding` (multilingual-e5-small) に置換。
  CR / Oracle / RULER で再計測し、tier-0 (BM25 only) との差分が出るか
  実機確認 (現状 7/8 vs 7/8 で差なし → 出るはず)
- **`tier-0-1-2` の compressor 置換**: 現行 `TruncateCompressor` を
  LLMLingua-2-mBERT に置換。
  圧縮率 (`compressed_chars / retrieval_chars`) と recall の関係を
  jsonl に記録、`compression_ratio` metric (既存) で集計
- **新 ablation `tier-1-3-encoder`** (案): Summarizer / QueryClassifier /
  EventDetector を全て encoder-only impl に統一した「LLM 委譲一切なし」
  経路を追加し、`full` (LLM 経路) との accuracy / latency / cost を比較
- **`full` baseline は維持**: LLM 委譲版 (Qwen3.5-4B) の数値を継続的に
  baseline として記録。新 impl がそれを上回ることが目標

`CaseMetric` schema は既存の
`retrieval_latency_ms` / `retrieved_chunks` / `retrieval_chars` /
`compressed_chars` で十分カバー可能 (新フィールド追加不要)。

## 7. 既知のリスク・検討事項

### 7.1 ML runtime 統合の sunk cost と保守性

`ort` を依存に追加すると:

- バイナリサイズ: ONNX Runtime の動的ライブラリ (.so / .dylib / .dll) が
  ~30-50 MB、tsumugi-core の build artifact に直接影響
- CI 環境: Linux x86_64 / macOS arm64 / Windows でそれぞれ ONNX Runtime
  のバージョンが必要、`Swatinem/rust-cache@v2` のキャッシュキー設計を
  見直す必要あり
- バージョンピン: ONNX opset version と Rust crate version がずれると
  silent breakage を起こす可能性。`ort` の minor version を `Cargo.toml`
  で固定し、weights 側 export 時の opset を documentation に記録

緩和策:

- `tsumugi-core` の `onnx` feature flag で全 ONNX 系実装を gate (既に
  `OnnxEmbedding` でその構造あり)
- default features は LLM 委譲版 / 古典統計版のみ、`onnx` を opt-in に
  することで base build はライブラリ依存を持たない

### 7.2 多言語性能 (特に日本語)

採用モデル 4 つの日本語性能リスク:

| モデル | 日本語サポート | リスク |
|---|---|---|
| DistilBART-CNN-6-6 | ❌ (英語ニュース fine-tune) | 日本語要約は別 BART (`ku-nlp/bart-base-japanese` 等) 検討 |
| LLMLingua-2-mBERT | △ (mBERT は 104 言語事前学習、MeetingBank fine-tune は英語) | per-token classifier は語彙レベル、日本語 token への汎化は不透明 |
| MiniLM-L6-v2 | ❌ (英語) | multilingual-MiniLM-L12-v2 (~118M) または paraphrase-multilingual-MiniLM-L12-v2 への切替 |
| GLiNER2 | △ (GLiNER は multilingual 版あり、GLiNER2 は要確認) | 日本語 NER 品質要検証 |

要対応事項:

- 各 trait の new impl は **モデルパス / 言語を constructor で受ける**
  汎用設計とし、日本語向けと英語向けを差し替えられるようにする
- ベンチに日本語データセットが現状ない (LongMemEval / MAB CR / RULER は
  英語中心)。`japanese-bench.yml` は Phase 4-β スコープ (TODO.md) なので
  本計画も英語ベンチでの評価を主、日本語性能は別 phase で取り扱う

### 7.3 訓練データの責務分離

`SetFitClassifier` は tsumugi-core が訓練データを保持できないため
(汎用フレームワークなので domain-specific labels を入れるべきでない)、
以下の責務分離を取る:

- **tsumugi-core**: 推論コード + ONNX weights のロード機構のみ提供
- **訓練データ + label set**: ダウンストリーム (Mayu 等) 側でデータ
  生成 + Python `setfit` で訓練 + ONNX export
- **default impl**: tsumugi-core にはダミー weights を含めず、ユーザに
  weights ファイルパスを構成時に渡してもらう。`new_with_weights(path)`
  pattern

### 7.4 GLiNER2 の成熟度

GLiNER2 は 2025 年公開のため、以下が未確認:

- HF Hub に最終 weights が公開済みか / public license か
- ONNX export の公式サポートがあるか
- Rust 推論 (ort / candle) への適用例があるか

リスクが顕在化した場合のフォールバック:

1. **GLiNER (v1, NAACL 2024)** に降格。classification + relation 機能は
   失うが、span detection としての core 機能は維持
2. **DeBERTa-v3-MNLI zero-shot** に切替。N pass 必要だが weights は
   完全公開、ONNX export 確立済み
3. **Cross-encoder MS-MARCO MiniLM** に切替。22M で軽量、ただし event
   detection 用途への転用は閾値調整必要

### 7.5 既存 LLM 委譲版の API 互換性

`LlmLinguaCompressor` 等を新 impl で置換する際、既存ユーザコード
(`tsumugi-cli` 等) が壊れないかは要確認。新旧で同じ trait (`PromptCompressor`
等) を実装するため、struct 名を変えれば API 互換性は保たれる。

検討事項:

- 既存名の維持: `LlmLinguaCompressor` を encoder-only 版にリネームし、
  LLM 委譲版を `LlmDelegationCompressor` 等に変える (name swap)
- 並列維持: 両方残し、ユーザに選ばせる (recommended)
- 削除: LLM 委譲版を Phase 4 で削除 (non-goal、汎用 framework として
  選択肢を奪うのは過剰)

本計画では **並列維持** を採用する。

## 8. ライセンス確認

採用モデル 4 つについて HF model card / 元論文 / GitHub リポを確認した
結果、**全て Apache License 2.0 で公開されており tsumugi (Apache-2.0)
と整合する**。採用 blocker なし。

| モデル | License | 整合性 | 主な注記 |
|---|---|---|---|
| DistilBART-CNN-6-6 | Apache-2.0 | ○ | 問題なし |
| LLMLingua-2-mBERT | Apache-2.0 | ○ | 元データ MeetingBank が CC BY-NC-ND だが、Microsoft 責任で Apache-2.0 公開済み。tsumugi はデータ自体を再配布せず weights のみ HF Hub 経由で取得 |
| MiniLM-L6-v2 (sentence-transformers) | Apache-2.0 | ○ | 問題なし |
| GLiNER2 (`fastino/gliner2-base-v1`) | Apache-2.0 | ○ | 訓練データに GPT-4o 合成データを含むが上流 (fastino) が Apache-2.0 公開済みで責任完結 |

### 8.1 DistilBART-CNN-6-6

- **HF model card**: <https://huggingface.co/sshleifer/distilbart-cnn-6-6>
  (license タグ `apache-2.0` 明示)
- **再配布**: 商用 OK。NOTICE / LICENSE 表示と「重要な変更点の明記」が
  Apache 2.0 標準条件として必要
- **派生**: copyleft ではないが、再配布 weights には改変有無の表示義務あり
- **訓練データ**: CNN/DailyMail (Apache-2.0) と XSum (BBC 由来、研究用途
  想定)。weights の Apache-2.0 配布責任は sshleifer (元 Hugging Face / Meta)
  が負っている
- **要対応**: `THIRD_PARTY_LICENSES.md` に「Model: sshleifer/distilbart-cnn-6-6,
  License: Apache-2.0, Source: HF Hub, Original: facebook/bart-large-cnn
  distilled by Sam Shleifer」を追記。`download_models.sh` で weights 取得時
  に Apache-2.0 LICENSE と NOTICE を併せて取得する手順を組み込み

### 8.2 LLMLingua-2-mBERT

- **HF model card**: <https://huggingface.co/microsoft/llmlingua-2-bert-base-multilingual-cased-meetingbank>
  (license タグ `apache-2.0` 明示)
- **再配布**: 商用 OK、Apache-2.0 標準
- **訓練データの注意点**: 元データ MeetingBank は **CC BY-NC-ND 4.0**
  (NonCommercial, NoDerivatives) であり、データセット単体では商用利用不可。
  ただし以下の理由で tsumugi として実用上問題なし:
  1. Microsoft Research が weights を Apache-2.0 として公開 (上流 license
     判断責任は Microsoft 側に存在)
  2. 学習済み weights が訓練データの copyright derivative かは法域・解釈
     次第だが、Microsoft が明示的に Apache-2.0 で出している以上、下流
     ユーザは Apache-2.0 に従えば足りる
  3. tsumugi はデータセット自体を再配布せず、weights のみ HF Hub から
     ダウンロードする方針
- **要対応**: `THIRD_PARTY_LICENSES.md` に「Weights: Apache-2.0 (Microsoft)」
  + 「Note: Underlying MeetingBank corpus is CC BY-NC-ND 4.0; tsumugi does
  not redistribute the corpus, only consumes Microsoft-issued weights」
  を併記して透明性を担保。法務的に厳密を要する商用利用者には自前
  fine-tune 可能なよう `PromptCompressor` trait で抽象化済の旨も記載

### 8.3 MiniLM-L6-v2

- **HF model card**: <https://huggingface.co/sentence-transformers/all-MiniLM-L6-v2>
  (license タグ `apache-2.0` 明示)
- **再配布**: 商用 OK
- **派生**: Apache-2.0 標準
- **訓練データ**: Reddit/S2ORC/Stack Exchange/MS MARCO 等 1B+ ペア。各
  データセットの license は雑多だが、sentence-transformers チーム
  (UKP Lab / Hugging Face) が Apache-2.0 で公開済みのため上流責任は完結。
  SetFit base encoder としての利用は完全に想定範囲内
- **要対応**: `THIRD_PARTY_LICENSES.md` に「Model:
  sentence-transformers/all-MiniLM-L6-v2, License: Apache-2.0, Base:
  nreimers/MiniLM-L6-H384-uncased (Microsoft, MIT)」を追記

### 8.4 GLiNER2 (重要訂正)

調査の過程で **HF org 名が当初想定の `knowledgator/` ではなく
`fastino/` であることが判明**。Knowledgator は GLiNER v1 系列 (`urchade/`
および `knowledgator/gliner-*`) のメンテナで、GLiNER2 (Zaratiana et al.,
2025; arXiv:2507.18546) は **fastino 社からのリリース**である。

- **HF model card**: <https://huggingface.co/fastino/gliner2-base-v1>
  (license タグ `apache-2.0` 明示)。バリアント:
  - `fastino/gliner2-base-v1` (default、本計画の採用候補)
  - `fastino/gliner2-large-v1` (高精度)
  - `fastino/gliner2-multi-v1` (多言語、日本語適用時の候補)
- **GitHub**: <https://github.com/fastino-ai/GLiNER2>
- **再配布**: 商用 OK、Apache-2.0 標準
- **訓練データの注意点**: 実データ 135,698 件 (ニュース/Wikipedia/法律/
  ArXiv/PubMed) + **GPT-4o 合成データ 118,636 件**。OpenAI Terms of Use
  (旧版で「OpenAI と競合するモデル開発に Output を使用しない」条項) の
  解釈余地はあるが、上流 (fastino) が Apache-2.0 で公開済みのため
  tsumugi 下流ユーザは fastino の license に従えば足りる
  (DistilBART / LLMLingua-2 と同じ構図)
- **要対応**:
  1. `THIRD_PARTY_LICENSES.md` に「Model: fastino/gliner2-base-v1
     (or large/multi), License: Apache-2.0, Paper: arXiv:2507.18546,
     Org: fastino-ai」を追記
  2. **本計画ドキュメントおよび `docs/TODO.md` の HF org 名を `fastino/`
     に統一** (本 commit で修正済)
  3. `benches/scripts/download_models.sh` で GLiNER2 取得時の HF パスを
     `fastino/gliner2-*` に設定 (実装着手時に対応)

### 8.5 フォールバック判断

- **GLiNER2 weights は HF Hub に公開済**で fastino org に複数バリアントあり、
  ダウンロード数も実用レベル。**フォールバック不要、計画変更不要**
- 仮に将来 fastino が weights を非公開化または license 変更した場合の
  代替経路:
  - **(a) Knowledgator GLiNER v1** (`knowledgator/gliner-bi-large-v2.0`
    等、Apache-2.0): 同じ span-classification API で event detection 用途
    には十分機能
  - **(b) DeBERTa-v3-base + 自前 fine-tune** (GLiNER の base は元々
    DeBERTa-v3-base): MIT license、自由度最大だが fine-tune コスト発生

### 8.6 全体の `THIRD_PARTY_LICENSES.md` 更新方針

実装着手時 (各モデルの impl PR と並行) に以下を `THIRD_PARTY_LICENSES.md`
に追記:

```markdown
## Model Weights (downloaded at run-time)

| Model | License | Source | Notes |
|---|---|---|---|
| DistilBART-CNN-6-6 | Apache-2.0 | HF: sshleifer/distilbart-cnn-6-6 | — |
| LLMLingua-2-mBERT | Apache-2.0 | HF: microsoft/llmlingua-2-bert-base-multilingual-cased-meetingbank | Underlying MeetingBank corpus is CC BY-NC-ND 4.0; weights are Microsoft-issued under Apache-2.0 |
| MiniLM-L6-v2 | Apache-2.0 | HF: sentence-transformers/all-MiniLM-L6-v2 | — |
| GLiNER2 | Apache-2.0 | HF: fastino/gliner2-base-v1 | Training data includes GPT-4o synthesized samples; weights are fastino-issued under Apache-2.0 |
```

## 9. 進捗管理

本計画は `docs/TODO.md` の Phase 5 セクションとして管理される。

各実装ステップ (§ 5.2 の 1-5) は別 PR として切り出し、レビュー粒度を
1 PR ≤ 800 行に保つ。`bench.yml` の workflow_dispatch から各 ablation
を選択的に実行できるよう既存 `--ablations` flag を活用する。

---

*本計画は Phase 4-α Step 3 (Tier ablation matrix) 完了後の 2026-04-30 に
起草。実装着手は別 PR にて。*
