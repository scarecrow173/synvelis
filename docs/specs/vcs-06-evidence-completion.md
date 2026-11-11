---
id: "SPEC-034VNNGsOaycnOBjCAH3m1"
type: "spec"
status: "approved"
title: "VCS-06 evidence completion — RESTORE-08 plugin_instances verbatim + COMPAT-03/04 CI legs"
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
    - "SPEC-pkRnrZ2RKJfPW1ymwMSENS"
    - "SPEC-034VpB76W8P3twLKj0HlHB"
    - "SPEC-034V2wnQPWFJdKz68KlwJG"
    - "DESIGN-034VG5a2ikhPyz9RCyjDDT"
  refines: []
  refined-by: []
  derives-from:
    - "https://github.com/scarecrow173/synvelis/issues/97"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "evidence/reports/vcs-06-compat-evidence.md"
    - "crates/synvelis-project/tests/vcs06_plugin_instance_matrix.rs"
    - ".github/workflows/ci.yml"
    - "tools/ci-summary.mjs"
  defers: []
  deferred-by: []
---

# VCS-06 evidence completion — RESTORE-08 plugin_instances verbatim + COMPAT-03/04 CI legs

## What Should Be Built

headless で完結する VCS-06 evidence の残り 2 系統を補完する。

1. **VCS-RESTORE-08**（plugin_instances verbatim
   restore-as-copy）: 新規
   `crates/synvelis-project/tests/vcs06_plugin_instance_matrix.rs`
   に fixture を追加し、`restore_as_copy` の `fork_project`
   （Project clone + `project_id` 再採番のみ）が
   `Project::plugin_instances`（issued `PluginInstanceId`・pin
   された `PluginRef`・単調 `watermark`）を **verbatim 保持**
   することを、publish 済み bundle の `SnapshotEnvelope`
   readback で pin する（先例:
   `restore_as_copy_preserves_monitoring_verbatim`）。
2. **VCS-COMPAT-03/04**（CI verify legs の実測記録）:
   `evidence/reports/vcs-06-compat-evidence.md`（新規）に、
   PR #268/#306/#323 をカバーする ci.yml verify leg の
   run URL・outcome を required/observed 区別つきで記録する。
   `gh pr checks`・`gh run list`・`gh run view` で自分で取得した
   実測値のみを記録し、CI leg を実 OS/plugin/consumer 受入に
   換算しない。

あわせて doc chain（spec/design/plan/task/test-spec/impl-ir）を
`vcs-06-evidence-completion` 命名で作成する。

## Why Now

headless fixture 行列（全 Case ID の Pass/Pass*）は PR
#268/#306/#323 で記録済みだが、2 つの coverage gap が残る:

- `plugin_instances`（schema v13 の persisted
  `StablePluginInstanceId` store）は restore-as-copy の verbatim
  保持が未実測 — `fork_project` は Project 全体を clone する
  ため理論上保持されるが、envelope readback での pin がない。
  VCS diff 側の不可視ギャップは別途 #324 で扱う（本 slice は
  restore 側のみ）。
- ledger の VCS-COMPAT-03 行は「local `mise` verification only;
  CI lane not executed here / NotRun」のまま。実際には #83 Close
  後の現行 ci.yml（`workflow_dispatch` 専用 Full CI）で main
  向け verify leg が実走しており、その run URL・outcome を
  集約 evidence として未記録だった。

## Users and Value

- Issue #97 の受入担当者: RESTORE-08 と COMPAT-03/04 の
  Expected/Observed を ledger 行として統合できる実測証拠を得る。
- 将来の apply/diff surface 拡張担当者（#324 系）: restore 側の
  verbatim 契約が pin され、diff 可視化追加時の対称性判断材料に
  なる。
- CI 運用担当者: COMPAT-03/04 の証拠が「どの run のどの leg が
  required/observed か」を run URL 付きで追跡できる。

## Scope

### In Scope

- `crates/synvelis-project/tests/vcs06_plugin_instance_matrix.rs`
  （新規・test のみ、1 test）
- `evidence/reports/vcs-06-compat-evidence.md`（新規）
- 本 spec / design / plan / task / test-spec / impl-ir チェーン

### Out of Scope

- `crates/*/src/` の変更（公開 API の外部観測契約のみを oracle
  にする）
- `evidence/reports/vcs-06-fixture-matrix-ledger.md` の編集
  （別 Worker が占有中 — ledger 行追加は Coordinator の後続
  slice で統合）
