# コンテキスト管理システムに関する研究・実装サーベイ

このドキュメントは、tsumugi の設計方針を既存研究と照らし合わせるために 2026-04-22 に行った調査の記録。「長期プロジェクトの動的コンテキスト管理」の既存手法を整理し、tsumugi に取り込むべき要素を抽出する目的。

> **2026-04-23 更新**: 調査内容を `concept.md` / `tech-architecture.md` / `TODO.md` に統合済み。各節の ✅ マークは統合箇所への対応。

## 1. 調査範囲と前提

扱う問題領域:

- LLM のコンテキストウィンドウ制約下で、長期プロジェクトの情報をどう管理・投入するか
- ドメイン非依存の「記憶」の構造と運用
- LLM を使わない / 軽量化する自律的コンテキスト管理

扱わない領域:

- LLM 本体の長文脈対応技術 (1M コンテキスト対応モデル等)
- モデル圧縮・量子化 (別サーベイ: `2026-04-model-landscape.md`)
- RAG の個別要素技術 (chunking, embedding model 選定等)

## 2. 前提整理: 「会話履歴全保持」と「選択的投入」は別層の話

既存手法を調べる過程で、議論が 2 つの独立した層に分かれていることを確認した。

### 層 1: ストレージ (何を保存するか)

| 派閥 | 代表例 | 特徴 |
|---|---|---|
| 全履歴保持派 | MemGPT, Letta, mem0, Zep | 全データを残す、検索で必要分を取り出す |
| 要約圧縮派 | MemoryBank, ReadAgent, LangChain ConversationSummaryBuffer | 古い履歴は要約で置き換える |
| ハイブリッド | RAPTOR, MemoryOS | 生データ + 階層的要約を併用 |

### 層 2: コンテキスト投入 (何を LLM に渡すか)

| 派閥 | 代表例 |
|---|---|
| 全量投入 | LangChain ConversationBufferMemory 等の古い naive 手法のみ |
| 選択的投入 | **現代の真面目な手法はほぼ全部**: Mem0, Letta, MemGPT, RAPTOR, Zep, つむぎ |

### 既存研究のコンセンサス

「全履歴を毎回 LLM に渡せば渡すほど良い」という仮説は**実験的に否定されている**。主要な裏付け:

- **MEM1 (2025-06)**: メモリと推論の相乗で効率化、全履歴投入の非効率性を実証
- **AgentFold (2025-10)**: Proactive Context Management、必要前に圧縮
- **Compress to Impress (2024-02)**: 現実会話では圧縮メモリが圧倒的に有利
- **Context Rot 研究 (Anthropic / Chroma 等)**: 文脈が長いほど注意散漫
- **Microsoft + Salesforce (2024)**: 断片的文脈を多ターンで与えると **LLM 性能が 39% 低下** (context clash)

→ tsumugi の「全データ保存 + 選択的投入」は既存研究の主流と整合。

### 選択的投入の副作用 (設計時に注意すべき)

| 副作用 | 内容 | tsumugi での対策余地 |
|---|---|---|
| Context clash | 断片同士の矛盾で LLM 混乱 | Fact supersession で古い情報を明示除外 |
| 投入不足 | 重要文脈を取り逃す | RelevanceScorer の調整、階層的要約 |
| supersession 見逃し | 古い情報が活性化 | Fact の supersession 関係をチェック |
| 話題分断 | 物語性喪失 | Chunk の連続性保持、階層的要約で抽象層を補完 |

## 3. 既存手法の分類

### 3.1 投入選択の戦略 (9 パターン)

