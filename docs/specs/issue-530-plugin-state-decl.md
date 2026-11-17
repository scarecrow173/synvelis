---
id: "SPEC-8C2JgN39wTW9v4aw04Cg"
type: "spec"
status: "approved"
title: "Issue #530 spec — stateless plugin を PluginStateDecl で表現する契約"
created: "2026-11-19"
updated: "2026-11-19"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/530"
    - "docs/discovery/issue-530-plugin-state-decl.md"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on:
    - "docs/discovery/issue-530-plugin-state-decl.md"
    - "docs/adr/issue-530-plugin-state-decl.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/issue-530-plugin-state-decl.md"
    - "evidence/sv530/decision-ledger.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/discovery/issue-530-plugin-state-decl.md"
  derived-by:
    - "docs/designs/issue-530-plugin-state-decl.md"
    - "docs/plans/issue-530-plugin-state-decl.md"
  verifies: []
  verified-by: []
  references: []
---

# Issue #530 spec — stateless plugin を PluginStateDecl で表現する契約

## 目的

state save/restore を実装しない stateless plugin を
`PluginCapabilitySnapshot` 経由で `PluginVerdict::Live` 判定できる
契約を `synvelis-player::rcp` に定義し、`digest: None` の
「stateless by design」と「未 capture」を型上区別可能にする。
未 capture（capture すべきだが無い）は引き続き fail-closed を維持
する。

## 受入条件（Issue #530 転記 + Coordinator 確定）

- [ ] stateless plugin（state save/restore 非実装）を
      `PluginCapabilitySnapshot` 経由で Live 判定できる契約が
      定義される
- [ ] `digest: None` の「未 capture」と「stateless by design」が
      型上区別可能になる
- [ ] 未 capture（capture すべきだが無い）状態は引き続き
      fail-closed（`StateUnknown`/`StateUnsupported` 等）を維持する
- [ ] `synvelis-player` の既存 `rcp_04` oracle・
      `synvelis-workspace` の `rcp_selection_integration` suite が
      新契約で pass する
- [ ] `mise run check/lint/test synvelis-player`・
      `mise run check/lint/test synvelis-workspace`・
      `mise run fmt-check` が pass する

## 契約要件（design/ADR で確定する判定表）

| env `state_restore` | decl state | verdict |
| --- | --- | --- |
| `Verified` | Known + digest Some + exact_eligible | `Live`（既存） |
| `Verified` | 上記以外（digest None 含む） | `StateUnknown`（既存・fail-closed 維持） |
| `Unsupported` | stateless profile（digest None + quality `Unsupported` + status ≠ `Known`） | **`Live`（新規）** |
| `Unsupported` | 上記以外 | `StateUnsupported`（既存） |
| `Unknown` | 任意 | `StateUnknown`（既存・fail-closed 維持） |

## 非目標

- `SessionRoutingEngine` への selection 配線（sv#514 で「実装不要」
  確定済み — 再燃させない）
- 実 plugin での state capture 実装（TIM-04 領域）・実機受入
  （sv#296 cross-gate — NotRun 記録）
- GUI 表示面（sv#295）
- `synvelis-project` の variant/domain adapter 契約
  （`StateCaptureQuality`/`OpaqueStateStatus` は read-only consume）
- `PluginVerdict`/`StateRestoreSupport`/`PluginStateDecl` への
  variant/field 追加を伴う設計（lease 外波及を避ける — design/ADR
  で却下根拠を記録）
