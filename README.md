# つむぎ (Tsumugi)

創作向け AI エージェントのための、階層的ナラティブ・コンテキスト処理ミドルウェア。

**つかさ** (TRPG GM 補助) および **つづり** (小説執筆補助) の共通コアエンジン。将来的には **つくも** (RPGツクール特化) のセッション記憶層としても利用可能な設計を目指す。

## 現在のフェーズ

Phase 0。詳細は `docs/concept.md` / `docs/tech-architecture.md` を参照。

## 関連リポジトリ

- [chatstream](https://github.com/penta2himajin/chatstream) — 兄弟ミドルウェア。音声 AI デバイス向け。共通の設計思想 (階層的インデックス、trait 抽象) を共有する
- [oxidtr](https://github.com/penta2himajin/oxidtr) — Alloy モデルからの多言語コード生成。つむぎの骨格生成に使用
- [tsukasa](https://github.com/penta2himajin/tsukasa) — TRPG GM 補助製品
- [tsuzuri](https://github.com/penta2himajin/tsuzuri) — 小説執筆補助製品
- [tsukumo](https://github.com/penta2himajin/tsukumo) — RPGツクール特化製品 (将来的統合候補)
