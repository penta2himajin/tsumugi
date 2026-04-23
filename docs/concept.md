# つむぎ — コンセプト資料

## 一言で

**「長期プロジェクトを持つ AI エージェントに一貫性を与える、ドメイン非依存のコンテキスト管理エンジン。」**

---

## プロダクトビジョン

つむぎは、LLM API の前段に配置するコンテキスト管理ミドルウェアである。「長期プロジェクト + 裁定蓄積 + 動的注入 + 階層構造」を必要とする AI エージェントに共通する基盤機能を提供する。

LLM のコンテキストウィンドウ制約を超えた以下を実現する:

- **情報永続化**: セッション / 章 / 開発履歴の全データを失わず保持
- **動的注入**: 現在の文脈に応じて必要な情報だけをコンテキストに展開
- **話題 / シーン / ファイル切替の自動検知**: 適切なタイミングで関連情報を取り出す
- **裁定の版管理 (supersession)**: 「以前は X、今は Y」を明示的に表現
- **未解決事項の追跡**: 伏線・未完了 TODO・秘匿トリガー等

つむぎは**製品ではなく、製品を支えるライブラリ**である。直接ユーザーに売られるのではなく、上位製品に組み込まれて機能を発揮する。

### なぜ汎用として位置付けるか

当初は「創作 AI 向け」として設計したが、つかさ / つづり / つくもの bottom-up 分析を経て、コアの主要抽象はドメイン非依存であることが判明した。creative feature として分離するのは、**Character / Scene / StylePreset / LoreEntry** の 4 つの創作固有抽象のみ。

| 抽象層 | 内容 | 配置 |
|---|---|---|
| ドメイン型 | Chunk, Fact, PendingItem | core (ドメイン非依存) |
| 処理層 | Context Compiler, Retriever | core (ドメイン非依存) |
| trait 群 | Storage / Embedding / LLM / Retriever / RelevanceScorer / EventDetector / PromptCompressor / QueryClassifier / Summarizer | core (ドメイン非依存) |
| 創作拡張 | Character, SceneView, StylePreset, LoreEntry | creative feature |

ドメイン非依存のコアは**「長期プロジェクトで AI の一貫性を保つ」という抽象的な問題を解く**ものであって、その応用が偶然 TRPG・小説・ゲーム開発だっただけ。

---

## 設計原則

### 1. Bottom-up 抽出

つむぎの設計は、つかさ / つづり / つくもで実際に必要になった機能から抽出された。推測で作られた抽象ではない。

- 使われない機能は含まない
- 上位製品の実需に裏打ちされた API のみを提供
- 将来の製品追加時は、その製品の要件から再抽出する

### 2. Turn 非依存のコア

Turn の具体型 (Dialogue / Narration / Check / Edit / EventCommand 等) は製品ごとに大きく異なる。つむぎはこれらの具体型を知らない。

- `Chunk.text: String` — 正規化された表示用テキスト
- `Chunk.items: Vec<serde_json::Value>` — 製品ごとのドメイン型 (serialize 済み)

製品側が Turn 型を定義し、serialize してつむぎに渡す。

### 3. Trait 駆動の拡張性

9 種の trait で主要な差し替え点を提供する。詳細と配置は `tech-architecture.md` を参照。

| trait | 役割 | 主な Tier |
|---|---|---|
| `StorageProvider` | 永続化層 | — (I/O) |
| `EmbeddingProvider` | 埋め込み生成 | Tier 1 |
| `LLMProvider` | LLM 呼び出し | Tier 3 (必要時のみ) |
| `Retriever` | 検索戦略 (BM25 + cosine ハイブリッドがデフォルト) | Tier 0-1 |
| `RelevanceScorer` | 関連度スコア (時間減衰 / 章順 / ファイル近接 / 無減衰) | Tier 0 |
| `EventDetector` | 話題切替 / トリガー検知 | Tier 0-3 (cascade) |
| `QueryClassifier` | クエリ種別判定 (regex / BERT) ★新 | Tier 0-1 |
| `PromptCompressor` | プロンプト圧縮 (LLMLingua-2 等) ★新 | Tier 2 |
| `Summarizer` | 階層的要約生成 ★新 | Tier 2-3 |

### 4. ドメイン非依存コア + 創作拡張の feature flag 分離

- **`tsumugi-core`** (default features): ドメイン非依存の抽象・実装
- **`tsumugi-core` + `creative`**: Character / SceneView / StylePreset / **LoreEntry** を有効化

つかさ・つづりは `features = ["creative"]` で依存、つくもは default features で依存する。将来、別ドメインの拡張 (例: `coding`, `research`) を追加する場合は、同じ feature flag 方式で独立させる。

> **creative の命名について**: 暫定名。現状は創作 3 製品で共有される抽象の集合として機能しているが、意図が曖昧になった時点 (例: 「創作でないが近い抽象を持つドメイン」が出現した場合) で `story` などへ改名する可能性がある。

### 5. 4-tier 処理階層 (LLM 非依存を優先)

