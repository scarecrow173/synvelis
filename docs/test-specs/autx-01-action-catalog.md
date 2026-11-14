---
id: "TSPEC-034Xh5EIhL7g4kg2TmAMyA"
type: "test-spec"
status: "approved"
title: "AUTX-01 Action Catalog headless test spec（synvelis#260）"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/260"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-workspace/src/action_catalog.rs"
      - type: "file"
        path: "crates/synvelis-workspace/src/action_catalog/fixture.rs"
      - type: "file"
        path: "crates/synvelis-workspace/tests/autx01_action_catalog.rs"
      - type: "file"
        path: "docs/impl/ir/autx-01-action-catalog.md"
      - type: "file"
        path: "docs/impl/exp/autx-01-action-catalog.md"
      - type: "file"
        path: "docs/test-specs/autx-01-action-catalog.md"
      - type: "file"
        path: "evidence/sv260/README.md"
    modified:
      - type: "file"
        path: "crates/synvelis-workspace/src/lib.rs"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on:
    - "docs/automation/design-capability-action.md"
    - "docs/automation/design-trust-boundary.md"
    - "docs/specs/command-gateway.md"
    - "docs/designs/command-gateway-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/ir/autx-01-action-catalog.md"
    - "docs/automation/spec.md"
    - "docs/automation/design-owner-dag.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/command-gateway.md"
    - "docs/designs/command-gateway-design.md"
  derived-by: []
  verifies:
    - "docs/specs/command-gateway.md"
    - "docs/designs/command-gateway-design.md"
  verified-by: []
  references:
    - "https://github.com/scarecrow173/synvelis/issues/259"
    - "https://github.com/scarecrow173/synvelis/issues/261"
    - "https://github.com/scarecrow173/synvelis/issues/262"
    - "https://github.com/scarecrow173/synvelis/issues/266"
    - "https://github.com/scarecrow173/synvelis/issues/271"
    - "https://github.com/scarecrow173/synvelis/issues/272"
  defers: []
  deferred-by: []
---

# AUTX-01 Action Catalog tests

## Purpose

`crates/synvelis-workspace/tests/autx01_action_catalog.rs` が固定する
AUTX-01 headless 契約を記録する。実 CLI surface（#262）・実
ScriptApiModel（#271）・実 GPUI/OS lane（#114/#267）での再検証が来ても、
headless で担保した行と残る `NotRun` 行の境界を失わないための仕様。

## Feature

AUTX-01 Action Catalog — read-only Capability→Action 投影・
状態別判定・no-authority・consumer-neutral 共有 fixture

## Rules

- **Rule AUTX-CAT-JOIN** catalog entry は登録 `CapabilityDescriptor`
  との join 結果であり、`descriptor` field は登録物 verbatim。
  `ActionPresentation` は `capability`/`display_key`/`category`/
  `description_key`/`parameter_hints`/`visibility_hint` のみを持ち、
  schema/target/effect/permission/availability/version の複写を
  型として持たない（`catalog_joins_presentations_with_current_descriptors`、
  `presentation_carries_only_display_metadata`）。
- **Rule AUTX-CAT-STATUS** `list`/`describe` は Unknown id・
  UnsupportedVersion・Disabled・Planned・Retired・orphan
  presentation を別々の typed verdict で返す。`Available` は
  `Availability::Available` の写しであり実行 grant ではない
  （`describe_distinguishes_unknown_id_and_unsupported_version`、
  `orphan_presentations_surface_as_unknown_or_unsupported`、
  `availability_states_project_verbatim_and_distinct`）。
- **Rule AUTX-CAT-SCHEMA** 宣言 `SchemaRef` が schema store で
  resolve できない incoherent 登録は `SchemaConsistency::Mismatch`、
  store 未供給は `Unknown` — 推測で `Consistent` にしない
  （`schema_consistency_is_typed_consistent_mismatch_unknown`）。
- **Rule AUTX-CAT-EXECUTOR** executor 有無は registry public API で
  観測不能のため `ExecutorState::Unknown` — `NoExecutor` は
  `Gateway::invoke` のみが authority（Deferred、
  `has_executor` predicate 待ち）。
- **Rule AUTX-CAT-NOAUTH** list/describe/entries は caller/grant/
  approval を生成しない。列挙後の invoke は毎回 Gateway
  bind/policy を通り、DenyAllPolicy 下では `Denied`、wire の
  self-declared authority field は `parse_wire` で拒否される
  （`enumeration_grants_no_authority_invoke_still_policy_gated`）。
