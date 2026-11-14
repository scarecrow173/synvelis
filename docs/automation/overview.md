# Synvelis Automation 契約文書群（SV-AUTX-00）

Issue [#259](https://github.com/scarecrow173/synvelis/issues/259) の成果物。
Action / Atomic Macro / Workflow / Script / Headless CLI を横断する統合契約・
Owner 境界・依存 DAG の canonical 文書セットである。

本ディレクトリは `docs/` の canonical タイプ別ディレクトリではなく、
自動化サブシステムの契約文書を集約するプロダクト文書ディレクトリである。
各文書は相互に相対リンクで参照し、外部 Issue は URL で参照する。
`README.md` は `doc-maintenance` が生成する管理 index であり手編集しない
（本書はその手前にある概要・文書チェーンの案内である）。

## 文書チェーン

| 役割 | 文書 | 内容 |
| --- | --- | --- |
| Overview | overview.md（本書）/ README.md（生成 index） | 文書群の案内・索引 |
| Spec | [spec.md](spec.md) | 対象範囲・要件・受入条件・検証方法 |
| Design | [design-capability-action.md](design-capability-action.md) | Capability→Action 対応・Outcome/Error 契約 |
| Design | [design-atomic-macro.md](design-atomic-macro.md) | Atomic Macro vs Workflow 分離契約 |
| Design | [design-trust-boundary.md](design-trust-boundary.md) | CLI Trust/OS Boundary |
| Design | [design-workflow-run.md](design-workflow-run.md) | Plan/Run 状態遷移・Fault/Recovery Matrix |
| Design | [design-script.md](design-script.md) | Script 統合契約（YAML/Luau/Rhai） |
| Design | [design-owner-dag.md](design-owner-dag.md) | Owner/Dependency DAG・Case ID・非重複境界 |
| Plan | [plan.md](plan.md) | 実装計画・Slice・統合ゲート |
| Task | [tasks.md](tasks.md) | Owner Issue 別タスク分解 |

## 読み方

- 実装 Owner（AUTX-01〜08 / SV-SCRIPT-01〜04）は、自分の Issue に対応する
  契約節と [design-owner-dag.md](design-owner-dag.md) の Owner 表を先に読む。
- 契約の一次正本は `crates/synvelis-gateway/`（GW-01〜10 merge 済み）と
  `docs/specs/command-gateway.md`・`docs/designs/execution-plan-design.md`。
  本文書群はそれらを統合・投影するものであり、Gateway の第二正本を作らない。
- 本文書群は DESIGN-ONLY の成果物であり、実装コード・テストは含まない。
  実装の検証は各 Owner Issue の責務である（[spec.md](spec.md) §検証方法）。

## ステータス

| 文書 | status |
| --- | --- |
| spec.md | draft（review-protocol レビューで承認へ） |
| design-*.md | draft（同上） |
| plan.md / tasks.md | draft（同上） |
