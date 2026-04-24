# つむぎ API 収益化戦略

## ステータス

- **著者**: @penta2himajin
- **初版**: 2026-04-24
- **ステータス**: Draft (提案)
- **関連**: [concept.md](./concept.md), [tech-architecture.md](./tech-architecture.md), [runtime-environment.md](./runtime-environment.md)

---

## 概要

本書は、つむぎを **公開 API およびマネージド SaaS** として提供する際の収益化戦略を定める。OSS (Apache 2.0) をコアとしつつ、Cloud 版で運用性と独自機能を提供する二層モデルを採る。

### 設計哲学

- **Bootstrap 前提**: 外部調達なしで持続可能な経済性を最優先
- **1 人の有料ユーザー獲得時点で黒字化** を実現可能な原価構造
- **OSS + マネージド Cloud** の二層モデル (Supabase / Redis / Graphiti 型)
- **BYOK (Bring Your Own Key) 標準**: embedding / LLM の変動費を顧客側に転嫁
- **Cloudflare エコシステム依拠**: 初期固定費を $1-5/月 に抑制
- **Hard Cap 明示**: 「無制限」表記を避け、粗利率の予測可能性を確保

### 非目標

- VC 調達前提の急成長
- Mem0 ($24M 調達) と同等速度での認知獲得
- 大規模営業体制 (Enterprise は機会次第)

---

## 原価構造分析

### 1 オペレーションあたりの原価 (2026-04 現在の Cloudflare + 主要 LLM API 価格に基づく)

| 操作 | 原価範囲 | 備考 |
|---|---|---|
| Chunk 追加 (text, D1 書込) | $0.0000003 | 無視可 |
| Embedding 生成 (OpenAI text-embedding-3-small, 500 tokens) | $0.00001 | BYOK で $0 |
| Embedding 生成 (Workers AI bge-small) | $0.0011 | non-BYOK 時のデフォルト候補 |
| Vectorize 書込 / クエリ | $0.00000004 | 無視可 |
| BM25 検索 (Tier 0, D1 FTS5) | $0 | 原価なし |
| セマンティック検索 (Tier 1) | $0.00001-0.001 | クエリ embedding が支配的 |
| Summary 生成 (Tier 2-3, GPT-4o-mini 1K+300 tokens) | $0.00033 | BYOK で $0 |
| Summary 生成 (Claude Haiku 4.5) | $0.0025 | BYOK で $0 |
| Supersession 自動判定 (LLM ベース, 2K+100 tokens) | $0.00036-$0.0025 | BYOK で $0 |
| 保存 D1 (1K chunks, ~5MB) | $0.0008/月 | 累積性、5GB 超で $0.75/GB/月 |
| 保存 Vectorize (1K vectors, 768 次元 float32) | $0.01/月 | 5M vectors まで Free |

### ユーザー種別ごとの月間原価見積

| 種別 | 月次消費プロファイル | 原価 (non-BYOK) | 原価 (BYOK) |
|---|---|---|---|
| **ライト** (試用) | 30 chunks, 100 search, 4 summary | ~$0.003 | ~$0.0001 |
| **アクティブ** (定常) | 500 chunks, 2K search, 30 summary, 20 supersession | ~$0.042 | ~$0.001 |
| **ヘビー** (プロ) | 5K chunks, 50K search, 500 summary, 500 supersession | ~$0.95 | ~$0.05 |
| **エクストリーム** | 100K chunks, 1M search, 10K summary, 1M chunks 累積 | ~$40 | ~$15 |

### 月間固定費 (つむぎ側運営コスト)

| 項目 | Free tier | Paid 到達条件 |
|---|---|---|
| Cloudflare Workers | $0 (100K req/日) | 500+ users で Paid $5/月 |
| D1 SQLite | $0 (5GB, 25M reads/日) | 大規模 chunks 累積で Paid $5/月 |
| Vectorize | $0 (5M vectors, 30M queries/月) | 大規模時 ~$10/月 |
| Workers AI (embedding) | $0 (10K neurons/日) | 従量 $0.011/1K neurons |
| ドメイン (.dev 等) | - | 約 $1/月 |
| **初期合計** | **$1/月** (ドメイン代のみ) | 500 users 付近まで Free tier 圏内 |

