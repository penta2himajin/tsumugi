# つむぎ (Tsumugi)

LLM アプリケーション向けの汎用メモリレイヤーフレームワーク。

セッション / 章 / 開発履歴といった**時間軸を持つプロジェクト**のコンテキストを `Chunk` の階層として保持し、要約・クエリ分類・プロンプト圧縮の抽象化を通じて LLM 入力に選択的に投入する。TRPG GM 補助・小説執筆・コーディングエージェント・研究補助・業務 AI など、長期プロジェクトのメモリが必要な領域への応用を想定する。

## 現在のフェーズ

詳細は [`docs/concept.md`](./docs/concept.md) / [`docs/tech-architecture.md`](./docs/tech-architecture.md) / [`docs/TODO.md`](./docs/TODO.md) を参照。

## ビルド & テスト

```bash
cargo build --workspace
cargo test  --workspace
cargo test  --workspace --all-features
bun run --cwd tsumugi-ts typecheck
bun run --cwd tsumugi-ts test
```

## 設計原則

- **コアはドメイン非依存**: `tsumugi-core` は汎用メモリレイヤー API のみを公開する。ドメイン固有の型はダウンストリームで実装する
- **フル履歴は保持、注入は選択的**: 全データを残しつつ LLM への投入は階層からの選択的合成で行う
- **段階的処理**: Tier 0 (BM25 / 規則ベース) → Tier 1 (軽量分類器) → Tier 2 (軽量圧縮) → Tier 3 (LLM 呼出)。安易に LLM を呼ばない
- **Storage / Embedding / LLM は trait 抽象**: コアは特定のベクタ DB / API / クライアントに依存しない
- **Alloy モデルが正典**: `models/` から oxidtr で Rust / TypeScript 型を生成。`gen/` 配下は手編集禁止

## License

Licensed under the Apache License, Version 2.0 (the "License"); you may not use
this software except in compliance with the License. You may obtain a copy of
the License in the [LICENSE](./LICENSE) file or at
<http://www.apache.org/licenses/LICENSE-2.0>.

Unless required by applicable law or agreed to in writing, software distributed
under the License is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR
CONDITIONS OF ANY KIND, either express or implied.