コンテキスト管理の各処理を 4 段階の計算コスト層に分けて実装する。**主処理パス (Tier 0-1) は LLM フリーで完結する** のが中核方針。Tier 2-3 はオプショナルな高品質化レイヤーとして扱う。

| Tier | 粒度 | コスト目安 | 例 |
|---|---|---|---|
| Tier 0 | 決定論 | μs 〜 ms | 正規表現、完全一致、BM25 (SQLite FTS5 + lindera)、階層走査、時間窓 |
| Tier 1 | CPU 軽量 | 数 ms | 小型 embedding (MiniLM / BGE-small)、BERT 分類器、IKE 二値化、ModernBERT |
| Tier 2 | GPU 中量 | 数十 ms | LLMLingua-2、embedding top-K 再ランク、軽量 LLM yes/no |
| Tier 3 | LLM フル | 数百 ms〜 | 階層要約生成、最終裁定抽出、最終生成 |

帰結として、最小構成ユーザー (Apple Silicon 8GB / 統合 GPU 環境) でも Tier 0-1 のみで主要機能が動作する。Tier 2-3 はアップグレード時に自然に恩恵を受ける構造。

### 6. Alloy による形式仕様 + oxidtr 生成

ドメインモデルは Alloy で記述し、oxidtr で Rust / TypeScript の型・テスト・不変条件を自動生成する。

Alloy に書くもの:
- 型構造 (newtype ID、enum、record フィールド)
- 参照整合性 (dangling ID 防止)
- 主要な不変条件 (階層非循環、supersession 非循環、pending items の寿命)

Rust に書くもの:
- 業務ロジック
- 検索・スコアリング・プロンプト組み立て

### 7. 具体最適化を維持

「汎用 = 何にでも使えるが何にも最適化されない」の罠を避ける。つむぎは:

- 3 製品 (つかさ / つづり / つくも) の具体要件に最適化しつつ
- 副次的に汎用性を持つ

仮説的な応用 (研究 / 教育 / 業務等) は可能性として示すが、**実需検証なしに設計の根拠にはしない**。

---

## 消費者 (上位製品)

### 現在の実需者