| 戦略 | 代表実装 | tsumugi の現状 | 反映 |
|---|---|---|---|
| A. 直近性 (recency) | ConversationBufferWindowMemory | TemporalDecayScorer / ChapterOrderScorer で対応済 | ✅ |
| B. 要約 + 直近ハイブリッド | ConversationSummaryBufferMemory | ResidentLayer.recent_turns + current_scene_summary | ✅ |
| C. 類似検索 (RAG) | Mem0, Zep, 大半の RAG 実装 | HybridRetriever (BM25 + cosine) 設計済 | ✅ |
| D. 構造化抽出 | Mem0 facts, Zep entities | Fact 抽象として設計済 | ✅ |
| E. 階層的要約 | **RAPTOR** | **Chunk 拡張 + Summarizer trait で採用** (Phase 2) | ✅ 採用 |
| F. 自己編集メモリ | MemGPT, Letta | 未採用、tsumugi 哲学との整合性要検討 | 見送り |
| G. 役割中心文脈化 | GAM (2026) | 部分的 (ダウンストリームでドメイン抽象を実装する形で対応) | 部分的 |
| H. 話題切替圧縮 | 3 段カスケード設計 | EventDetector で設計済 | ✅ |
| I. RL によるメモリ最適化 | MemAgent (2025-07), MemRL | 見送り (実装コスト高) | 見送り |

### 3.2 ストレージ構造の主要パラダイム

**OS 風階層メモリ (MemGPT / Letta)**:
- Core Memory (context window 内、RAM 相当)
- Recall Memory (会話履歴検索、SSD 相当)
- Archival Memory (長期、HDD 相当)
- LLM 自身が tool call でメモリを編集
- 制御性高いが LLM 呼び出しコストが大きい

**時間的知識グラフ (Zep)**:
- エンティティと関係を graph で保持
- 時間軸と意味類似のハイブリッド検索
- 複雑なエージェント workflow に向く

**階層要約ツリー (RAPTOR)**:
- チャンクを再帰的にクラスタリング + 要約
- ツリーの異なる深さから取得
- QuALITY ベンチマークで GPT-4 比 **20% 性能向上**
- RAGFlow v0.6.0 で採用

**役割中心グラフ (GAM, 2026)**:
- 発話者ごとに narrative thread を disentangle
- 多人数対話 (TV 脚本など) で効果大
- Qwen 2.5-7B で Mem0 を The Office subset で 11.60 vs 9.11 で上回る

### 3.3 production memory frameworks のベンチ

LoCoMo ベンチマーク (長期会話記憶) でのスコア:

| フレームワーク | LoCoMo スコア |
|---|---|
| SuperLocalMemory V3 Mode C | 87.7% |
| Letta (MemGPT 後継) | ~83.2% |
| Mem0 | ~76% |
| EverMemOS / MemMachine / Hindsight | 公開されていない |

標準 3 メモリスコープ (episodic / semantic / procedural) は業界合意。

## 4. LLM 非依存 / 軽量化のアプローチ

Kenya の関心領域。「LLM 自身にメモリ管理させるのは非効率」という直感と一致する研究が 2025-2026 に急速に整備されている。

### 4.1 ゼロ LLM 推論 (完全軽量)

**SelRoute (2026)** — tsumugi のニーズに最も近い ✅ **採用** (`RegexClassifier` として Phase 1、`BertClassifier` を Phase 3):

- クエリ時に **LLM 推論ゼロ、GPU 不要**
- 正規表現ベース分類器で 83% の routing 精度
- 6 種のクエリタイプ (時間 / 因果 / 比較 / 事実 / 前提 / マルチホップ) で検索戦略切替
- SQLite FTS5 + BGE-small-en (33M) / MiniLM-L6-v2 (22M) 小型埋め込み
- LoCoMo / MSDialog / QReCC / PerLTQA 等 8 ベンチマーク × 62,000+ インスタンスで汎化
- Recall@5 = 0.689、均一ベースラインを上回る
- RECOR (reasoning-intensive) では Recall@5 = 0.149 と弱く、限界を明示

**古典的 IR の再評価** ✅ 採用済 (lindera + BM25):

- BM25 + FTS5 (SQLite 組み込み) だけで多くのタスクで LLM retriever に近い性能
- 最小構成ユーザーのハードウェア (統合 GPU、MacBook Air 等) で動く唯一の選択肢
- tsumugi は lindera + BM25 を既に採用済

### 4.2 小型 encoder による token-level 分類