> **観察**: 初期の 1-500 ユーザーまでは **月固定費 $1 で運営可能**。Cloudflare Free tier の寛大さが、1 人黒字化戦略の土台となる。

---

## プラン設計

### Self-host (OSS / Apache 2.0)

- **価格**: $0
- 全機能、自己責任
- コア: Rust (`tsumugi-core`) + TypeScript SDK (`tsumugi-ts`)
- 対象: 技術力ある個人・企業、データ主権重視層

### Cloud Trial

- **価格**: $0 (7 日間限定)
- Chunks: 500 hard cap
- Summary: 10/月
- Rate: 30 req/min
- BYOK 必須
- 対象: 試用、評価

### Developer

- **価格**: **$9/月**
- Chunks: 10,000 hard cap
- Summary: 100/月 (hard cap)
- Supersession: 手動 API のみ (自動判定は Pro 以上の差別化機能)
- Rate: 100 req/min
- BYOK 必須 (embedding / LLM)
- Search: 無制限
- 対象: 個人開発者、小規模プロジェクト

### Pro

- **価格**: **$29/月**
- Chunks: 100,000 hard cap
- Summary: 2,000/月 (hard cap)
- Supersession 自動判定: 5,000/月
- Rate: 500 req/min
- Provenance hash chain: ✓
- Managed LLM option: +$10/月
- 対象: プロシューマー、SMB

### Business

- **価格**: **$99/月**
- Chunks: 1,000,000 hard cap
- Summary: Fair Use (100K/月以上は要相談)
- Actor-Aware Memory: ✓
- Provenance hash chain: ✓
- Managed LLM: 標準
- SLA: 99.5%
- Email サポート
- 対象: 中規模企業、マルチエージェント運用

### Enterprise

- **価格**: カスタム ($500+/月)
- オンプレミス option
- BYOK 選択可
- SOC 2 / HIPAA 準拠
- SLA 99.9%
- 専任サポート
- 対象: 規制業種 (金融 / 医療 / 法務)、大企業

---

## 使用量リミット設計

### 設計原則

リミットには **3 つの相反する目的** があり、バランスが必要:

1. **コスト防衛**: ヘビーユーザーによる原価爆発を防ぐ
2. **アップセル誘因**: 上位プランへの自然な動機付け
3. **UX 保護**: 正当な利用者の体験を損なわない

### リミット方式の選択

| リミット対象 | 方式 | 理由 |
|---|---|---|
| Summary / Supersession (LLM) | **Hard Cap** | 原価直撃、超過厳禁 |
| Storage (chunks 数) | **Hard Cap + Overage opt-in** | 累積性、超過は金銭解決 |
| Embedding (非 BYOK) | **Overage** | BYOK 推奨で回避可 |
| Request rate | **Throttle (429)** | 完全遮断より緩い |
| Search / Read 頻度 | 制限なし | 原価ほぼ無 |
| BM25 (Tier 0) | 制限なし | 原価 $0 |

### Overage 課金 (Opt-in)

デフォルトは **OFF**。ユーザーがダッシュボードで明示的に有効化した場合のみ課金対象。予期せぬ請求による信頼喪失を防ぐ。

| 項目 | Developer 超過料金 | Pro 超過料金 |
|---|---|---|
| Chunk (超過 1 つごと) | $0.002 | $0.001 |
| Summary (超過 1 回ごと) | - (Hard Cap、Pro 誘導) | $0.01 |
| Storage (GB 超過/月) | - | $2 |
| Rate limit 超過 | 429 (課金なし) | 429 (課金なし) |

### Power Law 対策

1 % のヘビーユーザーが 90 % のコストを食う典型パターンに対し、多層防御を敷く:

1. **Hard Cap による明示的上限**: 「無制限」表記は使わず、具体数値明示
2. **Batch API の 1 コール上限**: `max 100 chunks/call`
3. **異常利用検出**: 前週の 10 倍等の急増を自動検知、一時 throttle
4. **原価追跡ダッシュボード**: ユーザー / プラン別の消費を可視化、admin view

### 使用量情報の露出

