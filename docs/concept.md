# つむぎ — コンセプト資料

## 一言で

**「長期プロジェクトを持つ AI エージェントに一貫性を与える、汎用メモリレイヤーフレームワーク。」**

---

## プロダクトビジョン

つむぎは、LLM API の前段に配置するメモリレイヤーミドルウェアである。「長期プロジェクト + 裁定蓄積 + 動的注入 + 階層構造」を必要とする AI エージェントに共通する基盤機能を提供する。

LLM のコンテキストウィンドウ制約を超えた以下を実現する:

- **情報永続化**: セッション / 章 / 開発履歴の全データを失わず保持
- **動的注入**: 現在の文脈に応じて必要な情報だけをコンテキストに展開
- **話題 / シーン / ファイル切替の自動検知**: 適切なタイミングで関連情報を取り出す
- **裁定の版管理 (supersession)**: 「以前は X、今は Y」を明示的に表現
- **未解決事項の追跡**: 伏線・未完了 TODO・秘匿トリガー等

つむぎは**製品ではなく、製品を支えるライブラリ**である。直接ユーザーに売られるのではなく、上位アプリケーションに組み込まれて機能を発揮する。

### ドメイン非依存の汎用フレームワーク

つむぎは「長期プロジェクトで AI の一貫性を保つ」という抽象的な問題を解く。応用先 (TRPG GM 補助 / 小説執筆 / コーディングエージェント / 研究補助 / 業務 AI 等) に依らず、共通する抽象を提供する:

| 抽象層 | 内容 |
|---|---|
| ドメイン型 | `Chunk` / `Fact` / `PendingItem` / `SourceLocation` |
| 処理層 | Context Compiler、Retriever、Scorer、EventDetector |
| trait 群 | `StorageProvider` / `EmbeddingProvider` / `LLMProvider` / `Retriever` / `RelevanceScorer` / `EventDetector` / `QueryClassifier` / `PromptCompressor` / `Summarizer` |

ドメイン固有の型 (登場人物 / シーン記述 / 世界観辞典等) は**ダウンストリームのアプリケーション側で実装**する。tsumugi 本体は汎用フレームワークとして純化する。

---

## 設計原則

### 1. Bottom-up 抽出

つむぎの抽象は実需アプリケーションでの bottom-up 分析から導出された。推測で作られた抽象ではない。

- 使われない機能は含まない
- 実需に裏打ちされた API のみを提供
- 新しいユースケースが出れば再抽出する

### 2. Turn 非依存のコア

Turn の具体型 (Dialogue / Narration / Check / Edit / EventCommand 等) はアプリケーションごとに大きく異なる。つむぎはこれらの具体型を知らない。

- `Chunk.text: String` — 正規化された表示用テキスト
- `Chunk.items: Vec<serde_json::Value>` — アプリケーションごとのドメイン型 (serialize 済み)

アプリケーション側が Turn 型を定義し、serialize してつむぎに渡す。

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
| `QueryClassifier` | クエリ種別判定 (regex / BERT) | Tier 0-1 |
| `PromptCompressor` | プロンプト圧縮 (LLMLingua-2 等) | Tier 2 |
| `Summarizer` | 階層的要約生成 | Tier 2-3 |

### 4. 4-tier 処理階層 (LLM 非依存を優先)

コンテキスト管理の各処理を 4 段階の計算コスト層に分けて実装する。**主処理パス (Tier 0-1) は LLM フリーで完結する** のが中核方針。Tier 2-3 はオプショナルな高品質化レイヤーとして扱う。

| Tier | 粒度 | コスト目安 | 例 |
|---|---|---|---|
| Tier 0 | 決定論 | μs 〜 ms | 正規表現、完全一致、BM25 (SQLite FTS5 + lindera)、階層走査、時間窓 |
| Tier 1 | CPU 軽量 | 数 ms | 小型 embedding (MiniLM / BGE-small)、BERT 分類器、IKE 二値化、ModernBERT |
| Tier 2 | GPU 中量 | 数十 ms | LLMLingua-2、embedding top-K 再ランク、軽量 LLM yes/no |
| Tier 3 | LLM フル | 数百 ms〜 | 階層要約生成、最終裁定抽出、最終生成 |

帰結として、最小構成ユーザー (Apple Silicon 8GB / 統合 GPU 環境) でも Tier 0-1 のみで主要機能が動作する。Tier 2-3 はアップグレード時に自然に恩恵を受ける構造。

### 5. Alloy による形式仕様 + oxidtr 生成

ドメインモデルは Alloy で記述し、oxidtr で Rust / TypeScript の型・テスト・不変条件を自動生成する。

Alloy に書くもの:
- 型構造 (newtype ID、enum、record フィールド)
- 参照整合性 (dangling ID 防止)
- 主要な不変条件 (階層非循環、supersession 非循環、pending items の寿命)

Rust に書くもの:
- 業務ロジック
- 検索・スコアリング・プロンプト組み立て

### 6. 具体最適化を維持

「汎用 = 何にでも使えるが何にも最適化されない」の罠を避ける。つむぎは:

- 実需ユースケースの具体要件に最適化しつつ
- 副次的に汎用性を持つ

仮説的な応用は可能性として示すが、**実需検証なしに設計の根拠にはしない**。

---

## 想定ユースケース

汎用メモリレイヤーフレームワークとして、以下のような長期プロジェクト型 AI ユースケースを想定する:

- **TRPG GM 補助**: セッション横断の登場人物・裁定・伏線管理
- **小説執筆補助**: 章をまたぐ世界観・キャラ設定・伏線回収の追跡
- **コーディングエージェント**: 任意プロジェクトの裁定・パターン・却下履歴管理。Claude Code の `CLAUDE.md` 方式の限界 (静的・サイズ制約・supersession 不能) を解決
- **研究 / 学習補助 AI**: 読んだ論文、考察中の問い、参考文献を永続化
- **個人秘書エージェント**: ユーザーの好み、進行中プロジェクト、継続タスクを記憶
- **教育 / 指導 AI**: 学習者の進捗、つまずき、既習範囲を追跡
- **業務 AI**: 案件・裁定履歴・未解決事項の管理
- **チャットエージェントの長期記憶層**: mem0 / LangMem 相当のポジション

ドメイン固有の型 (Character / Scene / LoreEntry / 業務 Ticket 等) はダウンストリームのアプリケーション crate で実装し、tsumugi の汎用 `Chunk` / `Fact` / `PendingItem` の上に構築する。

---

## 他ライブラリとの位置付け

### 同じ課題を扱うライブラリ

| ライブラリ | 特徴 | つむぎとの差 |
|---|---|---|
| mem0 | 対話エージェント向け長期記憶、OSS + SaaS | 階層構造が浅い、Tier 抽象なし |
| LangMem (LangChain) | 意味記憶と連想記憶の管理 | 階層が浅い、ドメイン非依存度が低い |
| Zep | Knowledge Graph ベース、有償 SaaS | クラウド、Graph 形式に絞られる |
| Graphiti | event-centric memory、Apache 2.0 | グラフ依存、階層チャンク的アクセスが弱い |
| LangChain Memory | Buffer / Summary / Hybrid | 会話履歴中心、階層が浅い |
| Letta / MemGPT | LLM 自己編集メモリ | LLM 依存度が高く Tier 0-1 完結が難しい |

### つむぎの固有ポジション

- **階層的 + 動的**: 静的 Wiki / Lorebook ではなく、現在文脈に応じた動的注入
- **supersession 対応**: 裁定の版管理を明示的に表現
- **trait 駆動の拡張性**: 各 trait が差し替え可能、ローカル / クラウド両対応
- **LLM 非依存の Tier 構造**: Tier 0-1 のみで主処理パスが完結
- **日本語対応前提**: lindera による BM25、日本語 LLM 前提の設計
- **Alloy formal spec**: 型と不変条件が形式仕様に裏付けられる

---

## 設計決定の記録

bottom-up 分析と、`docs/research/context-management-survey.md` の既存研究レビューから導出された論点の決着:

| 論点 | 決定 | 理由 |
|---|---|---|
| Turn 表現 | Turn 非依存、`Chunk.items: Vec<Value>` に製品が serialize | ドメイン Turn 型が製品ごとに根本的に違う |
| Scene と Chunk | Scene 相当を別 entity にせず Chunk + metadata で表現 | 階層は単一機構、抽象を増やさない |
| RAG hybrid | BM25 + cosine、日本語は lindera | 両方の強みが必要、`Retriever` trait で差し替え |
| RelevanceScorer | trait 化、`TemporalDecay` / `ChapterOrder` / `FileProximity` / `NoDecay` 同梱 | 減衰モデルがユースケースで質的に異なる |
| EventDetector | trait 化、3 段カスケードを `CascadeDetector` で chain | トリガー・伏線活性化・パターン違反検知の共通構造 |
| **ドメイン固有型の配置** | **コアに置かず、ダウンストリーム実装** | Character / Scene / LoreEntry 等はアプリケーション固有。コアは汎用フレームワークとして純化 |
| **階層要約の表現** | 既存 Chunk の拡張 (新規 `HierarchicalSummary` 型は作らない) | RAPTOR のツリー構造は parent/children で表現可能、抽象数を増やさない |
| **SummaryLevel 型** | `u32` (0 = Raw、正数が抽象度) | 具体ラベル (Scene/Chapter/Arc 等) はドメイン跨ぎで意味が変わる、数値は汎用 |
| **SummaryMethod enum** | `LlmFull` / `LlmLingua2` / `SelectiveContext` / `ExtractiveBM25` / `UserManual` | 要約手法の選択を明示、Tier と直接対応 |
| **SourceLocation の表現** | trait 化 (core に `FileSourceLocation` 標準実装を同梱) | ファイルパス / URI / "session/3#scene2" 等の多様な表現をアプリケーションが差し替え可能 |
| **4-tier 処理階層** | 設計原則 #4 に明文化、主処理パスは Tier 0-1 完結 | LLM 非依存で動く最小構成ユーザー対応、Tier 2-3 はアップグレード恩恵 |
| **追加 3 trait** | `QueryClassifier` / `PromptCompressor` / `Summarizer` を trait 化 | 調査書 §8 の提案を統合、段階的実装で後方互換を保つ |
| Alloy 粒度 | 中間: 型 + 参照整合 + 主要ライフサイクル | 完全制約は重い、型のみでは価値薄い |
| 配布形態 | `tsumugi-core` + `tsumugi-cli` + `tsumugi-ts` の 3 クレート | シンプルな構造を維持 |

---

## 現在のフェーズ

詳細は [`docs/TODO.md`](./TODO.md) を参照。
