# つむぎ (Tsumugi)

長期プロジェクト向けの汎用的なナラティブ・コンテキスト管理ミドルウェア。

LLM API の前段に配置し、セッション / 章 / 開発履歴といった**時間軸を持つプロジェクト**のコンテキストを階層的に管理する。つかさ (TRPG GM 補助) / つづり (小説執筆補助) / つくも (RPGツクール特化) の共通コアとして設計されたが、ドメイン非依存のコアと創作向け拡張を feature flag で分離しており、**他ドメイン (コーディングエージェント / 研究補助 / 業務 AI 等) への応用も可能**。

## 現在のフェーズ

Phase 0。詳細は `docs/concept.md` / `docs/tech-architecture.md` を参照。

## 関連リポジトリ

- [chatstream](https://github.com/penta2himajin/chatstream) — 兄弟ミドルウェア。音声 AI デバイス向け。共通の設計思想 (階層的インデックス、trait 抽象) を共有する
- [oxidtr](https://github.com/penta2himajin/oxidtr) — Alloy モデルからの多言語コード生成。つむぎの骨格生成に使用
- [tsukasa](https://github.com/penta2himajin/tsukasa) — TRPG GM 補助製品 (creative feature 使用)
- [tsuzuri](https://github.com/penta2himajin/tsuzuri) — 小説執筆補助製品 (creative feature 使用)
- [tsukumo](https://github.com/penta2himajin/tsukumo) — RPGツクール特化製品 (core のみ使用)
