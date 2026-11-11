---
id: "IMPL-034VTFdV4uROWPZI1T0Xga"
type: "impl"
status: "completed"
title: "VCS-06 evidence completion slice (synvelis#97)"
created: "2026-11-11"
updated: "2026-11-11"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/97"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-project/tests/vcs06_plugin_instance_matrix.rs"
      - type: "file"
        path: "evidence/reports/vcs-06-compat-evidence.md"
      - type: "file"
        path: "docs/specs/vcs-06-evidence-completion.md"
      - type: "file"
        path: "docs/designs/vcs-06-evidence-completion-design.md"
      - type: "file"
        path: "docs/plans/vcs-06-evidence-completion-plan.md"
      - type: "file"
        path: "docs/tasks/vcs-06-evidence-completion-task.md"
      - type: "file"
        path: "docs/test-specs/vcs-06-evidence-completion.md"
      - type: "file"
        path: "docs/impl/ir/vcs-06-evidence-completion.md"
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/tasks/vcs-06-evidence-completion-task.md"
    - "docs/plans/vcs-06-evidence-completion-plan.md"
    - "docs/designs/vcs-06-evidence-completion-design.md"
    - "docs/specs/vcs-06-evidence-completion.md"
  implemented-by: []
  depends-on:
    - "docs/designs/vcs-06-evidence-completion-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/ir/vcs-06-monitoring-matrix-slice.md"
    - "docs/specs/vcs-06-fixture-fault-matrix.md"
    - "docs/specs/sf-06-cross-gate-oracle-case-ledger.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/plans/vcs-06-evidence-completion-plan.md"
  derived-by: []
  verifies:
    - "docs/test-specs/vcs-06-evidence-completion.md"
  verified-by:
    - "docs/test-specs/vcs-06-evidence-completion.md"
  references:
    - "evidence/reports/vcs-06-compat-evidence.md"
    - ".github/workflows/ci.yml"
    - "tools/ci-summary.mjs"
  defers:
    - "ledger 行追加（VCS-RESTORE-08・VCS-COMPAT-03/04 — ledger 占有のため Coordinator 後続 slice で統合）"
    - "docs/*/README.md index 再生成（同上・対象外）"
    - "*-REAL-* 行・実 GUI/実 OS/実 provider/consumer/RT — 外部 gate 残存"
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# VCS-06 evidence completion slice (synvelis#97)

## Summary

synvelis#97 VCS-06 の headless で完結する evidence 補完 slice。
`src/` 変更なし・test/report/docs のみの変更で、2 系統を実施した:

1. **VCS-RESTORE-08**: 新規
   `crates/synvelis-project/tests/vcs06_plugin_instance_matrix.rs`
   に `restore_as_copy_preserves_plugin_instances_verbatim` を追加。
   resolved processor 2 件（異なる `PluginRef` pin）+ unresolved
   processor 1 件の project を `restore_as_copy` し、publish 済み
   `SnapshotEnvelope` の `project.plugin_instances` が source と
   `PartialEq` 一致（`watermark` + 全 entry の `PluginInstanceId`/
   `PluginRef` verbatim）・fork `project_id` のみ再採番であることを
   pin した。
2. **VCS-COMPAT-03/04**: `gh pr checks` / `gh run list` /
   `gh run view` で実取得した ci.yml verify leg の run URL・
   head_sha・leg outcome を
   `evidence/reports/vcs-06-compat-evidence.md` に
   required/observed 区別つきで記録した。#83 Close に伴い
   COMPAT-03 の錨が現行 ci.yml verify/full lane である旨を明記。
   CI leg を実 OS/plugin/consumer 受入に換算していない。

doc chain（spec/design/plan/task/test-spec/impl-ir）を
`vcs-06-evidence-completion` 命名で作成した。
`evidence/reports/vcs-06-fixture-matrix-ledger.md` は別 Worker が
占有中のため未編集 — RESTORE-08・COMPAT-03/04 の行追加は PR 本文の
「未対応の作業」で Coordinator に統合依頼する。

## Context

Implements `docs/tasks/vcs-06-evidence-completion-task.md` per
`docs/specs/vcs-06-evidence-completion.md` and
`docs/designs/vcs-06-evidence-completion-design.md`, verified by
`docs/test-specs/vcs-06-evidence-completion.md`（synvelis#97 の
coverage-gap 補完 slice。headless fixture 行列は PR
#268/#306/#323 で記録済みで、本 slice は残る headless 完結分のみを
対象とする。Issue #97 は `*-REAL-*` 行・実 GUI/実 OS/実
provider/consumer・VCS Writer/3-way merge の外部 gate 残存で
Open のまま）。