レスポンスヘッダで常時表示:

```
X-Tsumugi-Usage-Chunks: 8123/10000 (81%)
X-Tsumugi-Usage-Summary: 45/100 (45%)
X-Tsumugi-Plan: developer
X-Tsumugi-Reset-At: 2026-05-01T00:00:00Z
```

Quota 超過時のエラーレスポンス:

```json
{
  "error": {
    "type": "quota_exceeded",
    "code": "chunks_limit_reached",
    "message": "Chunk limit (10,000) reached on Developer plan.",
    "limit": 10000,
    "current": 10000,
    "upgrade_url": "https://tsumugi.dev/pricing",
    "overage_available": true,
    "overage_price_per_chunk": 0.002,
    "request_id": "req_..."
  }
}
```

### 通知設計

- **80 % 到達**: Email 通知 (Resend API 経由)
- **100 % 到達**: Email + 次回利用時にレスポンスヘッダ警告
- **Overage 開始**: 日次サマリー Email

---

## 損益分岐シナリオ

### シナリオ A: 初期 (3-6 ヶ月)

- Developer × 3, Trial × 10
- 売上: $27/月 (ARR $324)
- 原価 + 固定費: $2/月
- **粗利: $25/月 (93%)** → 黒字

### シナリオ B: 早期拡大 (6-12 ヶ月)

- Developer × 20, Pro × 5
- 売上: $180 + $145 = $325/月 (ARR $3.9K)
- 原価 + 固定費: $15/月
- **粗利: $310/月 (95%)**

### シナリオ C: 成長期 (12-24 ヶ月)

- Developer × 100, Pro × 30, Business × 5
- 売上: $900 + $870 + $495 = $2,265/月 (ARR $27K)
- 原価 + 固定費: $80/月
- **粗利: $2,185/月 (96%)**

### シナリオ D: 安定成長 (24-36 ヶ月)

- Developer × 300, Pro × 100, Business × 20, Enterprise × 2
- 売上: $2,700 + $2,900 + $1,980 + $4,000 = $11,580/月 (ARR $139K)
- 原価 + 固定費: $500/月
- **粗利: $11,080/月 (95%)**

---

## 1 有料ユーザー黒字化の実現条件

以下 5 原則を守ることで、**最初の Developer $9 獲得時点で黒字化** が成立する:

### 1. BYOK 標準化

embedding / LLM の API キーを顧客持込とし、変動費をつむぎ側から除外する。つむぎは orchestration layer に徹する。

### 2. Cloudflare Free tier 最大活用

Workers 100K req/日、D1 5GB、Vectorize 5M vectors、Workers AI 10K neurons/日 の Free tier 内で 500 users まで運営する。月固定費はドメイン代 $1 のみ。

### 3. Tier 0 中心運用

BM25 + lindera (日本語) を主処理パスに据え、LLM 呼出は明示的オプトイン。Summary / Supersession 自動判定は Pro 以上の差別化機能として分離。

### 4. Free tier 厳格化

「無期限 Free」は **OSS self-host に限定**。Cloud Trial は 7 日 + 500 chunks の評価用途に徹する。Mem0 の無期限 10K Free に追随しない。

### 5. Self-host 積極推奨

技術力ある顧客は OSS self-host で無料運用可能とし、Cloud 版は運用負担軽減と独自機能 (Provenance / Actor-Aware / SLA) で差別化。「運用負担 vs ライセンス代」で自然に棲み分ける。

### 結果

- Developer $9 獲得時: 粗利 $7.50-8/月 = **83-89%**
- 20 users: MRR $300、粗利率 95%+
- 100 users: MRR $1K-2K、粗利率 95%+
- 500 users: Paid tier 移行 (MRR $5K+ で十分吸収)

---

## 実装フェーズ

### Phase α (Cloud launch 必須条件)

| 機能 | 実装コスト |
|---|---|
| Chunk 数 Hard Cap | D1 `SELECT COUNT(*)` + cached counter, ~2h |
| Rate limit throttle | Cloudflare Rate Limiting Rules (設定のみ), ~30min |
| Summary Hard Cap | monthly counter + check, ~4h |

**小計**: 半日〜1 日

