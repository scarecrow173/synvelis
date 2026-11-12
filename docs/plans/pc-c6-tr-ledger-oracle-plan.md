---
id: "PLAN-034W1XlCxVQQ7sFeO9N9c0"
type: "plan"
status: "approved"
title: "PC-C6 TR ledger oracle 実装計画"
created: "2026-11-12"
updated: "2026-11-12"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/103"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/pc-c6-tr-ledger-oracle.md"
    - "docs/designs/pc-c6-tr-ledger-oracle-design.md"
  implemented-by: []
  depends-on:
    - "docs/specs/pc-c6-tr-ledger-oracle.md"
    - "docs/designs/pc-c6-tr-ledger-oracle-design.md"
    - "docs/adr/pc-c6-tr-ledger-oracle.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/pc-c6-tr-ledger-oracle.md"
    - "docs/designs/pc-c6-tr-ledger-oracle-design.md"
    - "docs/adr/pc-c6-tr-ledger-oracle.md"
  derived-by: []
  verifies:
    - "docs/test-specs/pc-c6-tr-ledger-oracle.md"
  verified-by:
    - "docs/test-specs/pc-c6-tr-ledger-oracle.md"
  references: []
  defers: []
  deferred-by: []
---

# PC-C6 TR ledger oracle 実装計画 Implementation Plan

## Goal

`synvelis-player` に TR01/03/04/05/07/08 を dispatch-ledger 実経路で
再証拠化する結合 test を追加し、project 層 `NotRun` の残件を閉じる。
`src/`・他 crate・Cargo 設定は変更しない。

## Scope

- In: `crates/synvelis-player/tests/pc_c6_oracle_tr_dispatch.rs`
  （新規）、docs チェーン、impl record。docs index は Coordinator
  処理のため `--no-index` 生成。
- Out: spec の Out of Scope と同一。

## Upstream Documents

- spec: `docs/specs/pc-c6-tr-ledger-oracle.md`
- adr: `docs/adr/pc-c6-tr-ledger-oracle.md`
- design: `docs/designs/pc-c6-tr-ledger-oracle-design.md`
- test-spec: `docs/test-specs/pc-c6-tr-ledger-oracle.md`

## Design Inputs

- 実経路 rig（`host_plugin`/`fabricated_adapter`/`ledger_rig`）+
  test 内独立 oracle（`oracle_context_stale`/`oracle_stamp_freshness`/
  `oracle_channel_freshness`/`oracle_freshness`/`oracle_is_coherent`/
  `assert_trace_agrees`）— design doc が正本。
- 先例: `crates/synvelis-player/tests/trace_c3_05.rs`（fake plugin +
  `GraphRuntime` receipt 経路 rig）、
  `crates/synvelis-project/tests/pc_c6_oracle_tr.rs`（独立分類 oracle
  の規則）。

## File Map

| file | 変更 |
| --- | --- |
| `crates/synvelis-player/tests/pc_c6_oracle_tr_dispatch.rs` | 新規 — TR01/03/04/05/07/08 実経路 oracle |
| `docs/impl/ir/pc-c6-tr-ledger-oracle.md` | 実装記録（完了時） |
| `docs/*/README.md` | 生成 index は Coordinator が統合時に処理（本 slice では触れない） |

## Implementation Sequence

- [x] rig 基盤: `host_plugin` + `fabricated_adapter` + `ledger_rig`
      （fake plugin attach・graph compile/publish・`RtRuntime` 駆動・
      `observe_cycle` span/call window）+ oracle 導出関数群。
- [x] TR01 — stage 独立性（ledger 経路 receipt + `record_configured`/
      `record_probe` の slot 独立更新）。
- [x] TR03 — context 変更 stale + 実 2 回目 dispatch の re-key。
- [x] TR04 — `record_reported`/`mark_reporting_unavailable` の終端
      分類と probe channel 独立性。
- [x] TR05 — `plugin_reported` stamp provenance（Estimated 固定・
      Current 不可・age 超過 Stale{Age}・ClockUntrusted）。
- [x] TR07 — 同一 `ProbeObservation` を 2 context で記録し freshness
      が trace 個別であること。
- [x] TR08 — bounded retention（fabricated 経路）metrics/event
      会計の独立導出値照合。
- [x] canonical 検証実行 + impl record。

## Dependencies and Risks

- `src/`・他 crate・manifest・mise・CI は変更しない。
- oracle と実装の差異が出た場合は実装を oracle に合わせて変更せず、
  契約解釈を spec/design/test-spec に立ち返って確認する。
- TR08 の会計値は adapter 内部の event 生成順に依存する（design
  Risks 参照）— 内部変更は test fail として顕在化させる。
- `GraphRuntime` 経路は決定的 headless で実時間に依存しない。

## Task Breakdown

- T1: rig + oracle 基盤 + TR01/TR03/TR04/TR05/TR07（ledger 経路）
- T2: TR08（fabricated 経路・bounded retention）
- T3: 最終検証 + impl record

## Verification

- [ ] `mise run fmt-check` / `mise run check` / `mise run lint` /
      `mise run test` / `mise run test-rt` /
      `mise run check-app-native` / `mise run lint-app-native` /
      `mise run test-msrv` / `mise run build` — 全 exit status を
      impl record に記録。
- [ ] test-spec `docs/test-specs/pc-c6-tr-ledger-oracle.md` の各
      Feature が fixture で担保されること。

## Open Questions / Gaps

- なし。

## Self-Review

- scope は spec の In Scope と一致。`src/`・他 crate・Cargo 設定への
  変更なし。oracle 独立性は design/ADR の決定どおり。

## Implementation Handoff

- Recommended mode: inline execution（dispatched worker）。
- Independent work streams: none。
- User approval required before dispatch: no（worker spec で実装
  済み）。
- Executed task-by-task with verification checkpoints。

## Review Handoff

- TR08 の会計値の手計算根拠を impl record に記録すること。
- headless/fake の証拠境界（実 GUI/plugin/OS/RT/consumer は #114・
  Ruviform gate に委譲、`NotRun` 記録）を PR/impl record で明示する
  こと。