## Implementation

### Changes

- `crates/synvelis-project/tests/vcs06_plugin_instance_matrix.rs`
  （新規・1 test）— vcs06 系 matrix file と同型の helper
  （`controller`/`apply_op`/`AllPresentProbe`/`snap_live`/
  `plugin_ref`/`insert_processor`/`MapBlobs`/`StoreBackedPluginState`/
  `request`/`tmp_dir`）に加え、実 `DirectoryPluginStateStore` へ
  manifest + `Captured` blob を書き込む `plugin_state_source`
  helper（`vcs06_restore_matrix.rs` の
  `plugin_state_manifest_and_blobs_are_copied` と同型）を持つ。
  - `restore_as_copy_preserves_plugin_instances_verbatim`
    （VCS-RESTORE-08）: `InsertProcessor` op 経由で resolved
    processor 2 件（`vendor.dist`/`1.0.0`/rev3・
    `vendor.eq`/`2.1.0`/rev7 — `sync_processor`→`ensure`→
    `allocate` が id 1,2・watermark 3 を発行）と unresolved
    processor 1 件を構成。pre-condition で `watermark == 3`・
    entry 2 件・unresolved に entry 無しを assert。processor
    保持 project の restore は plugin-state source を要求するため
    実 store port を供給し `restore_as_copy` → `Published`。
    `dest` の `HEAD` → snapshot bytes を `SnapshotEnvelope` として
    readback し、fork `project_id == 777`（source `1` と別）、
    `envelope.project.plugin_instances ==
    c.live().plugin_instances`（`PartialEq` 全 field 一致）、
    保持 identity が fork の `PluginRef` で `resolve()` 可能、
    `validate_references()` 合格、`receipt.effects.source/git`
    untouched invariant を assert。
- `evidence/reports/vcs-06-compat-evidence.md`（新規）—
  lane 分類の正本（`.github/workflows/ci.yml` +
  `tools/ci-summary.mjs`）に基づく required leg（pr-mode
  `evidence` gate 対象: verify ubuntu/windows・rt-verify・
  gw10-verify・msrv-verify・ui-verify・dep-guard）と observed leg
  （`verify-macos`・`gw10-verify-macos`・`ui-verify-macos` —
  pr-mode では observed、full-mode では required）を区別して記録。
  PR head に check が無いこと（ci.yml は #145 で
  `pull_request` trigger 撤去済みの `workflow_dispatch` 専用）を
  実測として記録。merge commit を含む main 向け run
  42938180676（head `13e65189`）の leg outcome を URL 付きで
  転記し、#268/#306 は「測定済みだが非一様に green（windows
  verify が VCS-06 非関連の webdav MockServer TransientIo で
  failure）」、#323 は「covering run 無し = NotRun (pending)」と
  正直に記録。再検証 2026-11-11T19:21Z で run list 不変を確認。
- docs チェーン 6 file（spec/design/plan/task/test-spec/impl-ir）
  — `vcs-06-evidence-completion` 命名。`docs/*/README.md` index
  は対象外のため未更新。

### Decisions During Implementation

- RESTORE-08 の oracle は先例
  `restore_as_copy_preserves_monitoring_verbatim`
  （`vcs06_monitoring_matrix.rs`・VCS-RESTORE-07）と同じく
  publish 済み bundle の `SnapshotEnvelope` readback とした。
  `fork_project` は `snapshot.projection.project().clone()` に
  `project_id` を代入するだけなので、保持契約は
  `Project::plugin_instances` 全体（`watermark`・entry の
  `id`/`plugin`）の verbatim 一致で pin するのが最小かつ
  authoritative。
- identity 発行は test 側で `PluginInstanceIdentities::allocate`
  を直接呼ばず、`InsertProcessor` op の `sync_processor`→
  `ensure`→`allocate` 経路に委ねた — persisted store の発行
  semantics（水印単調・unresolved は entry 無し・Resolved のみ
  entry）をそのまま fixture に使い、test が発行機構を再実装
  しない。
- watermark の期待値は 3（「next id to issue」 semantics —
  2 回の allocate で id 1,2 発行・watermark 3）。test-spec の
  example 記述（watermark=2）は例示であり、実測値 3 を assert
  した。
- plugin-state source は port 抽象（`RestorePluginStateSource`）
  経由で実 `DirectoryPluginStateStore` の on-disk layout を
  供給 — mock ではなく実 store の manifest 名規約
  （`plugin_state_manifest_name`）と blob read を通すことで、
  restore が要求する「実 store 互換 source」の条件を満たす。