### Phase β (3 ヶ月以内)

| 機能 | 実装コスト |
|---|---|
| 使用量ダッシュボード | Cloudflare Pages + SDK, 3-5 日 |
| 80% / 100% Email 通知 | Resend API 連携, 1 日 |
| Overage 課金 | Stripe Usage-based billing, 2-3 日 |

**小計**: 約 1 週間

### Phase γ (6 ヶ月以内)

| 機能 | 実装コスト |
|---|---|
| 異常利用検出 | D1 analytics + alert, 2 日 |
| 古い chunk 自動削除 (opt-in) | scheduled worker, 1 日 |
| R2 cold storage 移行 (Business 以上) | R2 integration, 2-3 日 |

**小計**: 約 1 週間

### 全体

**総計**: 2-3 週間 (週 10 時間ペースで 6-8 週)

---

## 批判的観点とリスク

### 1. Developer $9 の心理的安さ

- 日本円で約 1,350 円、サブスクとして「躊躇なく止める」価格帯
- Churn 率が高くなる可能性
- **代替**: $19 で Churn 率低下するが、1 人黒字化の実現は遅れる
- **判断**: 初期は裾野広げ優先で $9、製品成熟後に grandfather 付き値上げを検討

### 2. Hard Cap 体験の厳しさ

- Mem0 の Free tier 10K chunks と比較され不利になる可能性
- 「Mem0 で無料だったのになぜ?」という疑問への対処が必要
- **対策**: OSS self-host を積極推奨 + 独自機能 (Supersession / Provenance) での差別化訴求

### 3. BYOK 標準化の UX 摩擦

- 初心者には OpenAI / Anthropic API キー取得・設定のハードル
- Self-serve 体験に段差が生じる
- **対策**: Managed LLM option (+$10/月) を Pro 以上で提供、Developer は BYOK 限定

### 4. Power Law の盲点

- 想定外の使い方 (バッチ処理で一気に 100K summary) による瞬間的スパイク
- Hard Cap だけでは対応できない
- **対策**: Rate limit + 1-call 制限 + 日次 Summary 上限の多層防御

### 5. Supersession 自動判定の Plan locked

- 差別化機能を Pro に置くことで Developer 層への訴求弱まる
- 「安い memory API」としてのみ使われる可能性
- **対策**: Developer でも **手動 Supersession API** は開放 (原価 0)、自動化のみ Pro 差別化

### 6. Overage 請求の心理的抵抗

- 「予期せぬ請求」は開発者が最も嫌う
- Stripe usage-based billing の評判は両極
- **対策**: デフォルト OFF、明示的 opt-in、上限設定機能 (月 $50 超で強制停止等)

### 7. 成長速度の遅さ

- Mem0 ($24M 調達、48K GitHub stars) に対して Bootstrap は 1/10-1/20 の速度
- Dan Martell $10M ARR 目標とは整合しない
- **受容**: $50K-$500K ARR を堅実に到達する道としては健全。急成長を放棄する代わりに、最初から黒字を確保する

---

## 将来の検討事項

- **MCP registry 登録後の流入分析**: Phase α ローンチ後の実測に基づき Distribution 戦略を再調整
- **価格調整タイミング**: 製品成熟後の Developer $9 → $19 移行戦略 (既存契約の grandfather 保証)
- **Self-host → Cloud 変換率の最大化**: 運用負担の定量化、移行パス整備
- **業種特化プラン**: 規制業種 (金融 / 医療 / 法務) 向けコンプライアンス強化版の検討
- **独自 Supersession Benchmark**: ATANT 等を参考に業界標準化を狙う (LongMemEval と補完)
- **Actor-Aware Memory の具体仕様**: Business 以上の差別化機能として設計を詰める
- **Managed LLM provider の選定**: Workers AI / OpenAI / Anthropic のコスト比較と切替戦略

---

## 関連ドキュメント

- [concept.md](./concept.md): プロダクトビジョンと設計原則
- [tech-architecture.md](./tech-architecture.md): 技術アーキテクチャ詳細
- [runtime-environment.md](./runtime-environment.md): 実行環境

---

*最終更新: 2026-04-24*