**LLMLingua-2 (ACL 2024)** — プロンプト圧縮を token 分類問題として定式化 ✅ **採用** (`LlmLinguaCompressor` として Phase 2):

- XLM-RoBERTa-large / mBERT レベルの encoder のみ、数百 M パラメータ
- GPT-4 から distillation した教師データで訓練
- 3x-6x 高速、20x 圧縮でも性能維持
- 多言語対応 (XLM-RoBERTa ベース)
- tsumugi の Context Compiler の圧縮段に直接適用可能

**LLMLingua オリジナル (EMNLP 2023)**:

- GPT-2 small や Llama-7B の small LM で perplexity ベース重要度推定
- Budget Controller + Iterative Token Compression + Alignment の 3 段構成
- LLMLingua-2 より古いがシンプル、第一バージョンに適する可能性

**Pref-LSTM (2025)**:

- BERT 分類器でユーザー嗜好を識別 + LSTM で soft-prompt 注入
- フリーズ LLM に追加、fine-tune 不要
- 「何を記憶するかの分類」に BERT、「取り出し」に LSTM という役割分担

**Selective Context (EMNLP 2023)** ✅ 採用 (`SelectiveContextCompressor` として Phase 2):

- 情報エントロピーでトークン冗長性を判定、刈り取り
- encoder forward のみ、LLM 生成不要
- 最もシンプル、実装容易

### 4.3 埋め込み特殊化 (学習不要 / 軽量変換)

**IKE: Isolation Kernel Embedding (2026-01)** ✅ **採用** (`IkeEmbeddingProvider` として Phase 3):

- **学習不要**で LLM embedding を二値 embedding に変換
- bitwise 計算で高速検索、メモリフットプリント極小
- Matryoshka Representation Learning (MRL) / Contrastive Sparse Representation (CSR) よりロバスト
- 最小構成ユーザー向きの高速類似検索として有力

**fastc**:

- Logistic Regression / Nearest Centroid + LLM embedding
- 学習不要、CPU 動作、並列実行可能
- 軽量 embedding モデル (tinyroberta-6l-768d, 22M) を使用
- 軽量に話題切替・トリガー検知を回す用途に適する

**ModernBERT (2024)** ✅ 検討中 (`BertClassifier` の基盤候補、Phase 3):

- BERT 大幅改修版 (RoPE、alternating attention、hardware 最適化)
- 8192 トークン context、前世代 encoder の 2-4x 高速
- 分類 / 検索 / ルーティング用途に理想的

### 4.4 決定論的 / 統計ベース

**DAM: Decision-theoretic Agent Memory (2025-12)** → Phase 4 実験項目:

- メモリ管理を逐次決定問題として定式化
- Read Policy と Write Policy に分離
- Write は Value Function + uncertainty 推定器で決定
- LLM を使わず価値関数と不確実性で決定
- Heuristic を超えつつ LLM 呼び出しも避ける
- 研究はこれから成熟段階

**MACLA (2025-12)**:

- Bayesian posterior で procedure の信頼性追跡
- Expected-utility scoring で行動選択
- 2,851 trajectories を 187 procedures に 15:1 圧縮
- メモリ構築 56 秒 = state-of-the-art LLM 学習の 2,800 倍速
- ALFWorld unseen で 90.3% (+3.1% generalization)
- ドメインは LLM エージェント手続き記憶寄り、つむぎの直接マッピングは要検討

### 4.5 Acon: Agent Context Optimization (2025-10)

- LLM 圧縮器を小型モデルに distillation、追加モジュール overhead 削減
- AppWorld / OfficeBench / Multi-objective QA で memory 26-54% 削減
- distillation 後も 95% 以上の精度維持
- 勾配不要、API ベースモデルにも適用可

## 5. 階層的要約 (RAPTOR 流) の詳細

Kenya が取り込みを希望した手法。✅ **採用** (Option A、既存 Chunk 拡張)。

### RAPTOR の構造

- チャンクを embedding 化
- UMAP で次元削減 + GMM でソフトクラスタリング
- 各クラスタを LLM で要約
- 要約を新たなチャンクとして再帰的にクラスタリング
- ツリー構造 (bottom-up)
- 検索時は複数階層から取得、抽象度の異なる情報を統合

