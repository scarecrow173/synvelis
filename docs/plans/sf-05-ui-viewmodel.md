---
id: "PLAN-uivmPqRsTuVwXyZaBcDeFg"
type: "plan"
status: "approved"
title: "SF-05 ui/ slice 実施計画 — Signal Flow・Probe headless view-model consumer path"
created: "2026-11-18"
updated: "2026-11-18"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/108"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/sf-05-ui-viewmodel.md"
  implemented-by: []
  depends-on:
    - "docs/designs/sf-05-ui-viewmodel.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/plans/sf-05-slice3-probe-subscriber-wiring-plan.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/sf-05-ui-viewmodel.md"
    - "docs/designs/sf-05-ui-viewmodel.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/sf-05-ui-viewmodel.md"
  references: []
  defers: []
  deferred-by: []
---

# SF-05 ui/ slice 実施計画 — headless view-model consumer path

## Goal

service 側 `syn.signal_flow.*`/`syn.probe.*` op（main 統合済み）に
対する ui/ 側 typed consumer を、DOM-free view-model 2 本と
headless vitest で実装し、wire contract を consumer 層で固定する。

## Scope

- `ui/src/ipc.ts` の `OpName` 拡張（4 op 追加のみ）。
- `ui/src/viewmodel/signal-flow-view-model.ts` 新規。
- `ui/src/viewmodel/probe-view-model.ts` 新規。
- `ui/test/signal-flow-view-model.test.ts`・
  `ui/test/probe-view-model.test.ts` 新規。
- `ui/test/conformance.test.ts` の op リストへ新 7 op 追記（任意枠、
  registered op 網羅のため実施）。
- `evidence/sv108/` 本 slice 証拠（新規ファイル）。
- doc chain `sf-05-ui-viewmodel`。

## File Map

| ファイル | 変更 |
| --- | --- |
| `ui/src/ipc.ts` | `OpName` union に 4 op 追加 |
| `ui/src/viewmodel/signal-flow-view-model.ts` | 新規: view/visual/inspection/routing consumer |
| `ui/src/viewmodel/probe-view-model.ts` | 新規: control/state/compare consumer |
| `ui/test/signal-flow-view-model.test.ts` | 新規: wire 契約 spec |
| `ui/test/probe-view-model.test.ts` | 新規: wire 契約 spec |
| `ui/test/conformance.test.ts` | validOp op リストへ 7 op 追記 |
| `evidence/sv108/ui-viewmodel-verification.md` 他 | 新規証拠 |
| `docs/{specs,designs,plans,tasks,test-specs,impl/ir,impl/exp}/sf-05-ui-viewmodel.md` | 新規 doc chain |

## Dependency Graph

`ipc.ts` OpName → 両 VM → 両 test spec → conformance 追記 →
evidence → doc chain → index 再生成（最後）。

## Risk Register

| リスク | 対策 |
| --- | --- |
| Debug 語彙と serde kebab の混同 | `*Wire` 型を op 面ごとに分離し design に語彙表を明記 |
| `view_id` scoping 事故（session op への混入） | ProbeViewModel で view-scoped action を別 method 群に分離し `view_id` を constructor 必須化 |
| publish/active・requested/confirmed の畳み込み | response 型を verbatim union で保持し test で区別を assert |
| stale outcome の client 側 retarget | outcome を discriminated union のまま返し test で `stale` verbatim を assert |

## Verification

- `mise run check-ui`（tsc `--noEmit` + vendor sync check）
- `mise run test-ui`（vitest 全 suite）
- `mise run fmt-check`
- `mise exec -- prek run --all-files`
- `mise run build-ui` は views/app 未変更のため **NotRun/不要**。
- cargo レーン・`*-msrv`・`*-full`・実 WebView/GPUI・2-window・
  #109/#103 oracle join は範囲外 **NotRun**。

## Rollback

ui/ の新規ファイル削除 + `ipc.ts`/`conformance.test.ts` の差分
revert のみで完全に戻る。Rust・既存 VM・store/app には触れない。
