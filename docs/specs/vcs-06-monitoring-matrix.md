---
id: "SPEC-pkRnrZ2RKJfPW1ymwMSENS"
type: "spec"
status: "approved"
title: "VCS-06 monitoring-domain matrix — monitor 宣言の diff/apply/restore 証拠行"
created: "2026-11-11"
updated: "2026-11-11"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/97"
    - "SPEC-vcs06-fixture-fault-matrix"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/rcd-03-monitor-persistence.md"
    - "docs/specs/rcd-03-vcs-diff-monitor-classification.md"
  refines: []
  refined-by: []
  derives-from:
    - "https://github.com/scarecrow173/synvelis/issues/97"
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# VCS-06 monitoring-domain matrix — monitor 宣言の diff/apply/restore 証拠行

## What Should Be Built

`crates/synvelis-project/tests/vcs06_monitoring_matrix.rs`（新規・
test のみ）を追加し、PR #318（`a218944b`）で VCS semantic diff に
分類追加された monitor 宣言（`ObjectKind::TrackMonitorDecl` /
`ObjectKind::PunchMarks`、`ComparisonDomain::Recording`、
`DIFF_POLICY_VERSION = 2`）について、VCS-06 の固定 Case ID 付き
証拠行を `evidence/reports/vcs-06-fixture-matrix-ledger.md` へ
追加する。

- monitor 宣言の add / modify / clear が typed change として
  区別されること（`Unknown` を `Equal` にしない方針と整合）。
- `apply_selected` の monitoring kind に対する実挙動を実測して
  記録する（現行 apply whitelist に含まれないため、期待実測は
  `Unsupported` fail-closed — 対応と偽装しない）。
- restore-as-copy で `project.monitoring` が verbatim round-trip
  することを検証する。

## Why Now

VCS-06 fixture matrix（PR #268 系）は diff policy v1（
`SCHEMA_VERSION = 11` 時代）で書かれており、monitoring 系
`ObjectKind` の Case が ledger にゼロ。RCD-03 永続化 spec が
明示的に follow-up として defer した部分
（`docs/specs/rcd-03-monitor-persistence.md` Deferred Design
Concerns・`docs/impl/ir/rcd-03-monitor-persistence-slice.md`）。
Issue #97 は統合受入 ledger Issue であり、coverage gap を実測
証拠で埋める必要がある。

## Users and Value

- Issue #97 の受入担当者: monitoring-domain の diff/apply/restore
  行を個別 Case ID で追跡できる。
- 将来の apply surface 拡張担当者: `Unsupported` fail-closed の
  実測行が、monitoring kind を apply surface に載せる slice の
  先行条件と境界を明示する。

## Scope

### In Scope

- `crates/synvelis-project/tests/vcs06_monitoring_matrix.rs`
  （新規・test のみ）
- `evidence/reports/vcs-06-fixture-matrix-ledger.md` の新規 Case
  行追加（VCS-DIFF-09/10・VCS-APPLY-07・VCS-RESTORE-07）と
  Environment ブロックの現在値への更新
- 本 spec / design / plan / task / test-spec / impl-ir チェーン
- `docs/*/README.md` 管理 index の再生成

### Out of Scope

- `crates/*/src/` の変更（公開 API で観測可能な範囲のみ。apply
  が monitor kind 未対応なら `Unsupported` fail-closed を実測
  として記録する。src 変更が必要と判明した場合は blocked 報告）
- apply surface への monitoring kind 追加（transplant payload・
  cascade・preflight 設計を要する別 slice）
- `vcs02_diff.rs` の既存 monitor 分類 fixture の重複再実装
  （分類規約の一次証拠は vcs02 に既にある。本 matrix は ledger
  用の独立実測行として Case ID 粒度で記録する）
- `*-REAL-*` 行・実 GUI・consumer gate・VCS Writer / 3-way merge
  ・他 OS・Issue #97 の Close

## Requirements

- `SetTrackMonitor`（upsert）→ `TrackMonitorDecl/Added`、宣言の
  field 変更 → `Modified`（field evidence 付き）、
  `ClearTrackMonitor` → `Removed` が typed change として区別
  される。monitor 宣言は `Recording` domain に写像され、owner
  track 本体へ重複 change を出さない。
- `SetPunchMarks` の declaration-presence 分類:
  `{None,None}`→宣言 = `Added`（有効 `[in,out)` なら
  `affected_range = SemanticTimeRange::Musical`）、宣言→宣言
  （mark 変更）= `Modified`、宣言→`{None,None}` = `Removed`
  （消えた宣言の covered span を報告）。
- `target_fingerprint` および `apply_selected` が monitoring
  kind を含む selection に対して `ApplyError::Unsupported`
  fail-closed で失敗し、live project を変更しないことを実測
  する（対応と偽装しない）。
- `restore_as_copy` が publish した bundle の snapshot envelope
  を読み戻し、`project.monitoring`（`monitoring.tracks` +
  `monitoring.punch`）が source project と verbatim 一致する
  ことを検証する（fork の `project_id` 差分のみ）。
- ledger の各行は実測値のみ記録し、既存行フォーマット
  （`Synvelis commit | Cargo.lock Ruviform SHA | OS/FS/Git/Plugin |
  Fixture/Capability | Expected/Observed | Pass/Fail/Unsupported/
  NotRun | Recovery/Rollback | CI/PR URL`）に従う。

## Acceptance Criteria

- [ ] monitor 宣言（`TrackMonitorDecl`/`PunchMarks`）の
  add/remove/modify が VCS-06 matrix で Case-ID 付きに検証され、
  typed change として区別されることを確認。
- [ ] `apply_selected` の monitoring kind に対する実挙動を実測
  して記録（対応なら Pass、未対応なら `Unsupported` fail-closed
  の実測 — 偽装しない）。
- [ ] restore-as-copy で `project.monitoring` が verbatim
  round-trip することを検証。
- [ ] ledger に新規 Case 行を追加し、既存行フォーマットに従う。
- [ ] 既存の VCS-06 matrix test・全 canonical lane が回帰しない。
- [ ] `*-REAL-*` 行・実 GUI・consumer gate・VCS Writer/3-way
  merge は本 slice 対象外 — Pass 扱いしない。
- [ ] `mise run fmt-check` / `check` / `lint` / `test` /
  `test-rt` / `test-msrv` を canonical 検証として実行し結果を
  impl-ir に記録する。

## Deferred Design Concerns

- monitoring kind の apply surface 追加 — `src/apply/` の
  supported_kind / entity_value / write_all 拡張を要する別 slice。
- `TrackMonitorDecl` の `lane` binding が指す lane の apply 時
  整合（宣言 transplant と lane ownership の順序設計）— 同上。
- `*-REAL-*`・COMPAT・VCS Writer/3-way merge — 外部 gate 残存。

## Sources and Relations

- `docs/specs/vcs-06-fixture-fault-matrix.md`（親 matrix spec。
  本 spec はその monitoring-domain coverage gap を回収する）。
- `docs/specs/rcd-03-monitor-persistence.md`（Deferred Design
  Concerns で diff 分類を follow-up とした永続化 spec —
  `docs/specs/rcd-03-monitor-persistence.md` 183-186 行目）。
- `docs/specs/rcd-03-vcs-diff-monitor-classification.md`
  （monitor kind の分類規約 spec — 本 matrix の Expected oracle）。
- `docs/impl/ir/rcd-03-monitor-persistence-slice.md` 248-250
  （defer 記録）。
- synvelis#97（統合受入 Issue、本 slice は Open のまま）。
- synvelis#117（monitor 宣言分類の実装 Issue、Refs）。