### tsumugi への適用 (決着)

以下の Option A (既存 Chunk の拡張) を採用。新たな `HierarchicalSummary` 型は作らない。

1. 親 Chunk が子 Chunk 群の**再帰的要約**として機能するよう意味付けを明示化
2. 階層レベルの明示 (`summary_level: u32`、0 = Raw、正数が抽象度)
3. 階層間の要約伝播タイミング (差分更新)
4. ユーザー編集済み要約の保護 (`edited_by_user` / `auto_update_locked` フラグ)

### 採用された Chunk 拡張 (`tech-architecture.md` 反映済み)

```rust
pub struct Chunk {
    // 既存フィールド...
    pub summary_level: u32,                  // 0 = Raw (葉)、正数が高抽象度 ※ u32 化で採用
    pub summary_method: SummaryMethod,       // LlmFull / LlmLingua2 / Extractive / UserManual / None
    pub edited_by_user: bool,                // ユーザー編集済み
    pub auto_update_locked: bool,            // 自動更新を防ぐフラグ
}
```

> **SummaryLevel が u32 になった理由**: 元案は enum (`Raw / Scene / Chapter / Arc / Project`) だったが、具体ラベルは創作ドメインに引っ張られすぎてコーディング等で使えない。汎用コンテキストエンジンとして純化するため u32 (0 = Raw、正数が抽象度) に変更。

親子関係は既存の parent/children で表現、要約は既存の summary フィールド。Alloy 不変条件:

- `summary_level == 0` ⇒ `items` 非空 (生データの葉)
- `summary_level > 0` ⇒ `children` 非空 (要約ノードは子を持つ)
- 親子間で親の `summary_level` > 子の `summary_level`

## 6. tsumugi への活用方針 (優先度付き)

### 6.1 最優先 (MVP で採用推奨) ✅ すべて採用

| 手法 | 採用理由 | 実装コスト | 反映先 |
|---|---|---|---|
| SelRoute 流クエリルーティング | 正規表現 + 小型 BERT、GPU 不要、最小構成で動く | 低 | Phase 1: `RegexClassifier` / Phase 3: `BertClassifier` |
| SQLite FTS5 + lindera BM25 | 既に計画済み、最小構成基盤 | 済 | Phase 1 採用済 |
| MiniLM / BGE-small 埋め込み | CPU 動作、第一選択 | 低 | Phase 1 `EmbeddingProvider` の既定候補 |
| EventDetector cascade (3 段) | LLM 呼び出しを最小化、既に設計済 | 済 | Phase 1 採用済 |

### 6.2 推奨 (Phase 1 後半〜 Phase 2) ✅ すべて採用済

| 手法 | 採用理由 | 実装コスト | 反映先 |
|---|---|---|---|
| LLMLingua-2 | Context Compiler の圧縮段、3-6x 高速 | 中 | Phase 2: `LlmLinguaCompressor` |
| IKE 二値 embedding | 最小構成で高速類似検索 | 中 | Phase 3: `IkeEmbeddingProvider` |
| fastc / ModernBERT 分類器 | Fact 抽出 / pending item 検出の軽量化 | 中 | Phase 3: `BertClassifier` (ModernBERT ベース) |
| 階層的要約 (RAPTOR 流) | 既存 Chunk 構造の拡張で表現可 | 中 | Phase 2: `HierarchicalSummarizer` |
| HierarchicalSummary 編集 UI | ユーザー調整可能性 | 中 | Phase 2: `edited_by_user` / `auto_update_locked` UX |

### 6.3 興味深いが実験段階 (将来検討)

| 手法 | コメント | 反映 |
|---|---|---|
| DAM Decision-theoretic Framework | 枠組み成熟待ち | Phase 4 実験項目 |
| MACLA Bayesian selection | 手続き記憶向き、長期プロジェクト系での適合性要検証 | 未計画 |
| Acon Context Optimization | Agent 系、つむぎへの直接マッピング要検討 | 未計画 |
| GAM 役割中心文脈化 | ダウンストリーム側でドメイン抽象を実装する形で対応する余地 | 保留 |