| 製品 | 用途 | tsumugi 利用度 | feature |
|---|---|---|---|
| [つかさ](https://github.com/penta2himajin/tsukasa) | TRPG GM 補助 | フル活用 | `creative` |
| [つづり](https://github.com/penta2himajin/tsuzuri) | 小説執筆補助 | 7 割活用 | `creative` |
| [つくも](https://github.com/penta2himajin/tsukumo) | RPGツクール特化 | 5-6 割活用 | core のみ |

### 仮説的応用 (将来検討、製品設計の根拠にはしない)

- **汎用コーディングエージェント**: つくものツクール特化を外せば、任意プロジェクトの裁定・パターン・却下履歴管理に転用可能。Claude Code の CLAUDE.md 方式の限界 (静的・200 行制約・supersession 不能) を解決する基盤になり得る
- **研究 / 学習補助 AI**: 読んだ論文、考察中の問い、参考文献を永続化
- **個人秘書エージェント**: ユーザーの好み、進行中プロジェクト、継続タスクを記憶
- **教育 / 指導 AI**: 学習者の進捗、つまずき、既習範囲を追跡
- **業務 AI**: 案件・裁定履歴・未解決事項の管理
- **チャットエージェントの長期記憶層**: mem0 / LangMem 相当のポジション

これらは**仮説**であり、実需検証が済むまでは tsumugi の API 設計に影響を与えない。

### 兄弟プロジェクト

- [chatstream](https://github.com/penta2himajin/chatstream): 音声 AI デバイス向けの同型ミドルウェア。共通する設計思想 (階層、trait 抽象、全データ保持) を共有する独立実装

---

## 他ライブラリとの位置付け

### 同じ課題を扱うライブラリ

| ライブラリ | 特徴 | つむぎとの差 |
|---|---|---|
| mem0 | 対話エージェント向け長期記憶、OSS + SaaS | 創作ドメイン (Character, Scene) を持たない、階層構造が浅い |
| LangMem (LangChain) | 意味記憶と連想記憶の管理 | 製品特化機能が少ない、汎用チャット向け |
| Zep | Knowledge Graph ベース、有償 SaaS | クラウド、Graph 形式に絞られる |
| LangChain Memory | Buffer / Summary / Hybrid | 会話履歴中心、階層が浅い、creative ドメイン概念なし |
| Claude Code の CLAUDE.md + Auto Memory | 静的 + 暫定的な動的記憶 | supersession なし、階層なし、ドメイン特化機能なし |
| SillyTavern Lorebook | キーワードトリガーで静的情報を挿入 | 静的辞書、動的階層なし、会話ドメイン特化 |

### つむぎの固有ポジション

- **階層的 + 動的**: 静的 Wiki / Lorebook ではなく、現在文脈に応じた動的注入
- **supersession 対応**: 裁定の版管理を明示的に表現
- **trait 駆動の拡張性**: 各 trait が差し替え可能、ローカル / クラウド両対応
- **LLM 非依存の tier 構造**: Tier 0-1 のみで主処理パスが完結
- **日本語対応前提**: lindera による BM25、日本語 LLM 前提の設計

### chatstream との関係

両者は**兄弟ミドルウェア**であり、同じ設計思想を共有する:

- 階層的コンテキスト管理
- trait 駆動の拡張性
- oxidtr + Alloy の開発フロー
- 全データ保持 (損失なき要約)

違い:
- chatstream は音声 AI デバイス向け (レイテンシ厳しい、選択肢少ない UI 制約)
- tsumugi はテキスト AI 向け (レイテンシ緩い、UI 制約なし)

将来的に `EventDetector` trait 等を共通化する道筋はあるが、当面は各自独立実装。

---

## 設計決定の記録

つかさ / つづり / つくもの bottom-up 分析と、`docs/research/context-management-survey.md` の既存研究レビューから導出された論点の決着:

| 論点 | 決定 | 理由 |
|---|---|---|
| Turn 表現 | Turn 非依存、`Chunk.items: Vec<Value>` に製品が serialize | ドメイン Turn 型が製品ごとに根本的に違う |
| Character sheet | `sheet: Map<String, Value>` の自由形式、共通フィールドのみ型付け | CoC SAN vs 小説 backstory は共通化できない |
| Scene と Chunk | Scene = Chunk + metadata タグ、階層は単一機構 | 両創作製品とも 3 層の階層を持ち、Scene を別 entity にすると重複 |
| **LoreEntry の配置** | **creative feature に移設** (core から外す) | Lorebook 由来の keyword トリガー辞書であり、使用パターンが創作ドメインに属する。core は汎用コンテキストエンジンとして純化 |
| LoreEntry embedding | `EmbeddingStore` trait、デフォルトは長さで判定 | 短い固有名詞は不要、長い世界観説明は必要 |
| RAG hybrid | BM25 + cosine、日本語は lindera | 両方の強みが必要、`Retriever` trait で差し替え |
| RelevanceScorer | trait 化、`TemporalDecay` / `ChapterOrder` / `FileProximity` / `NoDecay` 同梱 | つかさ・つづり・つくもで減衰モデルが質的に異なる |
| EventDetector | trait 化、3 段カスケードを `CascadeDetector` で chain | つかさのトリガー・つづりの伏線活性化・つくものパターン違反検知の共通構造 |
| Character / Scene / StylePreset の配置 | feature flag `creative` で分離、`tsumugi-core` default features から除外 | つくもが使わない、汎用性を保つためコアに残さない |
| **creative 命名** | **暫定名** (将来 `story` 等への改名可能性を明記) | 現状は創作 3 製品に合致、意図が広がれば見直す |
| **階層要約の表現** | **既存 Chunk の拡張** (新規 `HierarchicalSummary` 型は作らない) | RAPTOR のツリー構造は parent/children で表現可能、抽象数を増やさない |
| **SummaryLevel 型** | **`u32` (0 = Raw、正数が抽象度)** | 具体ラベル (Scene/Chapter/Arc 等) はドメイン跨ぎで意味が変わる、数値は汎用 |
| **SummaryMethod enum** | `LlmFull` / `LlmLingua2` / `SelectiveContext` / `ExtractiveBM25` / `UserManual` | 要約手法の選択を明示、Tier と直接対応 |
| **SourceLocation の表現** | **trait 化** (core に `FileSourceLocation` 標準実装を同梱) | ファイルパス / URI / "session/3#scene2" 等の多様な表現を製品が差し替え可能、core の汎用性を保つ |
| **4-tier 処理階層** | 設計原則 #5 に明文化、主処理パスは Tier 0-1 完結 | LLM 非依存で動く最小構成ユーザー対応、Tier 2-3 はアップグレード恩恵 |
| **追加 3 trait** | `QueryClassifier` / `PromptCompressor` / `Summarizer` を trait 化 | 調査書 §8 の提案を統合、段階的実装で後方互換を保つ |
| chatstream 統合 | 当面独立、抽象を互換に保つ | 早期統合は設計を縛る、将来統合の余地は残す |
| Alloy 粒度 | 中間: 型 + 参照整合 + 主要ライフサイクル | 完全制約は重い、型のみでは価値薄い |
| 配布形態 | core + cli + ts の 3 クレート、creative は core の feature | シンプルな構造を維持 |

---

## 現在のフェーズ

Phase 0: 設計固め (bottom-up 抽出完了、汎用化ポジション確定、調査書統合完了、Alloy モデル初版着手中)

次の入り口:
- Alloy モデル `models/tsumugi-core.als` と `models/tsumugi-creative.als` の初版作成
- oxidtr 生成パイプライン動作確認 (core / creative 分離生成)
- `tsumugi-core` 内 `creative` feature flag の分離実装
- trait 定義 (9 種) と in-memory 実装と結合テスト

---

*最終更新: 2026-04-23*
