---
id: "SPEC-034ZjJ8aK8r4WHvvG44LZh"
type: "spec"
status: "approved"
title: "Issue #283 slice — strip-fields gateway commit-gate exercise spec"
created: "2026-11-17"
updated: "2026-11-17"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/283"
    - "docs/discovery/issue-283-strip-gateway-exercise.md"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by:
    - "docs/designs/issue-283-strip-gateway-exercise.md"
  depends-on:
    - "docs/discovery/issue-283-strip-gateway-exercise.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/cb-06-track-bus-adapter.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/discovery/issue-283-strip-gateway-exercise.md"
  derived-by:
    - "docs/designs/issue-283-strip-gateway-exercise.md"
    - "docs/plans/issue-283-strip-gateway-exercise.md"
  verifies: []
  verified-by:
    - "docs/test-specs/cb-06-track-bus-adapter.md"
  references:
    - "evidence/sv283/verification.md"
  defers: []
  deferred-by: []
---

# Issue #283 slice — strip-fields gateway commit-gate exercise spec

## 目的

CB-06 の `StripFieldsAdapter`（synvelis#283 PR #447）と
`VariantPorts::standard` 登録（CB-10 PR #483）は main 着地済みだが、
gateway commit-gate 経路で strip-fields scope を駆動する headless
検証が残っていた。本 slice は `synvelis.branch.{fork,edit,adopt,
discard,list,describe,compare}` の CB-10 typed contract 経路で
strip-fields lifecycle を exercise し、typed reject・scope digest
pin・adopt CAS/approval 契約が gateway 面でも成立することを
headless で pin する。

## 受入条件

- AC-1: test-local fixture（`AddTrackStrip` + `InsertProcessor` を
  `ProjectController` で適用、head revision 2）上で
  `synvelis.branch.fork` が strip-fields scope を受理し、
  candidate/adopted の 2 branch を持つ variant set を 1 CAS で
  commit する。
- AC-2: candidate branch への ordinary op edit
  （`synvelis.clip.mute`・`synvelis.track.create`・
  `synvelis.clip.move`）が全て `GatewayError::
  PreparationRejected`（`variant-unsupported`）で typed reject
  され、live revision・branch content revision・対象 entity が
  不変である。wire 表面に strip-domain inner op capability が
  存在しないため positive mask-gated edit は本 slice では
  表現不能 — この事実自体を境界証拠として記録する。
- AC-3: `synvelis.branch.adopt` が `ApproveToApply` grant +
  human approval receipt 経路で commit され、provenance に
  `outcome == "applied"`・`project.committed == true`・
  `project.undo_steps == 1`・`objects == 0`・
  `runtime.state.state == "not-requested"`（VCS-off headless）が
  記録される。scope-identical candidate でも adopted-pointer
  swap と frozen previous branch が実 project truth として
  commit される。
- AC-4: `expected_scope_digest` の stale 値と
  `expected_branch_revision` の stale 値が prepare simulation で
  `variant-stale-scope` / `variant-stale-branch-revision` に
  typed reject される（pin は advisory でなく検証済み）。
- AC-5: `synvelis.branch.{list,describe,compare}` が strip-fields
  scope を typed decode し、`scope_kind == "strip-fields"`、
  scope の strip/fields round-trip、`compare` の
  `scope_digests_equal` と `verified` decode が正直な値を返す。
- AC-6: `StripFieldsAdapter::boundary_dependencies` が
  strip→master `Main` connection のみを scope-affiliated
  boundary として報告し、`capabilities` が declared mask に対し
  Supported/Unsupported を正直に報告する（domain-level 補助
  証拠 — CB-STRIP-02/03 の display truth = enforcement truth）。
- AC-7: `evidence/sv283/` と cb-06 doc chain
  （test-specs/impl ir/impl exp）に本 slice の検証証拠を追記し、
  Issue #283 は OPEN 維持（`Refs #283` のみ）。

## Non-goals

- `crates/synvelis-gateway/src/`・`crates/synvelis-project/` の
  変更（adapter・registration は着地済み・consume のみ）
- wire 表面への strip-domain op capability 追加
  （`SetStripMute`/`SetParameterBase`/`InsertProcessor` 等を
  lower する capability — 必要なら別 Issue/dispatch 対象）
- Routing mask 拡張（send/connection/sidechain/bus op の候補
  受理）・scope entity removal — ledger で Deferred 記録済みの
  別 slice
- `tests/rig/mod.rs` の `demo_project()` 変更（test-local
  fixture で解決 — 既存 test 前提への波及を避ける）
- `cb10_variant_actions.rs`・`cb11_ai_vcs_off.rs` の編集
- 実 PCM/実 plugin・実 GUI・実機/RT 検証（外部 gate — NotRun
  記録のみ）
- MSRV lane・`check-full`/`test-full` 等の full lane
- docs index（`docs/**/README.md`）の編集・再生成
  （Coordinator が統合時に実施）
- Issue #283 の Close・auto-close 表現

## 制約

- PR 本文・コミットメッセージは `Refs #283` のみ。
  `Closes`/`Fixes`/`Resolves` 系は使わない。他 Issue/リポジトリ
  参照は inline code 形式（例: `synvelis#539`）。
- 新規依存・feature 追加なし。
- 実行していない検証を Pass と書かない。canonical 検証は
  `mise run` 経由、cargo 直接実行は補助証拠と明記。
- 文書・コメント・報告は日本語。

## Deferred Design Concerns

- wire 表面の strip-domain op capability | gateway の ordinary
  capability 表に strip op の lowering が無く positive
  mask-gated edit が表現不能 | host/gateway の op 表面設計が
  確定した時点で別 dispatch | 現状は typed reject が境界の
  正直な証拠であり fail-closed として安全

## Sources and Relations

- Issue: https://github.com/scarecrow173/synvelis/issues/283
- discovery: `docs/discovery/issue-283-strip-gateway-exercise.md`
- 関連 doc chain: `docs/test-specs/cb-06-track-bus-adapter.md`、
  `docs/impl/ir/cb-06-track-bus-adapter.md`、
  `docs/impl/exp/cb-06-track-bus-adapter.md`
- 検証証拠: `evidence/sv283/acceptance-ledger.md`、
  `evidence/sv283/verification.md`
- contract 先例: `docs/discovery/issue-539-arrangement-adapter.md`
  系（gateway adapter 登録 slice）