### 6.4 見送り

| 手法 | 見送り理由 |
|---|---|
| RL メモリ学習 (MemRL / MemAgent) | 訓練コスト高、効果未確認 |
| LLM 自己編集メモリ (MemGPT / Letta) | LLM 呼び出し多く非効率、Kenya の懸念通り |
| naive 全履歴投入 | Context Rot / Context Clash で性能劣化 |

## 7. tsumugi の処理階層 (4 tier) の提案 ✅ 採用

Kenya のニーズ (高速かつ効率的、LLM 非依存) を整理すると、以下の階層が自然。**`concept.md` 設計原則 #5 および `tech-architecture.md` 「4-tier 処理階層」に明文化済み。**

### Tier 0: ゼロコスト (即応、数 μs 〜 数 ms)

- 正規表現ルール (SelRoute 流)
- 完全一致キーワード (cascade Stage 1)
- BM25 (SQLite FTS5 / lindera)
- 決定論的ルール (直近 N、章順距離、ファイル近接)

### Tier 1: 低コスト、CPU で数 ms

- 小型 embedding (MiniLM 22M / BGE-small 33M)
- 二値化 embedding (IKE)
- BERT 分類器 (LLMLingua-2 / fastc)
- ModernBERT ルーティング

### Tier 2: 中コスト、GPU あれば数十 ms

- 軽量 encoder による token 重要度付け (LLMLingua-2)
- embedding 類似度 top-K 取得
- 軽量 LLM による yes/no 判定 (cascade Stage 3)

### Tier 3: 高コスト、LLM フル呼び出し

- 裁定抽出の最終選定
- 階層的要約生成
- 最終回答生成

### 階層適用の現状

現状の EventDetector cascade は既にこの 4 層構造を持つ。設計統合後は以下のとおり:

- **Retriever**: Tier 0-1 (LLM 不要、変わらず)
- **Context Compiler**: Tier 0-1 で大半を処理、Tier 2 で `PromptCompressor` 任意介入、Tier 3 は製品側最終生成
- **Summarizer**: `SummaryMethod` で Tier 1 (ExtractiveBM25) / Tier 2 (LLMLingua-2, Selective Context) / Tier 3 (LLM Full) を明示的に切替
- **Fact 抽出**: 製品側の `EventDetector` で Tier 1 (BERT 分類器) 候補抽出、必要なら Tier 3 (LLM) で最終整形
- **LLMProvider**: core の主処理パス (Tier 0-1) からは呼ばれない。製品の最終生成と Tier 2-3 trait 実装の内部利用のみ

## 8. 設計影響のメモ ✅ すべて統合済 (`tech-architecture.md` 反映)

### 追加 trait (統合済)

```rust
pub trait PromptCompressor: Send + Sync {
    // LLMLingua-2 / Selective Context / truncation 等
    async fn compress(&self, text: &str, target_ratio: f32) -> Result<String>;
}

pub trait QueryClassifier: Send + Sync {
    // SelRoute 流、正規表現ベース / BERT 分類器
    async fn classify(&self, query: &str) -> Result<QueryType>;
}

pub trait Summarizer: Send + Sync {
    // 階層的要約生成、method 指定可
    async fn summarize(
        &self, chunks: &[Chunk], target_level: u32
    ) -> Result<SummarizerOutput>;
}

pub enum SummaryMethod {
    LlmFull,             // Tier 3
    LlmLingua2,          // Tier 2
    SelectiveContext,    // Tier 2
    ExtractiveBM25,      // Tier 1
    UserManual,           // 人間が書いた
    None,                // 未生成
}

// ※ SummaryLevel は当初 enum 案だったが u32 (0 = Raw) に変更。
//    具体ラベル (Scene/Chapter/Arc) はドメイン跨ぎで意味が変わるため。
```

### 段階的実装方針 (統合済、`TODO.md` に反映)

