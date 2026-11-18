---
id: "DISC-034ZjInvJi1kqRJQbP8fB2"
type: "discovery"
status: "resolved"
title: "Issue #283 slice — strip-fields gateway commit-gate exercise 調査"
created: "2026-11-17"
updated: "2026-11-17"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/283"
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
    - "docs/test-specs/cb-06-track-bus-adapter.md"
    - "docs/impl/ir/cb-06-track-bus-adapter.md"
    - "docs/impl/exp/cb-06-track-bus-adapter.md"
    - "evidence/sv283/acceptance-ledger.md"
    - "evidence/sv283/verification.md"
    - "docs/discovery/issue-539-arrangement-adapter.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by:
    - "docs/specs/issue-283-strip-gateway-exercise.md"
  verifies: []
  verified-by: []
  references:
    - "crates/synvelis-gateway/tests/cb10_variant_actions.rs"
    - "crates/synvelis-gateway/tests/cb11_ai_vcs_off.rs"
    - "crates/synvelis-gateway/tests/rig/mod.rs"
    - "crates/synvelis-gateway/src/variant.rs"
    - "crates/synvelis-project/src/variant/strip.rs"
    - "crates/synvelis-project/src/variant/adapter.rs"
  defers: []
  deferred-by: []
---

# Issue #283 slice — strip-fields gateway commit-gate exercise 調査

## Summary

CB-06 の `StripFieldsAdapter` 本体と `VariantPorts::standard` 登録は
main 着地済みだが、gateway commit-gate 経路（prepare → proposal →
approval → apply → CommitGate）で strip-fields scope を駆動する
headless 検証が残っていた。本 slice はその経路 exercise を
`crates/synvelis-gateway/tests/cb06_strip_gateway.rs` として追加する。
調査の結果、wire 表面に strip-domain の inner op capability が
存在しないことが確定し、positive mask-gated edit は本 slice の
wire 契約では表現不能 — typed reject と adopt/query 面の pin が
正直な境界証拠となる（confidence: high — ソース実測に基づく）。

## Exploration Goal

`synvelis#283`（CB-06 Track/Bus 候補 Adapter tracker）の残件である
「gateway commit-gate 経路の strip-fields headless exercise」を、
lease 内ファイルのみで実現可能かを確定する。具体的には:

- `VariantPorts::standard(None)` 経由で strip-fields scope の
  fork → edit → adopt → describe/compare を CB-10 typed contract
  で駆動できるか
- `demo_project()` に track strip が無い問題を test-local fixture
  で解決できるか（`rig/mod.rs` の共有 fixture を変更せずに）
- routing/external op の typed reject と scope digest pin を
  gateway proposal/digest 面で検証できるか

## Key Issues and Assumptions

- **wire 表面に strip op capability が無い（confirmed）**:
  `lower_for` の capability 表は clip/track/lane/note/midi/asset/
  source 系のみで、`SetStripMute`・`SetParameterBase`・
  `InsertProcessor` 等の strip-domain `ProjectOp` へ lower する
  capability は存在しない。`synvelis.branch.edit` の inner op は
  ordinary capability のみを受け付けるため、mask-gated strip edit
  の positive 経路は wire 上表現不能。
- **candidate edit は adapter confinement を通る（confirmed）**:
  非 adopted branch への edit は `StripFieldsAdapter::
  edit_candidate` → `edit_scoped_project_op` を通り、scope 外・
  mask 外・unsupported op は `VariantError::Unsupported` →
  gateway prepare で `GatewayError::PreparationRejected`
  （"variant simulation failed: ..."）に typed map される。
- **adopted branch への edit は live edit（confirmed）**:
  `EditAdopted` は adapter confinement を通らず live へ直接適用
  される設計。strip-fields scope での confinement 負例は
  candidate branch に対してのみ意味を持つ。
- **strip fixture は test-local で構築可能（confirmed）**:
  `ProjectOp::AddTrackStrip { track_id }` が strip・chain・
  main ports・seeded channel targets（Fader/Mute/Solo）・
  master への main route を mint し、`InsertProcessor` が
  Inserts mask の owned objects を与える。`ProjectController`
  経由で適用すれば `demo_project()` の前提を壊さずに済む。