- **Rule AUTX-CAT-IDENTITY** `CapabilityRef`↔Action は 1:1 の
  version 付き参照。display 名変更で `capability`/`descriptor`/
  `status` は不変、invocation の意味は変わらない
  （`display_name_change_preserves_capability_identity`）。
- **Rule AUTX-CAT-FIXTURE** Track/Clip Query・ProjectMutation・
  RuntimeControl・AsyncOperation の 4 domain について、同一
  CapabilityRef/version + 明示 Stable Target + domain-correct
  consistency の `WireRequest` が `Gateway::bind`/`invoke` を通る
  consumer-neutral fixture がある。target は選択/focus/active
  window から推測しない（`fixture_invocations_cover_all_domains_with_explicit_targets`）。
- **Rule AUTX-CAT-VIEW** #266 の `project_action_catalog` は
  `catalog.descriptors()` をそのまま消費でき、view 契約は不変
  （`catalog_feeds_autx07_view_projection_unchanged`）。
- **Rule AUTX-CAT-UISEP** view-local UI action（`ViewAction` 系）は
  DAW Action ではない — registry 未登録の id は `Unknown` entry
  に留まり descriptor を持たず実行不能
  （`unregistered_ids_stay_unknown_and_unexecutable`）。

## Examples

- **Example AUTX-CAT-STATUS-01** `synvelis.track.query@9.9` の
  describe → `UnsupportedVersion{supported:[1.0]}`、descriptor 無し。
  `synvelis.no.such.capability@1.0` → `None`。
- **Example AUTX-CAT-STATUS-02** presentation が未知 id
  `synvelis.unregistered.ghost@1.0` を指す → entry は `Unknown`・
  `descriptor: None`・`presentation: Some` で list に残る。
- **Example AUTX-CAT-NOAUTH-01** DenyAllPolicy の Gateway に
  catalog を project しても、fixture caller の invoke は
  `GatewayError::Denied`。`"as_user"`/`"role"` を含む wire JSON は
  `parse_wire` が拒否する。
- **Example AUTX-CAT-FIXTURE-01** `midi.add-note@1.1` は
  `Resource{source}` target + `ExpectedRevision`、
  `job.render-export@1.0` は target 無し + `Unconditional` で
  それぞれ `Completed`/`Accepted` outcome。

## Guarantees

- catalog の唯一の capability source は供給された登録
  `CapabilityDescriptor` 集合であり、第二 registry・登録 API・
  authority field の複写を持たない。
- 機械可読な schema/target/effect/availability/version の表示値は
  常に登録 descriptor と一致する（clone ではなく同一 object）。
- `list`/`describe` は read-only で、いかなる Grant/Approval/
  Manual Authority も生成しない。
- 未登録 id・未対応 version・Disabled/Planned/Retired・schema 不整合・
  executor 不明が区別可能な typed verdict として得られる
  （No Handler のみ `ExecutorState::Unknown` で Deferred）。

## Non-goals

- 実 ScriptApiModel（#271）との ID/Version/Schema/Availability
  同一性照合 — #271 実装が main 未存在のため Deferred。
  本 spec は「catalog の唯一 source が Gateway descriptor である」
  ことを型・fixture・契約レベルで保証し、照合に備える。
- Script Action 登録・script→catalog 投影（#272）、CLI surface
  本体（#262）、Macro/Workflow 実行、CLI 認証。
- 実 GPUI render・実 OS・live Gateway adapter 経由の実行
  （#114/#267 別 Gate）。
- `parameter_hints` と schema field の整合検証（表示 metadata で
  あり authority ではない）。
- Project/Undo/RT/Canvas の独立 Oracle — 本 slice は触れない。

## Risk

- `ExecutorState::Unknown` のままでは「登録済みだが handler 無し」
  （`NoExecutor` になる capability）を UI が事前表示できない。
  fail-closed のため誤実行はないが、`has_executor` predicate
  （Deferred）導入まで表示上の区別は付かない。
- catalog は build 時 snapshot — registry 登録の追加は rebuild
  まで反映されない（登録は append-only/immutable のため
  descriptor 自体は変化しない）。
- display metadata（display_key/category/description_key）の
  i18n 解決は product 側の責務 — catalog は key を運ぶだけで
  解決も翻訳もしない。
