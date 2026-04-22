# つむぎ — TODO

フェーズ別。つかさ MVP と同時並行での Phase 1 実装を想定。

## Phase 0: 設計固め (現在、完了間近)

- [x] bottom-up 抽出: つかさ・つづり・つくもの docs 深掘り完了
- [x] 10 論点への決着 (concept.md / tech-architecture.md に反映済み)
- [x] 汎用コンテキストエンジンへの再ポジショニング (2026-04-22)
- [x] `FileProximityScorer` の追加と `SourceLocation` 追加を反映
- [x] `creative` feature flag による分離方針を反映
- [ ] Alloy モデル `models/tsumugi-core.als` 初版
  - [ ] Chunk, Fact, LoreEntry, PendingItem の sig
  - [ ] 参照整合性の述語
  - [ ] 階層非循環の不変条件
  - [ ] PendingItem ライフサイクルの不変条件
  - [ ] Fact supersession の非循環
  - [ ] `SourceLocation` の型定義
- [ ] Alloy モデル `models/tsumugi-creative.als` 初版
  - [ ] Character, SceneView, StylePreset の sig
  - [ ] Character の first_appearance 整合性
- [ ] oxidtr 生成フロー動作確認
  - [ ] core と creative の分離生成
- [ ] ワークスペース skeleton
  - [ ] `tsumugi-core/` の Cargo.toml + src/ 雛形
  - [ ] `creative` feature flag の設定
  - [ ] `tsumugi-cli/` の Cargo.toml + main.rs 雛形
  - [ ] `tsumugi-ts/` の package.json + tsconfig 雛形

## Phase 1: コア実装 (つかさ MVP と並行)

### A. 型定義とストレージ

- [ ] `tsumugi-core/src/domain/` 手書き拡張 (Alloy 生成 + 追加ロジック)
- [ ] `tsumugi-core/src/creative/` 手書き拡張 (feature = "creative")
- [ ] `StorageProvider` trait 定義 (core / creative 分離)
- [ ] `InMemoryStorage` 実装
- [ ] 結合テスト (save / load / delete / list、feature on/off 両方)

### B. Embedding / LLM

- [ ] `EmbeddingProvider` trait 定義
- [ ] `MockEmbedding` 実装
- [ ] `LMStudioEmbedding` 実装
- [ ] `LLMProvider` trait 定義
- [ ] `LMStudioProvider` 実装
- [ ] `OllamaProvider` 実装

### C. 検索とスコアリング

- [ ] `Retriever` trait 定義
- [ ] lindera による BM25 実装
- [ ] cosine 類似度実装
- [ ] `HybridRetriever` 実装
- [ ] `RelevanceScorer` trait 定義
- [ ] `TemporalDecayScorer` 実装
- [ ] `ChapterOrderScorer` 実装
- [ ] **`FileProximityScorer` 実装 (SourceLocation ベース)**
- [ ] `NoDecayScorer` 実装
- [ ] `CompositeScorer` 実装

### D. イベント検知

- [ ] `EventDetector` trait 定義
- [ ] `KeywordDetector` 実装
- [ ] `EmbeddingSimilarityDetector` 実装
- [ ] `LLMClassifierDetector` 実装
- [ ] `CascadeDetector` 実装

### E. Context Compiler

- [ ] `CompiledContext` 型定義 (core / creative 分離)
- [ ] 常駐レイヤー構築
- [ ] 動的レイヤー構築
- [ ] 結合テスト: 小説シナリオ 1 本で context compile (creative feature)
- [ ] 結合テスト: コーディングシナリオ 1 本で context compile (core のみ)

### F. 結合テスト

- [ ] TRPG シナリオ (CoC ミニセッション 3 シーン) の end-to-end
- [ ] 小説シナリオ (短編 5 章分) の end-to-end
- [ ] **ツクールシナリオ (MZ プロジェクトでの裁定記憶) の end-to-end**
- [ ] つかさ / つづり / つくもからの依存導入確認 (`cargo tree`)
- [ ] feature flag の組み合わせ検証 (default / creative)

## Phase 2: 上位製品統合と調整 (つかさリリース後)

- [ ] つかさで実戦投入、frictions を記録
- [ ] 抽出した friction から API 改善
- [ ] つづり実装開始前に API 安定化
- [ ] `SqliteStorage` 実装 (sqlx ベース)
- [ ] sqlite-vec 統合検証
- [ ] Summarizer パイプライン
- [ ] つくも実装でのフィードバック反映
- [ ] `FileProximityScorer` の改良 (モジュール依存グラフベース)

## Phase 3: TypeScript SDK

- [ ] `tsumugi-ts/` の実装開始
- [ ] oxidtr 生成型を活用した TS 側 API
- [ ] Tauri IPC での Rust ↔ TS 型整合性確認

## Phase 4: 拡張 (実需次第)

- [ ] chatstream との抽象共通化検討
- [ ] 追加 feature flag の設計 (`coding`, `research`, `business` 等、実需発生時のみ)
- [ ] Alloy 生成制約のさらなる追加
- [ ] パフォーマンス最適化 (embedding batch 化、検索インデックス)
- [ ] Summarizer パイプラインの品質改善

## Phase 5 (未定): 公開

- [ ] README / docs の英語版整備
- [ ] ライセンス決定
- [ ] crates.io 公開判断
- [ ] npm 公開判断 (tsumugi-ts)

## 未確定の大論点

- sqlite-vec の採用タイミングと性能評価
- `Chunk.items` の serialize 形式
- `SourceLocation` の表現抽象度
- Summarizer の LLM コスト (ローカル / クラウド fallback)
- chatstream との統合スコープと時期
- 公開可否 (OSS / 商用)
- 英語 docs の整備優先度
- 追加 feature flag (`coding`, `research`) の設計タイミング (実需が出たときだけ)
