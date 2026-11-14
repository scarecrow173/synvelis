---
id: "IMPL-034Xh5urQ31DNU1IgcvDZX"
type: "impl"
status: "completed"
title: "AUTX-01 Action Catalog（synvelis#260 headless fixture スライス）"
created: "2026-11-14"
updated: "2026-11-15"
owners: []
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
      - type: "file"
        path: "docs/impl/ir/README.md"
      - type: "file"
        path: "docs/impl/exp/README.md"
      - type: "file"
        path: "docs/test-specs/README.md"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/test-specs/autx-01-action-catalog.md"
  implemented-by: []
  depends-on:
    - "docs/automation/spec.md"
    - "docs/automation/design-capability-action.md"
    - "docs/automation/design-trust-boundary.md"
    - "docs/automation/design-owner-dag.md"
    - "docs/specs/command-gateway.md"
    - "docs/designs/command-gateway-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/automation/plan.md"
    - "docs/automation/tasks.md"
    - "docs/impl/ir/autx-07-gui-views.md"
    - "docs/impl/ir/autx-01-f01-has-executor.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/automation/design-capability-action.md"
    - "docs/automation/design-trust-boundary.md"
  derived-by: []
  verifies:
    - "docs/test-specs/autx-01-action-catalog.md"
  verified-by: []
  references:
    - "https://github.com/scarecrow173/synvelis/issues/259"
    - "https://github.com/scarecrow173/synvelis/issues/261"
    - "https://github.com/scarecrow173/synvelis/issues/262"
    - "https://github.com/scarecrow173/synvelis/issues/266"
    - "https://github.com/scarecrow173/synvelis/issues/271"
    - "https://github.com/scarecrow173/synvelis/issues/272"
    - "https://github.com/scarecrow173/synvelis/issues/446"
  defers:
    - "CapabilityRegistry::has_executor（解消: synvelis#446 / AUTX-01-F01 で着地 — ExecutorState は Installed/Absent/Unknown の 3 verdict、`docs/impl/ir/autx-01-f01-has-executor.md` 参照）。Gateway::schemas read accessor は sv#272/PR #438 で解消済み"
    - "実 ScriptApiModel（synvelis#271）との Capability ID/Version/Schema/Availability 同一性照合"
    - "Script Action 登録・script→catalog 投影（synvelis#272）"
    - "CLI surface 本体（synvelis#262 のリース）— 本スライスは consumer-neutral fixture のみ供給"
    - "実 GPUI render・実 OS・live Gateway adapter 経由の実行（synvelis#114/#267 別 Gate）"
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# AUTX-01 Action Catalog（synvelis#260 headless fixture スライス）

## Summary

Issue #260 の headless fixture スライス。`synvelis-workspace` に
read-only の Action Catalog 投影（`action_catalog` module）を実装した。
`ActionPresentation { capability, display_key, category,
description_key, parameter_hints?, visibility_hint? }` を Issue 契約どおり
define し、登録済み `CapabilityDescriptor` との join・`list`/`describe`
の状態別判定・consumer-neutral な共有 fixture を提供する。
第二 Registry・Executor・認可源・schema 源・target 源・Project writer・
job truth は一切作らない。

## Context

- 正本契約: `docs/automation/spec.md`（AUTX-00 umbrella）、
  `design-capability-action.md`（Action = `CapabilityRef` の
  presentation 投影・§3 表示 status 表）、`design-trust-boundary.md`
  （Metadata Disclosure / caller identity）、`design-owner-dag.md`
  （AUTX-01 = S1、#271/#272 境界）。
- 消費側: #266（AUTX-07, merge 済み）が建てた
  `automation_ui::project_action_catalog`/`ActionRow` view 契約は
  `&[CapabilityDescriptor]` を入力に取る — 本スライスの
  `ActionCatalog::descriptors()` がその実供給元となる
  consumer-neutral な投影を供給し、view 側契約は変更しない。
- `doc-driven-dev-graph` route は試行したが、リポジトリ全域の
  pre-existing な artifact-graph / broken-relation findings により
  `blocked` を返し task graph を生成しなかった（本スライスの変更とは
  無関係の既存損傷）。route 要求の impl/ir・impl/exp・test-specs
  文書は lease 指定パスに手動で作成した。

## Implementation

### Changes

