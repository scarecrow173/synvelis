---
id: "TASK-034ZeN5O8yT6dMB1tJteke"
type: "task"
status: "done"
title: "issue-286-diff-explorer-app-wiring"
created: "2026-11-17"
updated: "2026-11-18"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/286"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by:
    - "docs/impl/ir/issue-286-diff-explorer-app-wiring.md"
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/tasks/issue-517-render-branch-wiring.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/creative-branch-design.md"
  derived-by:
    - "docs/test-specs/issue-286-diff-explorer-app-wiring.md"
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# issue-286-diff-explorer-app-wiring

## Work

synvelis#286 の CB-09 Difference Explorer「後始末」slice —
headless view-model（`cb-09-diff-explorer` で実装済み）を
アプリケーションへ配線し、detached view の lifecycle・
`ViewSpec` mount・`syn.variant-diff.*` op dispatch・
CB-08 orchestrator / gateway CAS 経路への接続を完成させる。

- `synvelis-workspace`: `VariantDiffEngine` facade を追加 —
  CB-08 `VariantPorts`/`simulate`/`execute` を opaque に保持し、
  `compare`/`adopt_preview`/`adopt` を typed `VariantDiffIntent`
  経由で dispatch する。standard ports が CB-07
  `ArrangementAdapter` を含む全 merged adapter を登録し
  （synvelis#539）、signal provider 出力を
  provenance 保持のまま `AuxLayerEvidence` へ map する。
  `adopt` は `Adopt` intent のみ受理し、preview/compare
  intent は malformed reject（preview が commit 経路に
  入らないことを型で保証）。compare は commit と同じ asset
  blob probe を使い、probe 不在時は fail-closed。
- `synvelis-app`: `syn.variant-diff.visual`/`view`/`action`
  の 3 op を registry 登録。per-view `VariantDiffSession`
  （pin-scoped evidence）と bundle root 追跡を `UiOpsState`
  に保持し、project bundle 変更で engine/session/publish
  observation を再構築する。`ViewType::VariantDiff` 登録済み
  view のみ受理。view-local action は authority を参照せず
  適用し、authority action は persisted target、無ければ
  action 自身の explicit pin で context を解決する。
  compare は orchestrator 経由で evidence を採取して
  persisted semantic target に記録、adopt-preview は simulate
  のみ、adopt は明示 `params.approval` 必須の CAS commit。
  revision が実際に動いた時だけ session notify し、adopt
  receipt の `/runtime/state` が decode できた時だけ runtime
  publish observation を記録する（捏造しない）。
- `screens.rs`: `mount_variant_diff_view` を追加し `lib.rs`
  から re-export。`ViewSpec` が GPUI view host で mount
  できることを smoke test で固定。

## Classification

- `normal-plan-task`

## Upstream Documents

- `docs/specs/creative-branch.md`
- `docs/designs/creative-branch-design.md`
- `docs/test-specs/cb-09-diff-explorer.md`
- `docs/test-specs/issue-286-diff-explorer-app-wiring.md`
- `docs/impl/ir/cb-09-diff-explorer.md`
- `docs/impl/ir/cb-08-compare-orchestrator.md`
- GitHub issue `scarecrow173/synvelis#286`

## Dependencies

- CB-08 compare orchestrator（synvelis#285）— `VariantPorts` /
  `simulate` / `execute` の provider seam。
- CB-07 arrangement adapter（synvelis#284）— arrangement
  domain の supported 判定と adapter 実装。

## Affected Files

- `crates/synvelis-workspace/src/variant_diff.rs`
- `crates/synvelis-workspace/tests/cb09_diff_explorer.rs`
- `crates/synvelis-app/src/ui_ops.rs`
- `crates/synvelis-app/src/app.rs`
- `crates/synvelis-app/src/service.rs`
- `crates/synvelis-app/src/screens.rs`
- `crates/synvelis-app/src/lib.rs`
- `crates/synvelis-app/tests/cb09_variant_diff.rs`

## Done When

- [x] `syn.variant-diff.visual`/`view`/`action` が service
      registry に登録され、登録済み `ViewType::VariantDiff`
      view のみ受理する（open/close/target seed 含む）
- [x] compare が CB-08 orchestrator 経由で semantic/signal/
      perceptual evidence を pin-scoped に採取・保持し、
      projection は live authority/context を毎回再導出する
- [x] adopt-preview が commit せず preview のみ返し、adopt が
      明示 approval + CAS で commit する（stale revision は
      typed reject）
- [x] view-local action が project authority を一切変更しない
- [x] `ViewSpec` が `mount_variant_diff_view` で mount できる
- [x] headless（workspace 40 test）+ app（11 test）suite と
      clippy `-D warnings` が pass する

## Verification

- [x] `mise run check-app-native` / `lint-app-native` /
      `test-app-gpui`（GPUI driver 組立・clippy・
      GPUI-simulated test、final head で実行）
- [x] `mise run check synvelis-workspace` / `check synvelis-app`
- [x] `mise run lint synvelis-workspace` / `lint synvelis-app`
- [x] `mise run lint-app-gpui`（gpui-test feature lane）
- [x] `mise run test synvelis-workspace`（cb09_diff_explorer
      40/40 を含む全 suite）
- [x] `mise run test synvelis-app`（cb09_variant_diff 11/11
      を含む全 suite）
- [x] `mise run check-headless-deps`
- [~] `mise run fmt-check` — 本 worktree で cargo-fmt の
      `os error 206`（環境起因・diff 無関係）により fail。
      全 15 member の `cargo fmt -p -- --check` は個別 pass
      （evidence/sv286 参照）
- [x] `mise run check-app-gpui`
