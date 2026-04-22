# つむぎ — コンセプト資料

## 一言で

**創作のための、忘れない AI ミドルウェア。**

---

## プロダクトビジョン

つむぎは、LLM API の前段に配置する**創作ドメイン特化のコンテキスト管理ミドルウェア**である。TRPG キャンペーンや長編小説のように、長期にわたり多数のキャラクター・場面・設定が蓄積するユースケースで、LLM のコンテキストウィンドウ制約を超えて**一貫性**と**連続性**を維持する。

兄弟プロジェクトの chatstream が音声 AI デバイス向けに設計されているのに対し、つむぎは**テキスト創作**に最適化される。

### 核心的価値

**「前の章／前のセッションを覚えている AI」**

既存の創作 AI サービス (AIのべりすと、NovelAI、SillyTavern 等) は、ロアブックや character card などの**フラットな構造**でしかユーザーの蓄積を保持しない。長編になるほど情報の参照精度が落ちる。つむぎは階層的インデックスで**話題・場面・章の構造を動的に構築**し、関連する過去情報を LLM 呼び出しに自動で織り込む。

---

## ターゲットユーザー (= つむぎを利用する上位製品)

つむぎは最終ユーザーに直接販売されるものではなく、創作ドメインの上位製品に組み込まれる**中間層ライブラリ**である。

- **つかさ (Tsukasa)**: TRPG GM 補助。キャンペーン・セッション・NPC の階層的管理
- **つづり (Tsuzuri)**: 小説執筆補助。章・キャラクター・伏線の階層的管理
- **つくも (Tsukumo)** (将来): RPGツクール特化ツールでの長期開発セッション記憶

---

## 既存手法との位置づけ

| 手法 | アプローチ | つむぎとの差分 |
|---|---|---|
| SillyTavern Lorebook | キーワードトリガーで静的情報を挿入 | つむぎは階層的で動的、話題切り替えを検知して必要な範囲だけを注入 |
| NovelAI Codex | キャラ・場所・用語の辞書 | つむぎは辞書に加え、シーン間の時系列・因果構造を扱う |
| LangChain Memory | Buffer / Summary / Hybrid | つむぎは創作特化の Turn 多型 (narration / dialogue / action) に対応 |
| chatstream | 音声デバイス向けの階層的コンテキスト | 同じ設計思想を共有するが、テキスト創作特有のドメイン概念 (Chapter / Scene / Character / Lore) を第一級で扱う |

### 独自性

- **Turn の多型性**: 会話 (dialogue)、語り (narration)、行為 (action)、下書き (passage)、編集 (edit) を同一モデルで扱える
- **構造化ドメイン状態**: チャンクとは別レイヤーで Character / WorldState / PendingPlot 等を保持
- **形式仕様駆動**: Alloy モデルから Rust/TS の型・テスト・不変条件を oxidtr で自動生成

---

## アーキテクチャ (概要)

```
上位製品 (つかさ / つづり / つくも)
        ↓
[つむぎ]
  ├── Domain Model (Turn / Chunk / Character / Scene / Fact / LoreEntry)
  ├── Context Compiler — 常駐 + 動的コンテキストを組み立てる
  ├── Hierarchical Context Store — 全データを階層的に保持
  ├── Topic / Scene Switching Detector — cascade 式
  └── Trait 抽象
        ├── StorageProvider (InMemory / SQLite / ...)
        ├── EmbeddingProvider (Cloudflare / LM Studio / mock)
        └── LLMProvider (LM Studio / Ollama / Gemini / DeepSeek)
        ↓
ローカル LLM / クラウド LLM
```

詳細は `docs/tech-architecture.md` を参照。

---

## chatstream との関係

- **両者は独立実装**。つむぎは chatstream の派生ではなく、創作ドメインをゼロから設計した別クレート
- 共通する設計思想 (全データ保持、階層インデックス、trait 抽象) は引き継ぐ
- 将来的に chatstream の話題検知エンジンをつむぎに差し込めるよう、検知レイヤーは trait で抽象化する
- polarist-ai は chatstream をベースに構築されており、つむぎをベースにする想定はない (ドメインが異なる)

---

## 想定ユースケース

### つかさ経由

- キャンペーン 20 セッション目に「3 セッション前の商人 NPC 覚えてる？」で過去 chunk を復元
- ダイス判定の結果を fact として記録、後続の判定で参照
- 「隠し通路を探したが失敗した」などの pending_investigation を追跡

### つづり経由

- 第 10 章執筆中に「2 章で張った伏線」を lore entry として自動提示
- キャラクターの口調・性格を character sheet に蓄積、台詞生成時に参照
- 章末の未決事項 (pending_plot) を追跡し、後続章への引き継ぎを支援

---

## 未確定論点

以下はフェーズ進行に応じて順次決定する。

- Turn / Chunk 抽象の最終形 (多型 enum か、trait object か)
- 構造化ドメイン状態 (Character / WorldState) のストレージ設計
- chatstream の話題検知エンジンとの接続方針 (将来の adapter か、独立実装か)
- Alloy モデルの粒度 (不変条件どこまで書き切るか)
- 配布形態 (Rust crate 単体 / TS SDK 同梱 / Tauri 用 adapter)

---

*最終更新: 2026-04-22*