- `crates/synvelis-workspace/src/action_catalog.rs` — catalog 本体。
  - `ActionPresentation`: Issue 契約の 6 field のみ
    （`capability: CapabilityRef` + display/search/UI hint）。
    `serde(deny_unknown_fields)` で authority-shaped key の混入を
    deserialization 段階で拒否。
  - `ActionStatus`: `Available` / `Disabled{reason}` / `Planned` /
    `Retired` / `UnsupportedVersion{supported}` / `Unknown` —
    design §3 の表示 status を機械可読に分離。
  - `SchemaConsistency`: `Consistent` / `Mismatch{detail}` /
    `Unknown`（schema store 未供給時の typed Unknown）。
  - `ExecutorState`: 当初 `Unknown` 単一 variant（registry public
    API が handler 有無を観測できなかったため typed Unknown）。
    synvelis#446（AUTX-01-F01）で `CapabilityRegistry::has_executor`
    が着地し、executor lookup 供給時は `Installed`/`Absent`、
    未供給・descriptor 無し entry は `Unknown` を維持する
    3 verdict 化（`docs/impl/ir/autx-01-f01-has-executor.md`）。
  - `ActionEntry`: `capability` + `descriptor: Option<…>`（登録
    descriptor verbatim）+ `presentation: Option<…>` + 3 verdict。
    descriptor clone は登録時点の snapshot であり権威の複写ではない
    （表示値は常に登録物と一致）。
  - `ActionCatalog::project(capabilities, schemas, executors,
    presentations)`（`executors: Option<&CapabilityRegistry>` は
    synvelis#446 で追加）: descriptor slice と presentation を
    `CapabilityRef` で join。重複入力は
    `DuplicateCapability`/`DuplicatePresentation` で
    fail-closed。orphan presentation は `Unknown` /
    `UnsupportedVersion` entry として surface する。
  - `ActionCatalog::project_gateway(&Gateway, …)`: registry
    snapshot → `schemas()`/`registry()` の read accessor で
    schema・executor 判定（sv#272 の `Gateway::schemas()` と
    sv#446 の `has_executor` で `&Gateway` 化。かつては
    `&mut Gateway` + `schemas_mut()` sequential borrow だった）。
  - `list(&ActionFilter)` / `describe(&CapabilityRef)` /
    `entries()` / `descriptors()`。`describe` は known-id/unknown-
    version をその場で `UnsupportedVersion` verdict に合成し、
    完全未知 id のみ `None`。
- `crates/synvelis-workspace/src/action_catalog/fixture.rs` —
  consumer-neutral 共有 fixture。gateway fixture に無い
  Track/Clip `Query`（`synvelis.track.query`/`synvelis.clip.query`、
  明示 `Resource{Track|Clip}` target）を追加し、全 7 capability の
  `ActionPresentation` と、4 domain（Query×2/ProjectMutation/
  RuntimeControl/AsyncOperation）を明示 Stable Target +
  domain-correct consistency で網羅する `invocations()` を供給。
- `crates/synvelis-workspace/tests/autx01_action_catalog.rs` —
  15 test（下記 Validation）。
- `crates/synvelis-workspace/src/lib.rs` — module 登録と re-export
  のみ（既存 export は不変）。

### Decisions During Implementation

- **gateway accessor 追加は Coordinator 判断で否**。`No Handler`
  別判定と `Gateway::schemas()` read accessor を ask したところ、
  sv#263（AUTX-04）の lease 衝突で `crates/synvelis-gateway/` への
  変更は認められず、フォールバック指示（typed Unknown/Unsupported
  として扱い、必要 API を evidence/PR に記録）に従った。
  `ExecutorState::Unknown` がその typed Unknown であり、
  `SchemaConsistency::Unknown` が schema store 未供給時の同型。
  `CapabilityRegistry::has_executor` と `Gateway::schemas` は
  Deferred として記録し、Coordinator 側で後続 slice 化される
  （その後 `Gateway::schemas` は sv#272/PR #438、`has_executor`
  は sv#446/AUTX-01-F01 で着地し、`ExecutorState` は
  `Installed`/`Absent`/`Unknown` の 3 verdict になった —
  `docs/impl/ir/autx-01-f01-has-executor.md`）。
- **`ActionStatus` と `SchemaConsistency`/`ExecutorState` を分離**。
  availability lifecycle・schema 整合・executor 有無は直交する
  verdict であり、単一 status に潰すと「Available だが schema
  不整合」等を表現できない。design §3 の表は status 語彙のみを
  規定するため、schema/executor は別 field の typed verdict とした。
