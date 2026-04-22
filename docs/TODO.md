# つむぎ — TODO

フェーズ別。つかさ MVP と同時並行での Phase 1 実装を想定。

## Phase 0: 設計固め (現在、完了間近)

- [x] bottom-up 抽出: つかさ・つづりの docs 深掘り完了
- [x] 10 論点への決着 (concept.md / tech-architecture.md に反映済み)
- [ ] Alloy モデル `models/tsumugi.als` 初版
  - [ ] Chunk, Fact, Character, LoreEntry, PendingItem の sig
  - [ ] 参照整合性の述語
  - [ ] 階層非循環の不変条件
  - [ ] PendingItem ライフサイクルの不変条件
  - [ ] Fact supersession の非循環
- [ ] oxidtr 生成フロー動作確認
  - [ ] `oxidtr generate models/tsumugi.als --target rust --output tsumugi-core/src/domain/gen/`
  - [ ] `oxidtr generate models/tsumugi.als --target ts --output tsumugi-ts/src/gen/`
- [ ] ワークスペース skeleton
  - [ ] `tsumugi-core/` の Cargo.toml + src/ 雛形
  - [ ] `tsumugi-cli/` の Cargo.toml + main.rs 雛形
  - [ ] `tsumugi-ts/` の package.json + tsconfig 雛形

## Phase 1: コア実装 (つかさ MVP と並行)

### A. 型定義とストレージ

- [ ] `tsumugi-core/src/domain/` 手書き拡張 (Alloy 生成 + 追加ロジック)
- [ ] `StorageProvider` trait 定義
- [ ] `InMemoryStorage` 実装
- [ ] 結合テスト (save / load / delete / list の動作)

### B. Embedding / LLM

- [ ] `EmbeddingProvider` trait 定義
- [ ] `MockEmbedding` 実装 (テスト用、ハッシュベース)
- [ ] `LMStudioEmbedding` 実装
- [ ] `LLMProvider` trait 定義
- [ ] `LMStudioProvider` 実装 (OpenAI 互換 API)
- [ ] `OllamaProvider` 実装

### C. 検索とスコアリング

- [ ] `Retriever` trait 定義
- [ ] lindera による BM25 実装
- [ ] cosine 類似度実装
- [ ] `HybridRetriever` 実装 (重み可変)
- [ ] `RelevanceScorer` trait 定義
- [ ] `TemporalDecayScorer` 実装
- [ ] `ChapterOrderScorer` 実装
- [ ] `NoDecayScorer` 実装
- [ ] `CompositeScorer` 実装

### D. イベント検知

- [ ] `EventDetector` trait 定義
- [ ] `KeywordDetector` 実装
- [ ] `EmbeddingSimilarityDetector` 実装
- [ ] `LLMClassifierDetector` 実装
- [ ] `CascadeDetector` 実装 (3 段チェーン)

### E. Context Compiler

- [ ] `CompiledContext` 型定義
- [ ] 常駐レイヤー構築 (current scene / recent turns / active characters)
- [ ] 動的レイヤー構築 (related chunks / lore / pending / facts)
- [ ] 結合テスト: 小説シナリオ 1 本で context compile

### F. 結合テスト

- [ ] TRPG シナリオ (CoC のミニセッション 3 シーン分) を通した end-to-end テスト
- [ ] 小説シナリオ (短編 5 章分) を通した end-to-end テスト
- [ ] つかさ / つづりからの依存導入確認 (`cargo tree` で循環なし)

## Phase 2: 上位製品統合と調整 (つかさリリース後)

- [ ] つかさで実戦投入、frictions を記録
- [ ] 抽出した friction から API 改善 (trait シグネチャ変更、Context Compiler 調整)
- [ ] つづり実装開始前に API 安定化
- [ ] `SqliteStorage` 実装 (sqlx ベース)
- [ ] sqlite-vec 統合検証
- [ ] Summarizer パイプライン (chunk 肥大化時の階層化)
- [ ] Recency バイアスの実装微調整

## Phase 3: TypeScript SDK

- [ ] `tsumugi-ts/` の実装開始 (つづり実装時に要件明確化してから)
- [ ] oxidtr 生成型を活用した TS 側 API
- [ ] Tauri IPC での Rust ↔ TS 型整合性確認

## Phase 4: 拡張 (実需次第)

- [ ] chatstream との抽象共通化検討
  - [ ] `EventDetector` の chatstream 側輸入
  - [ ] Chunk 抽象の統合可能性評価
- [ ] `tsumugi-kv` サブクレート分離検討 (つくもが始まったら)
- [ ] Alloy 生成制約のさらなる追加 (実運用で見つかった不変条件)
- [ ] パフォーマンス最適化 (embedding batch 化、検索インデックス)
- [ ] Summarizer パイプラインの品質改善

## Phase 5 (未定): 公開

- [ ] README / docs の英語版整備
- [ ] ライセンス決定 (MIT / Apache-2.0 デュアル、または専有)
- [ ] crates.io 公開判断
- [ ] npm 公開判断 (tsumugi-ts)

## 未確定の大論点

- sqlite-vec の採用タイミングと性能評価
- `Chunk.items` の serialize 形式 (JSON がデフォルト、MessagePack 採用可否)
- Summarizer の LLM コスト (ローカルか、クラウド fallback か)
- chatstream との統合スコープと時期
- 公開可否 (OSS か商用ライブラリか)
- 英語 docs の整備優先度
