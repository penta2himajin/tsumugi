# 評価用データセット候補とライセンス調査

## ステータス

- **著者**: @penta2himajin
- **初版**: 2026-04-25
- **ステータス**: Draft (調査資料)
- **目的**: Mayu (旧 tsumugi) の内部テスト資産として利用可能なベンチマーク評価データセットを調査・整理する
- **関連**: [concept.md](./concept.md), [tech-architecture.md](./tech-architecture.md), [monetization-strategy.md](./monetization-strategy.md)

---

## 概要

Mayu の開発において、ベンチマークデータセットを **マーケティング目的のスコア指標** ではなく **内部テスト資産** として活用する方針を取る。具体的には:

- **機能網羅性の検証**: 5-6 カテゴリのメモリ能力を体系的にテスト
- **Regression 防止**: CI 統合により、機能変更時の精度低下を早期検出
- **Tier別 ablation**: Tier 0 (BM25) / Tier 1 (semantic) / Tier 2-3 (LLM) の貢献度を定量化
- **価格設計の根拠**: 「Tier X で Y % カバー」を内部的に確信し、プラン設計の正当化に使う
- **公開判断は別**: スコア公開は後送り、内部品質確保が先

本書では、調査した既存ベンチマークのライセンス状況と、Mayu のテスト資産として **法的にクリーンに使える** 候補を整理する。

---

## 調査した評価軸

メモリシステムに必要な能力を以下の軸で整理:

| 評価軸 | 内容 | Mayu の対応機能 |
|---|---|---|
| **Information Extraction (IE)** | 単純な事実検索 | Tier 1 (semantic search) |
| **Multi-Session Reasoning (MR)** | 複数セッション横断推論 | 階層 Summary、scope 連携 |
| **Knowledge Update (KU)** | 事実の更新追跡 | **Supersession (差別化機能)** |
| **Temporal Reasoning (TR)** | 時間軸推論 | provenance 内のタイムスタンプ |
| **Abstention (ABS)** | 「知らない」を返せるか | API 設計 |
| **Conflict Resolution (CR)** | 矛盾検出と更新 | **Supersession (差別化機能)** |
| **Test-Time Learning (TTL)** | 実行時の学習 | (将来的検討) |
| **Long-Range Understanding (LRU)** | 長文脈横断の抽象理解 | 階層 Summary |
| **Hallucination 抑制** | 誤生成の検出/防止 | **Provenance Hash Chain (Pro+)** |

---

## ライセンス調査結果一覧

### Tier S (最優先採用): 商用 OK + 機能網羅性高

| データセット | License | 規模 | カバー軸 | Mayu での価値 |
|---|---|---|---|---|
| **LongMemEval** | **MIT** | 500 QA, 5 cat | IE, MR, KU, TR, ABS | 業界標準、5 カテゴリ完全網羅 |
| **MemoryAgentBench** | **MIT** | 146 sample, 4 cat | AR, TTL, LRU, **CR** | **Conflict Resolution = supersession 直接検証** |
| **RULER** | **Apache 2.0** | 13 task, ~128K tok | retrieval, multi-hop, aggregation | NIAH 拡張、合成データ、Tier 0 baseline |

### Tier A (補完採用): 商用 OK

| データセット | License | 規模 | 用途 |
|---|---|---|---|
| **NarrativeQA** | **Apache 2.0** | 32K QA | Long-document reading、階層検証 |
| **MultiHop-RAG** | **ODC-BY 1.0** | 2,556 query | Multi-hop retrieval、ニュース記事 |
| **HotpotQA** | **CC BY-SA 4.0** | 113K QA | Multi-hop QA、Share-Alike 注意 |
| **NIAH (Kamradt)** | **MIT** | 合成可 | Tier 0 (BM25) baseline、需要時生成 |
| **ShareGPT** | **Apache 2.0** | 大規模 | LongMemEval 素材、合成元 |
| **UltraChat** | **MIT** | 大規模 | LongMemEval 素材、合成元 |
| **Nemotron-Personas** | **CC BY 4.0** | persona pool | persona 生成元 (将来用) |