- **presentation は `CapabilityRef` 完全一致で join**。version を
  解決・推測しない（`design-capability-action` §1: 版は厳密解決、
  "latest" 推測なし）。registry に無い version を指す presentation
  は `UnsupportedVersion` entry として残る。
- **`list` の既定は `include_unpresented: true`・
  `include_hidden: false`**。presentation 不在は「隠す指示」ではなく
  単に metadata 未整備なので registry truth は列挙可能に保ち、
  `visibility_hint: Hidden` は product の明示的な browse 除外指示
  として既定で除外する（`describe` では到達可能）。
- **fixture の wire は typed `WireRequest`**。`Gateway::bind`/`invoke`
  が typed request を受ける既存経路をそのまま使い、GUI lowering
  （`automation_requests`）と CLI（将来の `parse_wire` 経由）が
  同一 `CapabilityRef`/schema/target/consistency を共有することを
  fixture 層で固定する。CLI surface 本体は #262 のリース。

## Related Experiments

### Adopted

- `docs/impl/exp/autx-01-action-catalog.md` — 本スライスの設計判断と
  棄却案の記録（gateway accessor ask の結果を含む）。

### Rejected

- なし（exp 文書側に詳細）。

## Validation

- `mise run fmt-check`: Pass（`cargo fmt --all -- --check`、exit 0）。
- `mise run check synvelis-workspace`: Pass（`cargo check
  --locked --all-targets -p synvelis-workspace`、exit 0）。
- `mise run lint synvelis-workspace`: Pass（clippy warnings denied、
  exit 0）。
- `mise run test synvelis-workspace`: Pass — `autx01_action_catalog`
  15/15 test + 既存 suite 全 Pass（35 test binary、0 failed）。
- `mise run check synvelis-app`: Pass（exit 0 — 消費側非破壊）。
- impl audit（`audit_impl_record.js --dir docs/impl/ir`）: error 0、
  warning は defers 自由記述の broken-relation-link のみ
  （autx-07 と同型の既存 convention）。
- exp audit（`audit_experiment_log.js --dir docs/impl/exp`）:
  autx-01 findings 0。
- doc-status lint（test-spec）: non-blocking reciprocal-relation
  warning 3 件のみ（targets は lease 外の `docs/specs`・
  `docs/designs`）。
- docs index: `sync_impl_index.js --kind ir --apply`・
  `doc_maintenance.js index apply --dir docs/test-specs` で再生成済み
  （origin/main merge 後の conflict は生成物手順で解消）。
- OS: Windows（worktree ホスト）。toolchain は `mise` 管理。
- review-protocol: cooperative + adversarial の 2 pass 実施、
  CONFIRMED P0/P1 blocker 0 件（非 blocking note のみ）。

## Risks

- `ExecutorState` の `Installed`/`Absent` は executor lookup
  （`CapabilityRegistry`）供給時のみ報告される — `executors:
  None` の caller では全 entry が `Unknown`（fail-closed、
  観測不能を `Absent` で偽装しない）。`Absent` は display
  metadata であり、invoke の `NoExecutor` authority・policy
  経路は不変（sv#446 で解消した旧リスク: No Handler を
  `Unknown` と区別できなかった件）。
- `ActionCatalog` は build 時 snapshot — registry への後続登録は
  rebuild まで反映されない（登録は append-only/immutable のため
  descriptor 改変は起きない）。
- `parameter_hints` は product metadata であり schema field との
  整合は検証しない（hint 名と schema の不一致は表示品質の問題で
  authority 違反ではない）。

## Follow-ups

- ~~`CapabilityRegistry::has_executor` + `Gateway::schemas` read
  accessor~~ — 解消済み: `Gateway::schemas` は sv#272/PR #438、
  `has_executor` は sv#446/AUTX-01-F01（`evidence/sv446`）。
- #271 ScriptApiModel 投影との ID/Version/Schema/Availability 同一性
  照合 — #271 実装後に `catalog.descriptors()` を共通 source として
  照合する対応表は `docs/test-specs/autx-01-action-catalog.md` に記録。
- #262 CLI surface が `ActionCatalog`/`fixture::invocations()` を
  消費する際の結合検証。
- 実 GPUI/実 OS lane は #114/#267 の別 Gate（NotRun 記録済み）。