- **Phase 1**: `RegexClassifier` (正規表現), HybridRetriever (BM25 + cosine), Chunk に summary_level / summary_method / edited_by_user / auto_update_locked 追加、`TruncateCompressor`, `ExtractiveBM25Summarizer`
- **Phase 2**: `LlmLinguaCompressor`, `SelectiveContextCompressor`, `LlmSummarizer`, `HierarchicalSummarizer`
- **Phase 3**: `BertClassifier` (MiniLM / ModernBERT), `IkeEmbeddingProvider` (二値化)
- **Phase 4**: DAM Decision-theoretic 実験

## 9. tsumugi の独自性 (既存研究と比較)

整理を通じて、既存の memory framework 群と tsumugi の差分が明確になった:

- **ドメイン非依存の汎用フレームワーク**: Mem0 / Zep / MemGPT は汎用チャット前提、tsumugi は階層 + supersession + Tier 構造を持つ汎用メモリレイヤーとして抽象化、ドメイン固有の型はダウンストリームに委ねる
- **Rust + Alloy + oxidtr**: 主流は Python、型安全性と形式検証の価値
- **supersession を一級市民**: 他手法は要約による暗黙更新が多い、tsumugi は版管理を明示
- **ローカル完結前提**: Mem0 / Zep はクラウド中心、Letta は重い
- **複数ドメインで同時検証**: 小説 / TRPG / コード開発の 3 ドメインの bottom-up 抽出
- **LLM 非依存の tier 構造**: 「非効率性回避」哲学を設計に取り込む (主処理パスが Tier 0-1 完結)

## 10. 要検証項目 (MVP Phase 0 〜 1)

> `TODO.md` §要実機検証項目 に集約済。

1. SelRoute 方式の**日本語対応**: 論文は英語データセット中心、正規表現分類器の日本語適性要検証
2. LLMLingua-2 の**日本語性能**: XLM-RoBERTa 多言語対応だが、実タスク日本語品質は要検証
3. IKE 二値化 embedding の**retrieval 精度**: 主張される軽微な精度低下の実証
4. **階層的要約の更新タイミング**: 差分更新の戦略設計
5. **ユーザー編集済み要約と自動更新の競合**: locked フラグ UX 設計
6. **context clash (Microsoft/Salesforce 39% 低下) の創作系での再現**: 小説 / TRPG で同じ現象が出るか

## 11. 未確定の大論点

> 2026-04-23 時点で決着済みを ✅ でマーク。未決着のものは `TODO.md` §未確定の大論点 に集約。

- ✅ **階層的要約の導入を既存 Chunk 拡張で行うか、新規 HierarchicalSummary 型にするか** → **Chunk 拡張** (Option A 採用、`tech-architecture.md` 反映)
- ✅ **PromptCompressor / QueryClassifier / Summarizer trait の追加タイミング** → **Phase 1 で trait 定義と最小実装、Phase 2 で拡張実装** (`TODO.md` 反映)
- LLM 自己編集メモリの是非 (制御性 vs 自律性のトレードオフ) — **現状見送り**、判断は `TODO.md` 未確定論点へ
- MemoRAG 流のグローバル要約 (本体別途取得の 2 段階) の実装コスト — 未計画
- Event-Centric Memory (MAGMA, EverMemOS) のグラフ構造と Chunk 構造の両立 — `TODO.md` 未確定論点へ

## メタデータ

- 調査日: 2026-04-22
- 設計統合日: 2026-04-23
- 主要参考リスト:
  - [Agent-Memory-Paper-List (Shichun-Liu)](https://github.com/Shichun-Liu/Agent-Memory-Paper-List)
  - [Awesome-Efficient-Agents (yxf203)](https://github.com/yxf203/Awesome-Efficient-Agents)
  - [Awesome-LLM-Compression (HuangOwen)](https://github.com/HuangOwen/Awesome-LLM-Compression)
- 想定次回レビュー: 2026-10 前後 (半年ごと、モデルランドスケープと併せて)
- 注意: この領域は急速に発展中、MVP 実装直前にも再確認推奨