### Tier D (除外): 商用ライセンス制約

| データセット | License | 除外理由 |
|---|---|---|
| **LoCoMo** | **CC BY-NC 4.0** | 非商用ライセンス、本番開発から除外 |
| **ConvoMem (Salesforce)** | **CC BY-NC 4.0** | 非商用ライセンス、規模 75K あるが使用不可 |
| **DialSim** | TV scripts copyright | Friends/BBT/Office 著作権、使用回避 |

### 保留 (要直接確認): ライセンス不明確

| データセット | 状態 | 推奨 |
|---|---|---|
| **MemoryBench (THUIR / 清華)** | GitHub・HF 共にライセンス明示なし | **使用回避** (no license = all rights reserved) |
| **HaluMem (IAAR-Shanghai)** | 明示license不在、運営組織は CC BY-NC-ND 4.0 傾向 | **非商用の可能性高、使用回避** |
| **RealTalk** | snap-research 系、LoCoMo と同系列 | **CC BY-NC 4.0 の可能性高、使用回避** |
| **MSC (Multi-Session Chat)** | ParlAI MIT、データ層は要個別確認 | 確認後採用可能性 |

---

## 採用候補の詳細

### LongMemEval (MIT)

- **論文**: Wu et al., "LongMemEval: Benchmarking Chat Assistants on Long-Term Interactive Memory" (ICLR 2025)
- **GitHub**: [xiaowu0162/LongMemEval](https://github.com/xiaowu0162/LongMemEval)
- **HuggingFace**: [xiaowu0162/longmemeval-cleaned](https://huggingface.co/datasets/xiaowu0162/longmemeval-cleaned)
- **構造**:
  - 500 questions
  - LongMemEval_S: ~115K tokens / question (約 40-48 sessions)
  - LongMemEval_M: 約 500 sessions / question (約 1.5M tokens)
  - LongMemEval_oracle: evidence sessions のみ (検索精度評価用)
- **6 question type**:
  - single-session-user
  - single-session-assistant
  - single-session-preference
  - temporal-reasoning
  - **knowledge-update (Mayu supersession 検証)**
  - multi-session
  - `_abs` 接尾辞は abstention questions
- **License 詳細**: MIT。素材として ShareGPT (Apache 2.0) と UltraChat (MIT) を使用、release 自体は MIT で問題なし
- **Mayu での主用途**:
  - Knowledge Update カテゴリで supersession の動作検証
  - 5 カテゴリ ablation で Tier 別の貢献度測定
  - 業界標準スコアの内部把握

### MemoryAgentBench (MIT)

- **論文**: Hu et al., "Evaluating Memory in LLM Agents via Incremental Multi-Turn Interactions" (ICLR 2026 採択)
- **GitHub**: [HUST-AI-HYZ/MemoryAgentBench](https://github.com/HUST-AI-HYZ/MemoryAgentBench)
- **HuggingFace**: [ai-hyz/MemoryAgentBench](https://huggingface.co/datasets/ai-hyz/MemoryAgentBench)
- **構造**: 146 sample、4 split
  - Accurate_Retrieval (22 rows)
  - Test_Time_Learning (6 rows)
  - Long_Range_Understanding (110 rows)
  - **Conflict_Resolution (8 rows)**
- **新規データセット**:
  - **EventQA**: 小説の時系列イベント連鎖を理解し「次に何が起こるか」を予測
  - **FactConsolidation**: 矛盾解消能力 (single-hop / multi-hop)
- **業界知見** (論文主要発見):
  - 「RAG は銀の弾丸ではない」: BM25 単純実装が GPT-4o-mini ベースラインを大幅超過 (NIAH-MQ で 100% vs 22.8%)
  - 「Conflict Resolution は最大の課題」: GPT-4o ベースで single-hop 60% / multi-hop 1 桁 %
  - 「Mem0 の memory 構築時間は BM25 の 20,000 倍」「Cognee は 512-token 入力で 1 サンプルに 3.3 時間」
- **License 詳細**: MIT (HF dataset card で確認)
- **Mayu での主用途**:
  - **Conflict Resolution split = supersession の直接検証 (最重要)**
  - Test-Time Learning は将来機能の評価軸
  - Long-Range Understanding で階層 Summary 検証

### RULER (Apache 2.0)

- **論文**: Hsieh et al., "RULER: What's the Real Context Size of Your Long-Context Language Models?" (2024)
- **GitHub**: [NVIDIA/RULER](https://github.com/NVIDIA/RULER)
- **構造**: 13 task across 4 categories
  - **Retrieval**: NIAH 拡張 (single/multi-keys、values、queries)
  - **Multi-hop tracing**: variable tracking (coreference 解決)
  - **Aggregation**: common/frequent words extraction (要約代理タスク)
  - **Question Answering**: distracting context 付きの既存 QA
- **特徴**:
  - 完全合成、シーケンス長と複雑度を可変
  - 4K - 128K-2M tokens まで段階的評価
  - NVIDIA 製、業界で長文脈評価の標準
- **License**: Apache 2.0 (LICENSE ファイルで確認)
- **Mayu での主用途**:
  - Tier 0 (BM25) の長文脈 baseline 測定
  - 合成データなので著作権リスクなし、CI で自由に利用
  - Multi-hop tracing は Mayu の階層検索評価に有用

### NarrativeQA (Apache 2.0)

- **論文**: Kočiský et al., "The NarrativeQA Reading Comprehension Challenge" (TACL 2018)
- **GitHub**: [google-deepmind/narrativeqa](https://github.com/google-deepmind/narrativeqa)
- **HuggingFace**: [deepmind/narrativeqa](https://huggingface.co/datasets/deepmind/narrativeqa)
- **構造**: 32,747 train + 3,461 valid + 10,557 test、平均 41,000 word/story
- **License**: Apache 2.0
- **Mayu での主用途**: 長文書読解、階層 Summary の意味的整合性検証

### MultiHop-RAG (ODC-BY 1.0)

- **論文**: Tang & Yang, "MultiHop-RAG: Benchmarking Retrieval-Augmented Generation for Multi-Hop Queries" (COLM 2024)
- **GitHub**: [yixuantt/MultiHop-RAG](https://github.com/yixuantt/MultiHop-RAG)
- **HuggingFace**: [yixuantt/MultiHopRAG](https://huggingface.co/datasets/yixuantt/MultiHopRAG)
- **構造**: 2,556 query、2-4 documents の evidence、2023-09 から 2023-12 のニュース記事
- **License**: ODC-BY (Open Data Commons Attribution、商用 OK、attribution 必要)
- **Mayu での主用途**: Multi-hop retrieval の精度評価、metadata 含む実用シナリオ

### HotpotQA (CC BY-SA 4.0)

- **論文**: Yang et al., "HotpotQA: A Dataset for Diverse, Explainable Multi-hop Question Answering" (EMNLP 2018)
- **GitHub**: [hotpotqa/hotpot](https://github.com/hotpotqa/hotpot)
- **構造**: 113K QA、Wikipedia ベース、bridge / comparison タイプ、sentence-level supporting facts
- **License**: CC BY-SA 4.0 (Share-Alike)
- **Mayu での扱い**:
  - **読み込み利用は問題なし** (内部 CI 評価、結果は Mayu 独自著作物)
  - **データ自体の再配布は CC BY-SA 継承** (Mayu repo に含めない運用で回避)
  - Apache 2.0 OSS との互換性は「データ非配布」前提なら問題なし

### NIAH (MIT)

- **GitHub**: [gkamradt/LLMTest_NeedleInAHaystack](https://github.com/gkamradt/LLMTest_NeedleInAHaystack)
- **特徴**: 合成可能、Greg Kamradt 版が業界標準
- **License**: MIT
- **Mayu での主用途**: Tier 0 (BM25) の純粋 baseline、自前生成も容易

### Nemotron-Personas (CC BY 4.0)

- **NVIDIA 製**: 多様な persona の合成データセット
- **License**: CC BY 4.0
- **Mayu での主用途**: 将来的な独自ベンチマーク (日本語版含む) 構築時の persona 素材

---

## 除外候補の詳細

### LoCoMo (CC BY-NC 4.0)

- **論文**: Maharana et al., "Evaluating Very Long-Term Conversational Memory of LLM Agents" (ACL 2024)
- **GitHub**: [snap-research/locomo](https://github.com/snap-research/locomo)
- **構造**: 10 conversations、各 ~300 turns / 9K tokens / 35 sessions、1,986 QA pairs
- **業界での位置**: Mem0 vs Zep の論争中心、業界注目度最高
- **除外理由**:
  - **CC BY-NC 4.0 (非商用)** ライセンス
  - Mayu 商用化前提では本番開発に使用不可
  - 「研究目的」のグレーゾーンも、商用 SaaS の品質保証 CI に組み込むのはリスク
- **業界影響への対処**:
  - LongMemEval (Knowledge Update) + MemoryAgentBench (Conflict Resolution) の **二軸で supersession 優位を主張**
  - LoCoMo スコア比較は内部研究目的でのみ参照、商用判断には使わない

### ConvoMem (CC BY-NC 4.0)

- **論文**: Salesforce AI Research, "ConvoMem Benchmark: Why Your First 150 Conversations Don't Need RAG" (2025-11)
- **HuggingFace**: [Salesforce/ConvoMem](https://huggingface.co/datasets/Salesforce/ConvoMem)
- **構造**: 75,336 QA、100 personas、40,000 filler conversations、6 evidence categories、context size 1-300 messages
- **論文の主張**: 「最初の 150 conversations は RAG 不要」「naive long context が 70-82% 精度、洗練された RAG-based memory が 30-45%」
- **除外理由**:
  - **CC BY-NC 4.0 (非商用)** (HF dataset card で 2026-04-25 確認)
  - 当初推測 (CC BY 4.0) は誤り、HF cardで明確に NC と記載
- **論文主張との整合性**: Mayu の「低 Tier 志向」と論文主張は方向性一致、引用は可能

### DialSim

- **論文**: Kim et al., "DialSim: A Real-Time Simulator for Evaluating Long-Term Multi-Party Dialogue Understanding" (2024)
- **GitHub**: [jiho283/Simulator](https://github.com/jiho283/Simulator)
- **除外理由**: Friends / Big Bang Theory / The Office の TV スクリプト全文使用 = TV 局の著作権、商用利用は明確に不可

---

## 保留 (使用回避推奨) 候補の詳細

### MemoryBench (THUIR / 清華)

- **論文**: Ai et al., "MemoryBench: A Benchmark for Memory and Continual Learning in LLM Systems" (2025-10)
- **GitHub**: [LittleDinoC/MemoryBench](https://github.com/LittleDinoC/MemoryBench)
- **HuggingFace**: [THUIR/MemoryBench](https://huggingface.co/datasets/THUIR/MemoryBench)
- **構造**: 11 公開ベンチマーク統合 (Locomo, DialSim, LexEval, IdeaBench, JuDGE 等)、20,000 cases、英語 + 中国語
- **保留理由**:
  - GitHub repo に LICENSE ファイル不在
  - HF dataset card にライセンス記載なし
  - **GitHub の no license = all rights reserved** (著作権法デフォルト)
  - 構成データ自体に LoCoMo (CC BY-NC) を含む = 派生物も非商用継承
- **対応**: 著者直接確認しない限り使用回避

### HaluMem (IAAR-Shanghai / MemTensor)

- **論文**: "HaluMem: Evaluating Hallucinations in Memory Systems of Agents" (2025-11)
- **GitHub**: [MemTensor/HaluMem](https://github.com/MemTensor/HaluMem)
- **HuggingFace**: [IAAR-Shanghai/HaluMem](https://huggingface.co/datasets/IAAR-Shanghai/HaluMem)
- **構造**: HaluMem-Medium + HaluMem-Long、~15K memory points、3.5K queries、最大 1M tokens / user
- **保留理由**:
  - GitHub repo に明示 license 不在
  - 同じ IAAR-Shanghai 配下の他モデル (xFinder 系) が **CC BY-NC-ND 4.0** 採用傾向
  - 非商用かつ改変禁止の可能性高
- **代替**: Mayu 自作の hallucination test (Provenance Hash Chain ベース)

### RealTalk (推定 CC BY-NC 4.0)

- **論文**: Lee et al., "REALTALK: A 21-Day Real-World Dataset for Long-Term Conversation" (2025-02)
- **GitHub**: [danny911kr/REALTALK](https://github.com/danny911kr/REALTALK)
- **構造**: 21 日間のリアル人間対話、10 participants × 2 conversations
- **保留理由**:
  - 著者陣 (Maharana, Pujara, Ren, Barbieri) は LoCoMo と同系列、Snap Inc. インターンで作成
  - 論文 preprint は CC BY 4.0 だが、データセット本体は LoCoMo と同じ CC BY-NC 4.0 の可能性高
  - 直接確認していない
- **対応**: 直接確認しない限り使用回避

### MSC (Multi-Session Chat)

- **論文**: Xu et al., "Beyond Goldfish Memory: Long-Term Open-Domain Conversation" (ACL 2022)
- **配布**: ParlAI フレームワーク (`parlai/tasks/msc`)
- **構造**: 5 セッション、237K train + 25K valid examples
- **保留理由**: ParlAI 自体は MIT ライセンスだが、データ層が個別ライセンスの可能性
- **対応**: ParlAI の各 task ライセンスを個別確認後、採用可能性あり

---

## ライセンス遵守の運用ルール

### 1. Mayu リポジトリへのデータ含有方針

- **原則**: ベンチマークデータは Mayu リポジトリに含めない
- **CI 実行時**: 公式配布元 (HuggingFace, GitHub Releases) から都度ダウンロード
- **理由**:
  - Share-Alike (CC BY-SA) 等の継承制約を回避
  - データセット側の更新追従コスト削減
  - リポジトリサイズ管理

### 2. Attribution / Citation の管理

- `THIRD_PARTY_LICENSES.md` を作成し、使用する各データセットの:
  - 名称、ライセンス、引用情報 (BibTeX)
  - 公式 URL
  - 使用範囲 (内部 CI のみ / 公開評価結果あり)
- README に「使用ベンチマーク」セクションを設置

### 3. 評価結果の公開境界

- **内部スコア**: Kenya のみ閲覧、Mayu 開発判断に使用
- **準公開スコア**: マイルストーン報告、ブログ等で参照
- **公式スコア**: README、論文、公式サイトでの主張
- ライセンスの attribution 要件は **公式スコア時に必須**、内部利用では citation 内部記録で十分

### 4. 派生データセットの扱い

- HotpotQA を改変して再配布する場合 → CC BY-SA 4.0 継承必須
- Mayu の Apache 2.0 OSS と衝突するため、**改変・再配布はしない**
- 評価結果のみを Mayu 著作物として扱う

### 5. 新作ベンチマーク追従戦略

- 新作 benchmark は 3-6 ヶ月毎に登場
- **追従ポリシー**:
  - 信頼ソース固定: NVIDIA、DeepMind、MIT、ICLR/NeurIPS 採択論文
  - 年 1-2 回 review 実施、Tier S/A 候補を更新
  - License 変更や新規データセットを定期チェック

---

## 想定される Mayu Test Suite 構成

```
tests/benchmarks/
├── core/                       # 必須実行 (CI で nightly)
│   ├── longmemeval/            # MIT、5 カテゴリ網羅
│   │   ├── runner.ts
│   │   └── data/               # .gitignore (要 download)
│   └── memoryagentbench/       # MIT、4 カテゴリ
│       ├── runner.ts
│       └── data/               # .gitignore
├── extended/                   # 補完実行 (CI で weekly)
│   ├── ruler/                  # Apache 2.0、長文脈
│   ├── narrativeqa/            # Apache 2.0
│   ├── multihop-rag/           # ODC-BY
│   └── hotpotqa/               # CC BY-SA 4.0
├── baseline/                   # baseline 専用 (任意実行)
│   ├── niah/                   # MIT、合成生成
│   └── sharegpt-sample/        # Apache 2.0
└── custom/                     # Mayu 自作
    ├── japanese-supersession/  # 自作、日本語 supersession
    └── japanese-temporal/      # 自作、日本語 temporal

# License attribution
THIRD_PARTY_LICENSES.md
```

---

## 段階的実装計画

### Phase 1: ライセンス安全な基盤構築 (1-2 週間)

1. LongMemEval ダウンロード & runner プロトタイプ
2. MemoryAgentBench ダウンロード & runner プロトタイプ
3. RULER 合成生成スクリプト動作確認
4. `THIRD_PARTY_LICENSES.md` 作成

### Phase 2: 機能網羅検証 (2-4 週間)

5. LongMemEval Knowledge Update で Mayu supersession の初期テスト
6. MemoryAgentBench Conflict Resolution で supersession 軸テスト
7. RULER で Tier 0 (BM25) の長文脈 baseline 測定
8. NIAH 合成で Tier 0 純粋 baseline

### Phase 3: 補完評価と CI 統合 (1-2 ヶ月)

9. NarrativeQA, MultiHop-RAG, HotpotQA 統合
10. CI (GitHub Actions) で nightly 実行
11. スコア regression alert 実装
12. 内部ダッシュボード (Cloudflare Pages 等)

### Phase 4: 独自ベンチマーク (3-6 ヶ月)

13. 日本語 supersession scenarios 設計 (50-100 問)
14. 日本語 temporal reasoning scenarios 設計
15. Provenance Hash Chain 検証ベンチ (HaluMem 代替)

---

## 業界文脈と Mayu のポジショニング

### LoCoMo を使えないことの戦略的影響

- 業界の中心議論 (Mem0 vs Zep) が LoCoMo
- LoCoMo スコア無しで「Mem0 より上」の直接比較は困難
- **代替戦略**:
  - LongMemEval Knowledge Update + MemoryAgentBench Conflict Resolution の **二軸で supersession 優位を主張**
  - 業界標準 (LongMemEval) には準拠
  - Mayu 独自軸 (supersession 精度) で差別化

### MemoryAgentBench の Conflict Resolution の重要性

- 論文の知見: 「single-hop で 60%、multi-hop で 1 桁 %」 = **業界全体の弱点**
- Mayu supersession が直接対応する領域
- ICLR 2026 採択で業界注目度高
- MIT ライセンスで Mayu が **法的にクリーンに高スコア主張**できる可能性

### ConvoMem 論文主張の引用利用

- ConvoMem データは使えないが、論文主張は引用可能
- 「First 150 conversations don't need RAG」は Mayu の **「低 Tier 志向」と方向性一致**
- 独自ベンチでこの主張を再現する戦略もあり得る

---

## 批判的観点とリスク

### 1. ConvoMem 除外の規模インパクト

- 75K QA を失う、業界最大規模
- LongMemEval (500) + MemoryAgentBench (146) で計 **646 問**、規模は ConvoMem の 1/100
- カテゴリ網羅性では十分、規模では劣る
- 自作日本語ベンチで補完する必要

### 2. 業界トレンド追従の継続コスト

- 新作 benchmark が 3-6 ヶ月毎に登場
- ライセンス確認 + 統合コストが定期発生
- **対策**: review 頻度を年 1-2 回に固定、Tier S/A 候補のみ追従

### 3. MIT ライセンスの「素材問題」

- LongMemEval は MIT だが、構成素材 (ShareGPT, UltraChat) が異なるライセンス
- 著者が「release 自体は MIT」と明言しているが、解釈によってはリスク
- **対応**: LongMemEval を **そのまま利用** に留め、改変再配布しない

### 4. CC BY-SA 4.0 (HotpotQA) の境界

- 「読み込み利用」と「再配布」の区別が法的に微妙
- データ非配布運用なら問題ないが、CI ログにデータ部分が混入するとリスク
- **対応**: CI ログから raw data を除外、metric/score のみ保存

### 5. 著者直接確認の運用負担

- HaluMem, RealTalk, MemoryBench (清華) は著者にメールすれば license 確認可能
- ただしレスポンス遅延 (週単位)、英語/中国語コミュニケーション必要
- **対応**: コア (LongMemEval + MemoryAgentBench + RULER) で十分とみなし、追加調査は後回し

### 6. 評価結果の公開タイミング

- Mayu の差別化 (supersession) を訴求するには、いずれスコア公開が必要
- 公開前に gaming / 過適合の批判リスク
- **対応**: 第三者再現性を最優先、生成スクリプトと結果を全公開

### 7. 自作ベンチマークの正当性

- 「自分で作ったベンチで勝つのは当然」と見られがち
- ATANT (continuity benchmark) や MemoryAgentBench (CR 軸) を**参照することで第三者性を担保**
- 独自ベンチは補完軸として位置付け、業界標準を主軸にする

---

## 将来の検討事項

- **MSC のライセンス確認**: ParlAI フレームワーク経由のデータ層を個別確認
- **MemoryBench (清華) 著者連絡**: 11 データセット統合の価値が高いため license 確認の価値あり
- **HaluMem 代替の自作**: Provenance Hash Chain 検証用の独自 hallucination ベンチマーク設計
- **日本語ベンチマーク群の構築**: 既存ベンチが英語中心、日本語特化の独自データセット設計
- **MemoryBench (Supermemory) フレームワーク採用検討**: Provider 比較の標準ツールとして
- **Vendor lock-in 回避**: 評価環境を Mayu 内製に保ち、外部評価サービスに依存しない

---

## 関連ドキュメント

- [concept.md](./concept.md): プロダクトビジョンと設計原則
- [tech-architecture.md](./tech-architecture.md): 技術アーキテクチャ詳細
- [monetization-strategy.md](./monetization-strategy.md): 収益化戦略 (Tier 別 ablation の価格設計根拠)
- [runtime-environment.md](./runtime-environment.md): 実行環境

---

## 参考文献

- Wu et al. (2025). "LongMemEval: Benchmarking Chat Assistants on Long-Term Interactive Memory." ICLR 2025.
- Hu et al. (2025). "Evaluating Memory in LLM Agents via Incremental Multi-Turn Interactions." ICLR 2026 (採択).
- Hsieh et al. (2024). "RULER: What's the Real Context Size of Your Long-Context Language Models?" arXiv:2404.06654.
- Maharana et al. (2024). "Evaluating Very Long-Term Conversational Memory of LLM Agents." ACL 2024.
- Tang & Yang (2024). "MultiHop-RAG: Benchmarking Retrieval-Augmented Generation for Multi-Hop Queries." COLM 2024.
- Yang et al. (2018). "HotpotQA: A Dataset for Diverse, Explainable Multi-hop Question Answering." EMNLP 2018.
- Kočiský et al. (2018). "The NarrativeQA Reading Comprehension Challenge." TACL 2018.
- Salesforce AI Research (2025). "ConvoMem Benchmark: Why Your First 150 Conversations Don't Need RAG." arXiv:2511.10523.
- IAAR-Shanghai (2025). "HaluMem: Evaluating Hallucinations in Memory Systems of Agents." arXiv:2511.03506.
- Lee et al. (2025). "REALTALK: A 21-Day Real-World Dataset for Long-Term Conversation." arXiv:2502.13270.

---

*最終更新: 2026-04-25*
