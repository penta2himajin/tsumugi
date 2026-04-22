# つむぎ — TODO

フェーズ別。依存関係のあるタスクは前提を明記する。

## Phase 0: 設計固め (現在)

- [ ] `docs/concept.md` のレビューと固め
- [ ] `docs/tech-architecture.md` の未確定論点を 1 つずつ決定
  - [ ] Turn 表現: enum vs trait object
  - [ ] Character sheet schema の柔軟度
  - [ ] Scene と Chunk の関係
  - [ ] lore entry embedding 戦略
  - [ ] RAG ハイブリッド方針
- [ ] Alloy モデル `models/tsumugi.als` の初版作成
- [ ] oxidtr 生成の動作確認 (Rust / TS 両方)
- [ ] ワークスペース skeleton (`tsumugi-core`, `tsumugi-cli`, `tsumugi-ts`) の初期化

## Phase 1: コア実装

- [ ] `StorageProvider` trait 定義 + `InMemoryStorage` 実装
- [ ] `EmbeddingProvider` trait 定義 + モック実装
- [ ] `LLMProvider` trait 定義 + LM Studio アダプタ
- [ ] ドメインモデル (Turn / Chunk / Character / Scene / Fact / LoreEntry) 実装
- [ ] 階層的 chunk store 実装
- [ ] 話題 / シーン切替検知 (Stage 1 埋め込み類似度)
- [ ] Context Compiler (常駐 + 動的)
- [ ] インテグレーションテスト (小説執筆シナリオ、TRPG セッションシナリオの 2 本)

## Phase 2: 上位製品からの利用

- [ ] つかさから tsumugi-core を依存として利用開始
- [ ] つづりから tsumugi-core を依存として利用開始
- [ ] Tauri 統合時の IPC 境界設計 (oxidtr 経由で Rust ⇔ TS 型共有)
- [ ] 話題検知 Stage 2 (軽量 LLM 分類) 実装
- [ ] Stage 3 (ユーザー確認 UI) の interface 定義 (UI 実装は上位製品側)

## Phase 3: 最適化と拡張

- [ ] 動的階層分割 (chunk 肥大化時の自動サブチャンク化)
- [ ] Pending plot / investigation の追跡機構
- [ ] recency バイアス (power-law 時間減衰)
- [ ] chatstream 話題検知モジュールとの差し替え可能性の検証
- [ ] SQLite ストレージ実装 (InMemory から永続化へ)
- [ ] TypeScript SDK (`tsumugi-ts`) 整備

## Phase 4 (未定): 公開

- [ ] README / docs の英語版整備 (公開する場合)
- [ ] ライセンス決定 (MIT / Apache-2.0 デュアル、または専有)
- [ ] crates.io 公開 (公開する場合)

## 未確定の大論点

- chatstream との統合方針 (adapter trait か、独立実装を貫くか)
- つくもへの適用可能性 (つくもはコード生成中心なので、つむぎの効用が薄い可能性あり)
- 公開戦略 (OSS か、商用ライブラリか)
