# SV-AUTX-00 実装計画（Plan）

対象: [spec.md](spec.md) の要件 R-1〜R-9。本計画は umbrella 契約確定後の
実装 Owner 群の進め方を定義する。AUTX-00 自体は実装を含まない。

## 1. Slice 構成

| Slice | Owner Issue | 依存 | 検証の性質 |
| --- | --- | --- | --- |
| S0 契約確定 | #259（本タスク） | — | 文書レビュー + index 整合 |
| S1 Action 投影 | #260 | S0 | fake adapter での catalog/outcome 契約試験 |
| S2 CLI issuer | #261 | S0 | adapter trust・Principal 分離の単体/契約試験 |
| S3 Workflow Plan | #264 | S0（契約） | plan lower/検証の単体試験 |
| S4 Coordinator | #265 | S3（真の Start Blocker） | fault matrix・resume の fixture 試験 |
| S5 Atomic Macro | #263 | S0・GW-04/05/06（merge 済み） | batch 境界・1 CAS/1 Undo の契約試験 |
| S6 CLI adapter | #262 | S2 の auth 受入（mutating 公開） | CLI↔Gateway 結合・機械可読出力 |
| S7 Script 基盤 | #271〜#274 | S3 の Plan 契約・RF#558/#559 PoC | typed port・drift 拒否・`--file` 面 |
| S8 GPUI UX | #266 | S1/S4 の mock/fixture 契約 | Mock/Fixture ベースの並行開発 |
| S9 E2E 受入 | #267 | S1〜S8 の統合 | MCP/CLI/GPUI 同一 Capability・独立 oracle |

## 2. 統合ゲート

- **Gate A（契約）**: 本 docs/automation/ 群の review-protocol 承認。
  以降の Owner Issue はこの契約を前提に実装する。
- **Gate B（並行着手）**: #260/#261/#264/#263 は fixture/mock で並行可。
- **Gate C（mutating CLI 解禁）**: #261 auth 受入をもって #262 の
  mutating surface を公開可。それまでは read-only/fixture のみ。
- **Gate D（統合受入）**: #267 で GUI/MCP/CLI/AI の同一 Capability・
  独立 Policy/Commit oracle を確認。headless/fake 成功を実 OS 証拠と
  みなさない（POLICIES.md 準拠）。

## 3. 検証戦略（POLICIES.md §5 準拠）

- 各実装 Owner は変更 crate に比例した `mise run check/test/lint <pkg>`
  を実施。`*-full`・3OS 横断・長時間回帰は Daily/Manual Full CI に委譲。
- OS-L1 は全 Required lane で必須。GUI-L2/EXT-L3 は実際に提供・使用する
  GUI/外部 device/plugin 機能がある lane のみ Required、なければ Deferred。
- macOS/Linux OS-L1 は現時点で Required、GUI-L2/EXT-L3 は Deferred
  （使用要件発生まで）。
- 本タスク（S0）は文書のみ: Markdown/リンク軽量チェック + doc index 整合。
  `mise run check`/`test`/`gw10`/`test-rt` の実行は各実装 Owner の責務で
  あり、本タスクでは NotRun（契約照合のみ）。

## 4. リスクと対策

| リスク | 対策 |
| --- | --- |
| 第二 Registry/Writer/Job の混入 | GW-R-13 + 本 DAG §3 の照合表を各 PR のレビュー観点にする |
| Script が authority を運ぶ設計への退行 | design-script §5 を script 系 Issue の acceptance に明記 |
| Workflow が Project/Job 正本を複写 | design-workflow-run §1 の所有表を #265 の review gate にする |
| Fault Matrix の未検証項目を Pass 扱い | case ID registry で NotRun/Unsupported を分離記録 |
| Ruviform PoC を製品サポートと誤認 | RF#559 を「PoC 限定」として `backend_requirement` で fail-closed |
