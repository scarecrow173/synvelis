---
id: "EXP-workspace-ux-scenarios-docchain"
type: "experiment"
status: "completed"
title: "Workspace UX 仕様正本化 — 既存契約再利用と文書チェーン生成アプローチ"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/3"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by:
    - "docs/impl/ir/workspace-ux-scenarios.md"
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/workspace-ux-scenarios-design.md"
    - "docs/designs/workspace-docking-state-architecture.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "docs/tasks/workspace-ux-scenarios-task.md"
  defers: []
  deferred-by: []
---

# Workspace UX 仕様正本化 — 既存契約再利用と文書チェーン生成アプローチ

## 背景と仮説

Issue #3 の UX 正本化は「既存の headless 契約（WS-A/ED-A・
統合設計）を再利用し、UX 操作面の規範だけを新規に書く」
アプローチが最も整合的と仮説を立てた。対抗案は UX 規範を
統合設計へ直接追記する案だったが、統合設計は論理構造・状態
正本・Command 契約の正本であり、ユーザー操作 UX を混在させると
責務境界（Issue #3「製品 UX の正本」vs Ruviform/統合設計の
基盤契約）が崩れるため棄却した。

## 観察・試行

- **既存契約の棚卸し**: `docs/designs/workspace-docking-state-
  architecture.md`・WS-A/ED-A spec/test-spec・SV-GPUI-03
  preset 契約・sv-gui-04 slice2 dock op 経路を読み、
  ViewInstanceId/WindowId/DockTree、Editing Selection/
  Inspection Focus/Linked Context の分離、Arrange/Edit 分離、
  `restore_or_default` の bounds 補正を確認。これらは全て
  headless で固定済みであり、UX 文書はそれらを前提参照する
  形にした。
- **design-doc 生成**: 初回生成でテンプレート断片の重複出力と、
  規範詳細セクション（旧 §4〜§9）が `## Data and Control
  Flow` 配下に `###`/`####` で誤ネストする構造問題が判明。
  見出しを `## 1.`〜`## 16.` のフラットな番号体系へ再構成し、
  内部 § 参照（§3.x→§4.x、§5.x→§6.x、§8.4→§9.4、§9.x→§10.x、
  §13→§15 等）を全件追従修正した。`doc_status lint` で
  front matter/relation の blocking finding がないことを確認。
- **review-protocol**: spec+design に Reviewer A（協調:
  要件写像・矛盾・実現可能性）/Reviewer B（敵対: エッジ・
  失敗モード・境界）の独立 pass を実施。Issue Candidates
  C1〜C6（Window 閉鎖の Undo 粒度未定義、Follow 元 View 喪失時
  の Inspector 挙動未定義、異 DPI のショートカット発火規則
  不在、空 Sub Window 自動閉鎖の根拠未記録、全 View 閉鎖時の
  アプリ終了挙動未定義、§6.2 の復帰経路の曖昧さ）を Validator
  で CONFIRMED（全て P2/P3、P0/P1 なし）と判定し、Judge で
  severity 確定。Worker 共通契約に基づき全件修正し、
  Decision Log D21〜D24 に確定化判断を記録。

## 結果

- 採用: 「既存契約を前提参照し UX 操作面のみ正本化」アプローチ。
  文書チェーン spec→design→plan→task→test-spec→ir/exp と
  `evidence/sv3/` 追跡表で Issue #3 §9 受入条件を網羅。
- review-protocol verdict: ACCEPT WITH NON-BLOCKING NOTES
  （指摘は全件解消済み）。spec/design status を `approved` に
  更新（Worker 共通契約の承認代替。人間承認の捏造ではない）。

## 残リスク

- 実機 lane（#114/#176）未実施のまま real-os 項目は NotRun。
  設計評価を実 OS Pass に換算しないことを PR・evidence に明記。
- repo 全体の `doc_status lint` baseline（122 blocking
  findings）と `doc-driven-dev-graph` route の `blocked`
  （focus-required）は本タスク範囲外の既存問題であり、
  新規文書起因の finding とは区別して記録した。
