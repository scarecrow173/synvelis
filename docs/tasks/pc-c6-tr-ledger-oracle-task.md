---
id: "TASK-034W1XvJrhZ2i1eRhcbxkP"
type: "task"
status: "done"
title: "PC-C6: TR01/03/04/05/07/08 dispatch ledger 実経路 oracle 結合タスク"
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
    - "docs/plans/pc-c6-tr-ledger-oracle-plan.md"
  implemented-by: []
  depends-on:
    - "docs/plans/pc-c6-tr-ledger-oracle-plan.md"
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
    - "docs/plans/pc-c6-tr-ledger-oracle-plan.md"
  derived-by: []
  verifies:
    - "docs/test-specs/pc-c6-tr-ledger-oracle.md"
  verified-by:
    - "docs/test-specs/pc-c6-tr-ledger-oracle.md"
  references: []
  defers: []
  deferred-by: []
---

# PC-C6: TR01/03/04/05/07/08 dispatch ledger 実経路 oracle 結合タスク

## Work

- T1 — `crates/synvelis-player/tests/pc_c6_oracle_tr_dispatch.rs`
  （新規）: `host_plugin` + `fabricated_adapter` + `ledger_rig` +
  test 内独立 oracle + TR01/03/04/05/07（ledger 経路）
- T2 — 同ファイル: TR08（fabricated 経路・bounded retention metrics
  会計）
- T3 — 最終検証（canonical 9 コマンド）+ impl record

## Classification

- test-only slice / synvelis-player 層 headless oracle 実経路
  （Issue #103 の PC-C3-TR01/03/04/05/07/08 行の実経路残件）

## Upstream Documents

- plan: `docs/plans/pc-c6-tr-ledger-oracle-plan.md`
- spec: `docs/specs/pc-c6-tr-ledger-oracle.md`
- design: `docs/designs/pc-c6-tr-ledger-oracle-design.md`
- adr: `docs/adr/pc-c6-tr-ledger-oracle.md`
- test-spec: `docs/test-specs/pc-c6-tr-ledger-oracle.md`

## Dependencies

- 前提: `origin/main` に TRACE-C3-05（`DispatchTraceAdapter`/
  `ParamDispatch`/`trace_c3_05.rs`）が merge 済みであること。
- 参照のみ: `crates/synvelis-player/src/trace.rs`・
  `src/dispatch.rs`（契約本体・変更禁止）、
  `crates/synvelis-project/tests/pc_c6_oracle_tr.rs`（oracle 規則の
  先例・変更しない）、`crates/synvelis-player/tests/trace_c3_05.rs`
  （rig 先例・変更しない）。

## Affected Files

- `crates/synvelis-player/tests/pc_c6_oracle_tr_dispatch.rs`（新規）
- `docs/` チェーン（specs/designs/plans/tasks/test-specs/adr）+
  `docs/impl/ir/`（impl record）
- `docs/*/README.md` は Coordinator が統合時に処理（本 slice では
  変更しない）

## Done When

- [x] TR01/03/04/05/07/08 が実経路 rig で生成した `ControlTrace` /
  `TraceMetrics` と oracle 導出の期待値を field 単位で照合し全 pass
- [x] `mise run fmt-check`/`check`/`lint`/`test`/`test-rt`/
  `check-app-native`/`lint-app-native`/`test-msrv`/`build` が全て
  exit 0
- [x] impl record + TR08 会計根拠記録済み、残件が NotRun/Deferred
  記録

## Verification

- canonical: `mise run fmt-check` / `mise run check` /
  `mise run lint` / `mise run test` / `mise run test-rt` /
  `mise run check-app-native` / `mise run lint-app-native` /
  `mise run test-msrv` / `mise run build` — 全て exit 0（2026-11-12）
- 補助証拠: `cargo test -p synvelis-player --test
  pc_c6_oracle_tr_dispatch` 6 tests 全 pass、MSRV（rust 1.89.0）
  でも同 test 全 pass

## Evidence（2026-11-12）

- `mise run fmt-check` → exit 0
- `mise run check` → exit 0
- `mise run lint` → exit 0
- `mise run test` → exit 0（`pc_c6_oracle_tr_dispatch` 6 tests 全
  pass: tr01/tr03/tr04/tr05/tr07/tr08）
- `mise run test-rt` → exit 0
- `mise run check-app-native` → exit 0
- `mise run lint-app-native` → exit 0
- `mise run test-msrv` → exit 0（rust 1.89.0 toolchain、
  `pc_c6_oracle_tr_dispatch` 6 tests 全 pass）
- `mise run build` → exit 0
- impl record: `docs/impl/ir/pc-c6-tr-ledger-oracle.md`（TR08 会計
  根拠・NotRun/Deferred 記録済み）

## NotRun / Deferred（正直な記録）

- 実 GUI/実 VST3/実 CLAP/実 OS/実 RT/consumer 照合 — #114 gate・
  Ruviform consumer gate に委譲（本 slice 対象外）。
- GUI-PC-01〜04 scenario 対応表 — #114・sv112 着地後の別作業。
- TR02/TR06 の実経路再証拠 — project 層で証拠化済み、別 slice。
- PC-C1-R01〜08・PC-C4-PL01〜08・PC-C5-UI01〜07・
  PC-C6-COMP01〜05 — Issue の残件、別 slice。