- COMPAT 記録は取得時点の snapshot として扱い、将来 run の
  結果を推測で補わない。run 42938180676 の windows verify
  failure（`synvelis-library --test webdav` の MockServer
  TransientIo・os error 10053/10054）は VCS-06 fixture と無関係
  な library lane の問題として区別し、flake/残存 defect の
  裁定は行わず記録のみとした。
- #83（独立 CI gate）は Closed（2026-11-04）のため、COMPAT-03
  の錨を現行 ci.yml の verify/full lane（`evidence` pr-mode
  gate + `ci-summary` full-mode gate）へ明示的に付け替えた。

## Related Experiments

### Adopted

<!-- none（実契約の測定 fixture と実測転記のみ — 探索的変更なし）。 -->

### Rejected

<!-- none -->

## Validation

`origin/main` @ `4443007`（#332 plugin_instances diff 分類・
#333 monitoring apply surface・#334 VCS-04 backend 配線・
#336 PC-C4-06 merge 済み）取り込み後の最終状態で実測
（Windows 11 x64・NTFS、pinned rustc 1.99.0、MSRV lane は
rust 1.89.0）:

- `mise run fmt-check` — Pass（`cargo fmt --all -- --check`、
  exit 0）
- `mise run check` — Pass（`cargo check --workspace
  --all-targets`、exit 0）
- `mise run lint` — Pass（`cargo clippy --workspace
  --all-targets -- -D warnings`、exit 0）
- `mise run test` — Pass（`cargo test --workspace --locked`、
  exit 0。`vcs06_plugin_instance_matrix.rs` 1 test を含む）
- `mise run test-rt` — Pass（`cargo test --workspace --locked
  rt_`、exit 0）
- `mise run test-msrv` — Pass（`mise exec rust@1.89.0 -- cargo
  test --workspace --locked`、exit 0）
- `mise run build` — Pass（`cargo build --workspace --locked`、
  exit 0）

補助証拠（canonical = mise、直接 cargo は補助）:
`cargo test -p synvelis-project --test vcs06_plugin_instance_matrix`
— 1 passed、`cargo clippy -p synvelis-project --all-targets --
-D warnings` — clean、`cargo fmt --all -- --check` — clean。

NotRun / Unsupported（test-spec/spec 宣言どおり、headless
fixture・CI 記録は以下を証明しない）:

- `*-REAL-*` 行・実 GUI・実 OS durability・実 provider/remote・
  plugin host・RT・consumer・他 OS 実機 — NotRun（外部 gate・
  後続 slice）。
- COMPAT-02（consumer precision）— NotRun。
- VCS Writer/3-way merge — Blocked 維持（#231）。
- CI leg の pass/fail は workflow 実行の証拠であり、実機受入に
  換算しない。#323 をカバーする ci.yml run は収集時点で未実走
  （NotRun pending — 失敗ではない）。

## Risks

- COMPAT evidence は収集時点（2026-11-11T19:21Z 再検証）の
  snapshot — その後の日次 dispatch で run が増えても本
  レポートは取得時点の実測を保持する。#323 の covering run が
  後続で実走・失敗した場合の ledger verdict 更新は Coordinator
  判断。
- run 42938180676 の windows verify failure は webdav MockServer
  の TransientIo で VCS-06 非関連と判断したが、flake か残存
  defect かの裁定は本 slice の範囲外 — 別 lane 問題として記録
  のみ。
- `plugin_instances` の VCS diff 可視化は #324 として #332 で
  実装済み（本 slice 着手後に merge）。本 slice の restore 側
  verbatim 契約は diff 側実装と独立に成立するが、ledger 側で
  両者の verdict を統合する際は #332 の merge 状態を前提に
  すること。

## Follow-ups

- `evidence/reports/vcs-06-fixture-matrix-ledger.md` への
  VCS-RESTORE-08 行追加（本 fixture の実測に基づく verdict は
  Coordinator が判定）と VCS-COMPAT-03/04 行更新
  （`vcs-06-compat-evidence.md` の実測を根拠に）— ledger 占有の
  ため Coordinator の後続 slice で統合。
- `docs/*/README.md` index 再生成 — 同上・対象外。
- synvelis#97 残件（本 slice で Close しない）: `*-REAL-*` 行・
  COMPAT-02・実 GUI/実 OS/実 provider/consumer gate・VCS
  Writer/3-way merge の統合受入（Close は Coordinator 判断）。
