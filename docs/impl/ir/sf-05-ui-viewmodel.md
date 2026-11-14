---
id: "IMPL-uivmSf05UiViewModel001"
type: "impl"
status: "in-progress"
title: "SF-05 ui/ slice Signal Flow・Probe headless view-model consumer path"
created: "2026-11-18"
updated: "2026-11-18"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/108"
  changes:
    added:
      - type: "file"
        path: "docs/specs/sf-05-ui-viewmodel.md"
      - type: "file"
        path: "docs/designs/sf-05-ui-viewmodel.md"
      - type: "file"
        path: "docs/plans/sf-05-ui-viewmodel.md"
      - type: "file"
        path: "docs/tasks/sf-05-ui-viewmodel.md"
      - type: "file"
        path: "docs/test-specs/sf-05-ui-viewmodel.md"
      - type: "file"
        path: "docs/impl/ir/sf-05-ui-viewmodel.md"
      - type: "file"
        path: "docs/impl/exp/sf-05-ui-viewmodel.md"
      - type: "file"
        path: "ui/src/viewmodel/signal-flow-view-model.ts"
      - type: "file"
        path: "ui/src/viewmodel/probe-view-model.ts"
      - type: "file"
        path: "ui/test/signal-flow-view-model.test.ts"
      - type: "file"
        path: "ui/test/probe-view-model.test.ts"
      - type: "file"
        path: "evidence/sv108/ui-viewmodel-verification.md"
      - type: "file"
        path: "evidence/sv108/ui-viewmodel-coverage.md"
    modified:
      - type: "file"
        path: "ui/src/ipc.ts"
      - type: "file"
        path: "ui/test/conformance.test.ts"
    deleted: []
    renamed: []
    moved: []
    generated:
      - type: "file"
        path: "docs/specs/README.md"
      - type: "file"
        path: "docs/designs/README.md"
      - type: "file"
        path: "docs/plans/README.md"
      - type: "file"
        path: "docs/tasks/README.md"
      - type: "file"
        path: "docs/test-specs/README.md"
      - type: "file"
        path: "docs/impl/ir/README.md"
      - type: "file"
        path: "docs/impl/exp/README.md"
  implements:
    - "docs/tasks/sf-05-ui-viewmodel.md"
  implemented-by: []
  depends-on:
    - "docs/tasks/sf-05-ui-viewmodel.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/sf-05-ui-viewmodel.md"
    - "docs/designs/sf-05-ui-viewmodel.md"
    - "docs/plans/sf-05-ui-viewmodel.md"
    - "docs/test-specs/sf-05-ui-viewmodel.md"
    - "docs/impl/ir/sf-05-slice3-probe-subscriber-wiring-slice.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/plans/sf-05-ui-viewmodel.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/sf-05-ui-viewmodel.md"
  references: []
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted:
      - "docs/impl/exp/sf-05-ui-viewmodel.md"
    rejected: []
---

# SF-05 ui/ slice headless view-model consumer path

## Summary

`ui/src/ipc.ts` の `OpName` に `syn.signal_flow.view|visual|
inspection|routing` を追加し、`SignalFlowViewModel`・
`ProbeViewModel` の 2 つの DOM-free consumer を新設した。
`installFakeHost` harness 経由の vitest 2 spec で wire contract
（intent 完全一致・verbatim 写像・エラー経路・非捏造）を検証。
Rust・既存 VM・store/app/views には一切触れていない。

## Context

service 側 op は全て main 統合済み（`ui_ops.rs`）。本 slice の
GAP は ui/ consumer のみ。`syn.signal_flow.view` response は
`format!("{:?}")` の Debug 語彙、`syn.probe.*` は serde
kebab-case — 2 系統の語彙を `*Wire` 型で分離して保持した。

## Implementation

### Changes

- `ui/src/ipc.ts`: `OpName` union 末尾に 4 op 追加
  （`syn.probe.*` は既存のまま不変）。
- `signal-flow-view-model.ts`: `SignalFlowViewModel`。
  `loadView`（projection verbatim + `viewGeneration` token）、
  visual 8 method（ack `view_generation` で token 更新）、
  inspection 5 method（focus cache）、routing 6 method
  （`applied|cancelled|stale` outcome verbatim、`routingState`
  のみ gesture cache 更新）。
- `probe-view-model.ts`: `ProbeViewModel`。constructor で
  `viewId` 必須化。session-scoped 5 action は `view_id` 非送信、
  view-scoped 4 action は `view_id` 必須送信。`refreshState`・
  `compare` は response を verbatim 保持。
- `signal-flow-view-model.test.ts`/`probe-view-model.test.ts`:
  intent `toEqual` 完全一致・Debug/kebab 語彙 verbatim・
  publish/active・requested/confirmed 分離・`stale` 非 retarget・
  `engine_attached:false` honest empty・wire error reject・
  malformed `view_id` passthrough・初期非捏造を検証。
- `conformance.test.ts`: validOp op リストへ新 7 op 追記。

### Decisions During Implementation

- **class member 衝突の回避**: private method `inspection()` が
  public getter `inspection` と prototype 上で衝突し、getter が
  method を shadow して test が `[Function inspection]` を返した。
  `sendInspection` へ改名。`viewId` field/getter 衝突も
  `viewIdValue` field + `viewId` getter で解消（exp doc 参照）。
- **`routing` helper の generic 化**: `call<T>` を直接返す private
  generic method にし、public method ごとの cast を排除。
- **`view_id` 省略規約**: `SignalFlowViewModel` は `viewId`
  undefined 時に param 自体を送らない（`sf_view_param` の
  main-view fallback に委譲）。`ProbeViewModel` は view-scoped
  action が `view_id` 必須のため constructor required。
- **`gesture` cache の限定**: `routingState()` のみが gesture
  cache を更新し、他 routing op の response で cache を
  推測更新しない（wire が送らない状態を捏造しない原則）。
- **serde entity tag の PascalCase**: review で
  `SemanticSignalEntityRef` が `#[serde(tag, content)]` かつ
  `rename_all` 無しと判明し、`entity` tag は `"Track"` 等の
  PascalCase variant 名（boundary/kind/mode の kebab と同一
  payload 内で混在）。初版の lowercase 型・fixture を修正した。

## Related Experiments

### Adopted

- `docs/impl/exp/sf-05-ui-viewmodel.md` — class member 衝突の
  診断と generic helper 化の記録。

### Rejected

- なし（設計変更を要する分岐は発生しなかった）。

## Validation

| コマンド | 結果 |
| --- | --- |
| `mise run check-ui` | Pass（tsc `--noEmit` + vendor sync check） |
| `mise run test-ui` | Pass（8 files / 73 tests、うち新規 2 spec 13 tests） |
| `mise run fmt-check` | Pass |
| `mise exec -- prek run --all-files` | Pass |

`mise run build-ui` は views/app 未変更のため NotRun/不要。
cargo レーン・`*-msrv`・`*-full`・実 WebView/GPUI・2-window・
SF-UX-08/Windows L2・#109/#103 oracle join は範囲外 NotRun
（`evidence/sv108/ui-viewmodel-verification.md` 参照）。

## Risks

- VM は pull 型で push channel を持たない — view 変化の通知は
  consumer 側の refresh 責務（後続 GUI slice で設計）。
- `view_id` の `"view-{n}"` 形式検証は service 側に委譲 — VM は
  第二 validation を持たず wire reject をそのまま surface する。

## Follow-ups

- GUI mount slice（view-model を SolidJS surface へ接続）。
- #114 GUI gate・#109 oracle join（別 slice/Issue 管轄）。