- `docs/*/README.md` 管理 index の再生成（同上・対象外）
- `vcs06_restore_matrix.rs`・`vcs06_apply_matrix.rs`・
  `plugin_instance_persist.rs` の変更
- VCS-APPLY-07 行の verdict 更新（#96 待ち）
- `plugin_instances` の VCS diff 可視化（#324・別 slice）
- `*-REAL-*` 行・実 GUI・実 OS durability・実 provider/remote・
  plugin host・RT・consumer・VCS Writer/3-way merge・
  Issue #97 の Close

## Requirements

- RESTORE-08 fixture は、resolved processor 2 件（異なる
  `PluginRef`）+ unresolved processor 1 件（identity entry 無し）
  の project を `restore_as_copy` し、publish 済み
  `SnapshotEnvelope` の `project.plugin_instances` が source と
  `PartialEq` 一致（`watermark` + 全 entry の `id`/`plugin`
  verbatim）すること、fork `project_id` のみが再採番されること、
  保持された identity が fork の `PluginRef` に対して
  `resolve()` で機能することを assert する。
- processor 保持 project の restore は plugin-state source を
  要するため、実 `DirectoryPluginStateStore` の manifest +
  Captured blob を port 経由で供給する（`vcs06_restore_matrix.rs`
  の `plugin_state_manifest_and_blobs_are_copied` と同型）。
- COMPAT evidence は `gh` で実取得した run URL・head_sha・
  leg outcome のみを記録し、required leg（pr-mode `evidence`
  gate 対象）と observed leg（`*-macos`・full-mode では required）
  を区別する。PR head に CI が無いこと（#145 で trigger 撤去済み）
  も実測として記録する。
- Pass* を Pass にしない、NotRun/Unsupported を Pass にしない、
  CI leg を実機受入に換算しない。

## Acceptance Criteria

- [ ] `vcs06_plugin_instance_matrix.rs` の test が
  `plugin_instances` verbatim 保持を envelope readback で pin し
  pass する。
- [ ] `evidence/reports/vcs-06-compat-evidence.md` が #268/#306/#323
  をカバーする ci.yml run の URL・outcome・required/observed
  区別を記録する。
- [ ] #83 Close に伴い COMPAT-03 の錨が現行 ci.yml
  verify/full lane である旨が明記される。
- [ ] ledger 未編集の理由と統合依頼（RESTORE-08・COMPAT-03/04
  の行追加）が PR 本文に記録される。
- [ ] 既存の VCS-06 matrix test・全 canonical lane が回帰しない。
- [ ] `mise run fmt-check` / `check` / `lint` / `test` /
  `test-rt` / `test-msrv` / `build` を canonical 検証として実行し
  結果を impl-ir に記録する。

## Deferred Design Concerns

- `plugin_instances` の VCS diff 可視化 — #324（実装 Issue）が
  Owner。本 slice は restore 側 verbatim のみ。
- ledger 行追加（VCS-RESTORE-08・VCS-COMPAT-03/04 の verdict
  更新）— ledger ファイル占有のため Coordinator の後続 slice。
- `docs/*/README.md` index 再生成 — 同上。
- webdav MockServer の Windows TransientIo（run 42938180676 の
  verify windows 失敗原因）— VCS-06 非関連の別 lane 問題として
  記録のみ。flake/残存 defect の裁定は対象外。
- `*-REAL-*`・COMPAT-02（consumer precision）・実 GUI・他 OS
  実機 — 外部 gate 残存。

## Sources and Relations

- `docs/specs/vcs-06-fixture-fault-matrix.md`（親 matrix spec）。
- `docs/specs/vcs-06-monitoring-matrix.md`（coverage-gap slice
  の先例 — RESTORE-07 の verbatim readback 手法を踏襲）。
- `docs/specs/sf-06-cross-gate-oracle-case-ledger.md`（verdict
  語彙 `Pass`/`Pass*`/`Fail`/`NotRun`/`Unsupported`/`Blocked`）。
- `docs/designs/plugin-instances-vcs-diff-design.md` と
  `docs/discovery/plugin-instances-vcs-diff-gap.md`（#324 —
  diff 側ギャップの設計正本。本 slice は restore 側のみ）。
- `crates/synvelis-project/src/restore/mod.rs` `fork_project`
  （clone + `project_id` 再採番のみの実装）。
- `.github/workflows/ci.yml` + `tools/ci-summary.mjs`
  （required/observed leg 定義の正本）。
- synvelis#97（統合受入 Issue、本 slice は Open のまま）。
- synvelis#83（独立 CI gate — Closed、COMPAT-03 の旧錨）。
