---
id: "PLAN-034V2wD3RqGEqlst7HFeVD"
type: "plan"
status: "approved"
title: "SF-06 cross-gate headless slice — Domain Oracle・Case registry・fault fixture・ledger"
created: "2026-11-10"
updated: "2026-11-10"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/109"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/sf-06-cross-gate-oracle-case-ledger.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/adr/sf-06-cross-gate-domain-oracle-case-id-evidence-ledger-headless.md"
    - "docs/test-specs/sf-06-cross-gate-oracle.md"
    - "docs/tasks/sf-06-cross-gate-oracle-headless.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/sf-06-cross-gate-oracle-case-ledger.md"
    - "docs/designs/sf-06-cross-gate-oracle-domain-evidence.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/sf-06-cross-gate-oracle.md"
  references: []
  defers: []
  deferred-by: []
---

# SF-06 cross-gate headless slice — Domain Oracle・Case registry・fault fixture・ledger Implementation Plan

## Goal

Issue #109 の headless slice として、SF-01〜SF-05 を横断する
独立 Domain Oracle・Case ID registry・fault fixture・
evidence ledger を `synvelis-project` の test 層と
`evidence/reports/` に実装する。

## Scope

単一の実装ストリーム。test/fixture/evidence/docs のみで、
`src/` 配下の実装本体（#106/#107/#108 並行実装領域を含む）
には触れない。

## Upstream Documents

- `docs/specs/sf-06-cross-gate-oracle-case-ledger.md`
- `docs/adr/sf-06-cross-gate-domain-oracle-case-id-evidence-ledger-headless.md`

## Design Inputs

- `docs/designs/sf-06-cross-gate-oracle-domain-evidence.md`

## File Map

- `crates/synvelis-project/tests/sf06_oracle.rs` — 独立 oracle
  （`AudibilityOracle`・`routing_invariant_violations`）・
  fixture topology（`build_mix_project`・`build_chain_project`）・
  sweep/fault/migration test 6 件。
- `evidence/reports/sf-06-cross-gate-ledger.md` — Case ID
  registry + lane 別結果 ledger（Issue #109 schema）。
- `docs/specs|adr|designs|plans|test-specs|tasks|impl/ir/
  sf-06-*.md` — docs チェーン。

## Implementation Sequence

- [x] fixture helper（controller/cmd/add_track/add_track_strip/
  add_bus/connect_main/add_send/engage_bool_targets）を
  `sf_02.rs`/`routing_sf01.rs` の慣例に合わせて実装。
- [x] `AudibilityOracle` — persisted 生フィールドから
  port→strip 解決・mute/solo 抽出・enabled edge・BFS・
  documented policy を独立再実装。
- [x] `routing_invariant_violations` — persisted 契約の独立
  再検査（map key==id・master 一意・owner 解決・方向・
  send↔connection 1:1・非循環・mute/solo 単一権威）。
- [x] fixture topology 2 系統 + exhaustive mute×solo sweep。
- [x] fault matrix（stale CAS・stale capability・cycle・
  dup temp・illegal kind/map・referenced delete → Live/
  Revision/Undo 不変 + 事後 undo の正確性）。
- [x] v5→v6 migration oracle（宣言 fixture から期待投影を導出、
  決定性・save head 更新を検証）。
- [x] oracle 自己検証（corruption fixture で非空 violation）。
- [x] `evidence/reports/sf-06-cross-gate-ledger.md` —
  Case ID registry + lane 別結果（headless Pass・実機/
  consumer NotRun）。
- [x] docs チェーン + 索引再生成。

## Dependencies and Risks

- #104 Routing 契約（merged）に依存。#106/#107/#108 は並行実装中
  のため本 slice は persisted 層のみを対象とし、それらの
  実装確定を待たない。
- oracle が SUT と同じ誤読を共有するリスク → documented
  policy のみを根拠にし、corruption fixture で検出能力を
  自己検証。
- #106/#107/#108 の契約変更で本 test が陳腐化するリスク →
  ledger の `fixture & command`/`expected vs observed` 列で
  再評価点を明示。

## Task Breakdown

- `docs/tasks/sf-06-cross-gate-oracle-headless.md` — 本 slice
  全体を 1 task として管理（test 6 件 + ledger + docs は
  一つの検証可能な成果物）。

## Verification

- [x] `cargo test -p synvelis-project --test sf06_oracle`
  exit 0（6 test 全 Pass）。
- [ ] `mise run fmt-check` exit 0。
- [ ] `mise run check` exit 0。
- [ ] `mise run lint` exit 0。
- [ ] `mise run test` exit 0。
- [ ] `mise run test-rt` exit 0。
- [ ] docs 索引が doc-maintenance preview→apply で再生成済み。

## Open Questions / Gaps

- SF-ACT/PROBE/UX/COMPAT の runtime・GUI・device・consumer
  lane は本 slice では `NotRun` — #106/#107/#108 merge と
  #114/#176/#219/#294 gate 完了後に ledger を更新する。
- Ruviform resolved SHA は `Cargo.lock` の git revision を
  ledger に記録（`0f5b5737`）。

## Self-Review

- [x] Every spec requirement maps to a task.
- [x] No placeholders remain.
- [x] File paths, relations, status values, and terminology are
  consistent.

## Implementation Handoff

- Recommended mode: inline execution（worker dispatch、
  subagent なし）。
- Independent work streams: none。
- User approval required before dispatch: yes（orchestration
  dispatch による承認済み）。
- If approved: discover available implementation and delegation
  capabilities in the current environment, then execute
  task-by-task with review checkpoints.
- If declined or unavailable: execute inline using the same
  dependency graph and verification matrix.

## Review Handoff

- reviewer focus: oracle の独立性（SUT 関数を呼んでいないか）、
  fault matrix の「reject→状態不変」の網羅性、ledger の
  NotRun/Unsupported の正直さ、headless Pass の昇格がないこと。
