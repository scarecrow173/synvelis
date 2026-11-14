---
id: "EXP-6su2AsIDpcBjtHGaGUycQj"
title: "Automation / Modulation 統合 Parameter Control 実験メモ (synvelis#6)"
created: "2026-11-14"
updated: "2026-11-14"
---

# 実験記録: Automation / Modulation 統合 Parameter Control（synvelis#6）

## E1 doc-driven-dev-graph router の状態

`route_graph.js --cwd . --json` を実行。`status: blocked`、
`current/next: probe`、`edgeId: null`。blocker は **既存の
repository-wide broken relation**（`docs/designs/rcd-*`・
`tim-02-*`・`vcs-*`・`docs/impl/ir/*` の deferred relation が
存在しない path を指す）であり、本 slice が新たに作ったものでは
ない。context-inspector（sv#5）先例と同様、この blocked は
slice 起因でないため、router が返す delegate が無いまま
`spec → design → plan → task → test-spec → impl` の文書チェーンを
規約どおり作成した。無関係な broken relation は **修復しない**
（範囲外）。exp に記録して後続の doc-status 修復 slice に委譲。

## E2 artifact ID の生成

`docs/specs/context-inspector.md` 等の先例に倣い、`SPEC-`/`DESIGN-`/
`PLAN-`/`TASK-`/`TSPEC-`/`IMPL-`/`EXP-` + 22 文字 Base62 の
artifact ID を生成。UUIDv7 相当の乱数を base62 化し `padStart(22)`。
生成値:

- `SPEC-2i1xSQAXTQNZIpUBQGPt15`
- `DESIGN-2PaoXTMFb4NbwT7TNBcUTc`
- `PLAN-41PTXXqDP41Hx7iG3WXLBl`
- `TASK-1DDemfS68i5F8Lkla2LRN9`
- `TSPEC-5CuediZdfo8z7uI9MvTAHB`
- `IMPL-49LTYDUL26uBLtxAQGXxOz`
- `EXP-6su2AsIDpcBjtHGaGUycQj`

## E3 設計上の主要判断（design D-01〜20 の根拠）

- **Scope を 2 軸化**: Issue #6 の歴史的 Scope 表をそのまま型に
  せず、CTL-01 #121 の `ControlOwner × AutomationTimeScope` と
  CTL-02 #122 の `ParameterTargetId` を正本として消費。旧表は
  表示名と明記（D-01/D-02）。
- **競合の明示解決**: last-edit-wins を採用せず
  `AutomationConflict`/`ConflictPolicy`（PC-A-03・AUT-02）を
  正本として参照（D-03）。
- **値の 7 層分離**: Base/Automation/Reference/Source 寄与/
  Host-resolved/Plugin-reported/Probe 実測を出所付きで分離し、
  観測不能を `Unavailable` とする（D-07）。Host-resolved を
  Plugin 内部最終値と同義にしない（PC-C3/C4）。
- **記録は pre-modulation**: LFO 動作中の Automation 記録で
  二重 Modulation を起こさないため Manual/Reference を記録
  （D-08・AM-10）。Bake は AUT-F05 #139 へ deferred。
- **数値閾値を捏造しない**: Touch Return・LOD・精度の数値は
  既存契約（AUT-01 暫定 150ms）を参照するか TIM/Ruviform へ
  委譲し、本設計で新規決定しない（D-16/D-20）。

## E4 証拠分類の方針

`evidence/sv6/coverage.md` で AM-01〜13 を既存 headless テストへ
写像。`Pass (headless)`（実証済み）/`Partial`（一部のみ）/
`NotRun`（設計のみ）/`Deferred`（GUI・実機 Gate）/
`Unsupported`（非対応明示）を分離し、Requirement 状態と実行結果を
混同しない。headless 成功を製品 Pass と換算しない（POLICIES.md）。

## E5 未解決・後続へ委譲

- 実装は #98〜103・#121/#122・#131〜134 が担当。本 slice は
  契約と証拠分類のみ。
- GUI/実機/実 Plugin 受入は #102/#114/#103 の別 Gate。
- repository-wide broken relation（router blocker）は本 slice
  範囲外。doc-status 修復は別途。
