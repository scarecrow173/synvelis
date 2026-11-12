---
id: "TASK-034WxmIMFAnvLuxIgBXqMT"
type: "task"
status: "done"
title: "PC-C6: TR02/TR06 dispatch ledger 実経路 oracle 再証拠化タスク"
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
    - "docs/plans/pc-c6-tr02-tr06-realpath-oracle-plan.md"
  implemented-by: []
  depends-on:
    - "docs/plans/pc-c6-tr02-tr06-realpath-oracle-plan.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/tasks/pc-c6-tr-ledger-oracle-task.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/pc-c6-tr02-tr06-realpath-oracle.md"
  references: []
  defers: []
  deferred-by: []
---

# PC-C6: TR02/TR06 dispatch ledger 実経路 oracle 再証拠化タスク

## Work

- T1 — `crates/synvelis-player/tests/pc_c6_oracle_tr_dispatch.rs`
  既存 file への `tr02_context_mismatch_dimensions_real_ledger` 追加
  （7 次元単独 mismatch + 全次元 mismatch の `ProjectRevision` 優先 +
  coherence 維持を独立 oracle と field 照合）
- T2 — 同 file への `tr06_silence_vs_unobserved_real_ledger` 追加
  （sibling target で `Silence`→`Current` と `Missing`/`Disabled`/
  `Lost`/未 touch→`NoEvidence` の区別、target 間漏洩なし）
- T3 — file 冒頭 doc comment を TR01〜TR08 全行対象に更新、
  canonical 9 コマンド検証、docs index 再生成、impl record

## Classification

- test-only slice / synvelis-player 層 headless oracle 実経路
  （Issue #103 の PC-C3-TR02/TR06 行の実経路残件）

## Upstream Documents

- plan: `docs/plans/pc-c6-tr02-tr06-realpath-oracle-plan.md`
- spec: `docs/specs/pc-c6-tr02-tr06-realpath-oracle.md`
- design: `docs/designs/pc-c6-tr02-tr06-realpath-oracle-design.md`
- adr: `docs/adr/pc-c6-tr02-tr06-realpath-oracle.md`
- test-spec: `docs/test-specs/pc-c6-tr02-tr06-realpath-oracle.md`

## Dependencies

- 前提: `origin/main` に直前 slice（PR#354、TR01/03/04/05/07/08）の
  `pc_c6_oracle_tr_dispatch.rs` rig・独立 oracle が merge 済みで
  あること。
- 参照のみ: `crates/synvelis-player/src/trace.rs`・
  `src/dispatch.rs`（契約本体・変更禁止）、
  `crates/synvelis-project/tests/pc_c6_oracle_tr.rs`（oracle 規則の
  先例・変更しない）、`crates/synvelis-player/tests/trace_c3_05.rs`
  （adapter 単体契約・変更しない）。

## Affected Files

- `crates/synvelis-player/tests/pc_c6_oracle_tr_dispatch.rs`
  （test 2 本追加 + doc comment 更新）
- `docs/` チェーン（specs/designs/plans/tasks/test-specs/adr の
  `pc-c6-tr02*`）+ `docs/impl/ir/pc-c6-tr02*`（impl record）
- 触れた dir の `docs/<type>/README.md` index（doc-maintenance
  `index apply` で再生成）

## Done When

- [x] TR02/TR06 が実経路 rig で生成した `ControlTrace` と oracle
  導出の期待値を field 単位で照合し全 pass（8 test）
- [x] `mise run fmt-check`/`check`/`lint`/`test`/`test-rt`/
  `check-app-native`/`lint-app-native`/`test-msrv`/`build` が全て
  exit 0
- [x] impl record + fixture matrix ledger の TR02/TR06 行を実測
  どおりに記録、残件が NotRun/Deferred 記録

## Verification

- 補助証拠: `cargo test -p synvelis-player --test
  pc_c6_oracle_tr_dispatch` 8 tests 全 pass、`mise run check` exit 0
- canonical: `mise run fmt-check` / `mise run check` /
  `mise run lint` / `mise run test` / `mise run test-rt` /
  `mise run check-app-native` / `mise run lint-app-native` /
  `mise run test-msrv` / `mise run build` — impl record に結果記録

## Evidence

- canonical（2026-11-13、`origin/main` `323d434` merge 済み HEAD、
  Windows）: `mise run fmt-check`/`check`/`lint`/`test`/`test-rt`/
  `check-app-native`/`lint-app-native`/`test-msrv`/`build` 全て
  exit 0。`pc_c6_oracle_tr_dispatch` 8 tests 全 pass（stable・
  MSRV 1.89.0 双方）。
- 補助証拠: `cargo test -p synvelis-player --test
  pc_c6_oracle_tr_dispatch` → 8 tests 全 pass、`mise run check`
  → exit 0
- impl record: `docs/impl/ir/pc-c6-tr02-tr06-realpath-oracle.md`
  （fixture matrix ledger TR02/TR06 Pass 記録・NotRun/Deferred
  記録済み）
- review-protocol: cooperative + adversarial の独立 2 pass、
  CONFIRMED blocker なし

## NotRun / Deferred（正直な記録）

- 実 GUI/実 VST3/実 CLAP/実 OS/実 RT/consumer 照合 — #114・#101・
  #102・Ruviform gate に委譲（本 slice 対象外、`NotRun` 継続）。
- GUI-PC-01〜04 scenario 対応表 — #114・sv112 着地後の別作業。
- PC-C1-R01〜08・PC-C4-PL01〜08・PC-C5-UI01〜07・
  PC-C6-COMP01〜05 — Issue の残件、別 slice。