- **adopt は approval ticket が必須（confirmed）**:
  `adopt_guard` は空 ticket を `VariantError::Unapproved` で
  reject する。human apply は ticket を mint できないため、
  positive adopt は `ApprovalPolicy::ApproveToApply` の agent
  grant + human approval receipt 経路（cb10/cb11 先例）が必要。

## Alternatives and Comparison

| Option | Pros | Cons | Lean |
| --- | --- | --- | --- |
| A: `cb11_ai_vcs_off.rs` へ追記 | 既存 helper 再利用 | lease は新規ファイルを第一候補とし、cb11 は audio+arrangement の VCS-off lifecycle を担う別関心事。strip fixture（controller seed）が helper 群と混ざる | no |
| B: 新規 `cb06_strip_gateway.rs` + test-local fixture | 関心事分離・`rig/mod.rs` 非変更で既存 test 前提に波及しない | helper の一部複写が必要 | yes |
| C: `rig/mod.rs` の `demo_project()` に strip seed 追加 | 全 test が strip を持つ | serialize 比較・connection 数前提など既存 gateway test への波及リスク。lease も「波及が確定した場合のみ」 | no |

## Tentative Conclusions and Hypotheses

- **confirmed**: 本 slice の主成果物は新規
  `crates/synvelis-gateway/tests/cb06_strip_gateway.rs`。
  fixture は `AddTrackStrip` + `InsertProcessor` を
  `ProjectController` で適用した test-local project（head
  revision 2）を `DirectoryBundleStore` + `MemorySnapshotSource`
  に pin する。
- **confirmed**: positive mask-gated edit は wire 非表現のため、
  edit 面の証拠は「全 ordinary op が candidate で
  `variant-unsupported` に typed reject され、live/branch
  revision が不変」であること。routing/send/bus op は
  capability 自体が存在せず fail-closed。
- **confirmed**: adopt 面は scope-identical candidate でも
  `Applied`（1 undo step）を返し、`expected_scope_digest`・
  `expected_branch_revision` の stale pin が prepare simulation
  で `variant-stale-scope` / `variant-stale-branch-revision`
  に typed reject されることを検証する。
- **confirmed**: `boundary_dependencies` は strip→master `Main`
  connection のみを scope-affiliated boundary として報告し、
  `capabilities` は declared mask に対し Supported/Unsupported
  を正直に報告する（domain-level 補助証拠）。

## Open Questions

- なし（実装方針は確定。wire 表面の strip op capability 追加は
  本 slice の範囲外であり、必要なら別 Issue/dispatch 対象）。

## Promotion Candidates

- [x] spec-doc needed: `docs/specs/issue-283-strip-gateway-exercise.md`
- [ ] adr-doc needed: 新規 ADR は不要 — 既存 CB-06/CB-10 契約の
      consume のみで新規の architecture decision を含まない

## Sources and Relations

- Issue: https://github.com/scarecrow173/synvelis/issues/283
- dispatch 探索結果: `sv-cb06.explore.json`（残作業の実測位置・
  fixture 構成材料）
- 実測ソース: `crates/synvelis-gateway/src/variant.rs`（scope
  parse・registration）、`crates/synvelis-project/src/variant/
  strip.rs`（adapter 契約）、`crates/synvelis-project/src/
  variant/adapter.rs`（shared edit gate）、
  `crates/synvelis-gateway/tests/cb10_variant_actions.rs`・
  `cb11_ai_vcs_off.rs`（contract 先例）、
  `crates/synvelis-gateway/tests/rig/mod.rs`（fixture 群）

## Evidence

- 2026-11-17: `mise run check synvelis-gateway` で新規 test が
  コンパイル可能であることを確認（初回は `ClipId` import・
  `DomainAdapter` trait・associated fn・`&mut` 借用の 4 系統を
  修正）。`cargo test --test cb06_strip_gateway` で 2 test
  Pass（`cb06_strip_fields_gateway_commit_gate_lifecycle`・
  `cb06_strip_fields_scope_boundary_and_capabilities`）。
  `synvelis.note.add` は存在せず `synvelis.midi.note.add` で
  あること、および tick 引数 shape が異なることを
  `MalformedRequest` で実測確認し、既知 shape の
  `synvelis.clip.move`（musical anchor）へ置き換えた。
