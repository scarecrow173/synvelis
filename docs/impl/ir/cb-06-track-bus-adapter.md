---
id: "IMPL-034Xpa4SOZfOriBGBTfZdR"
type: "impl"
status: "in-progress"
title: "CB-06 Track/Bus 候補 Adapter — Strip Domain Mask・Routing参照保護・Adopt（synvelis#283）"
created: "2026-11-14"
updated: "2026-11-14"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/283"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-project/src/variant/strip.rs"
      - type: "file"
        path: "crates/synvelis-project/tests/cb06_strip_fields.rs"
      - type: "file"
        path: "docs/impl/ir/cb-06-track-bus-adapter.md"
      - type: "file"
        path: "docs/test-specs/cb-06-track-bus-adapter.md"
      - type: "file"
        path: "evidence/sv283/acceptance-ledger.md"
      - type: "file"
        path: "evidence/sv283/verification.md"
    modified:
      - type: "file"
        path: "crates/synvelis-project/src/variant/adapter.rs"
      - type: "file"
        path: "crates/synvelis-project/src/variant/mod.rs"
    deleted: []
    renamed: []
    moved: []
    generated:
      - type: "file"
        path: "docs/impl/ir/README.md"
      - type: "file"
        path: "docs/test-specs/README.md"
  implements: []
  implemented-by: []
  depends-on:
    - "docs/specs/creative-branch.md"
    - "docs/designs/creative-branch-design.md"
    - "docs/adr/cb-00-creative-branch.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/ir/cb-05-fx-range-adapter.md"
    - "docs/impl/ir/cb-04-instrument-adapter.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies:
    - "docs/test-specs/cb-06-track-bus-adapter.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# CB-06 Track/Bus 候補 Adapter — Strip Domain Mask・Routing参照保護・Adopt（synvelis#283）

## Summary

`VariantScope::StripFields` の実ドメイン `DomainAdapter`
（`StripFieldsAdapter`）を `synvelis-project` に実装する。
Track/Bus が共有する安定 `StripId` を scope root とし、
`StripDomainMask` が宣言した field family（Fader/Pan/Mute/Solo/
Inserts）のみを候補編集可能にする。channel field は singleton
`ParameterTarget`（`ControlOwner::Strip`）とその
automation/modulation closure、Inserts は strip の chain 全体の
`ProcessorChain` entity（processor 順序を含む）・processor・
port・control closure・完全内部 connection を scope 所有とする
（`scope_objects` が正本）。Send/Connection/
Sidechain/Bus 等の外部 routing は scope object にせず、
`boundary_dependencies` が typed `SemanticPortId` 境界で
strip-affiliated endpoint を持つ connection を inventory する。
mask 外 field・外部配線・scope entity 除去は全て型付き
fail-closed。adopt は既存 plan/preflight/CAS/Undo で宣言範囲
のみを 1 CAS で materialize する。schema/永続 model は非変更
（v14 据置）。実 plugin/PCM/OS/RT 受入は本スライス対象外
（`evidence/sv283/` で NotRun 記録）。

## Context

- Issue: synvelis#283（CB-06 Track/Bus 候補 Adapter）。
  `TrackId→StripId` を解決した明示 `StripDomainMask`、
  `mask 外 field は不変`、`mask 外 field への波及 → Reject`、
  `Routing 外部依存（send 先 strip が scope 外）→ 明示検証
  または Reject`、`Adoptは対象Mask内の全Fieldを1
  Project CAS/Undo`。
- AC: `CB-STRIP-RCP-01`、`CB-STRIP-01`〜`CB-STRIP-05`。
- CB-01（#278）が `VariantScope::StripFields`/`StripDomainMask`/
  `scope_objects` の所有権導出を、CB-04（#281）が
  processor-owned control op の confinement を、CB-05（#282）
  が typed boundary dependency inventory・外部配線 fail-closed・
  recording gate 共有を確立済み。本 adapter はそれらを再利用し、
  strip 固有の field mask gating・strip-affiliated routing 境界
  保護・bus/track 両 owner の adopt を追加する。

## Implementation

### Changes

- `variant/strip.rs`（新規）: `StripFieldsAdapter` —
  `DomainAdapter` 実装。`validate_scope`（scope kind・strip
  解決・非空 mask・`scope_objects` 全導出）、
  `fork_candidate`/`edit_candidate`/`preflight_adopt`
  （recording gate）、`capabilities`（`StripFieldsCapabilities`
  報告 = enforcement と一致）、`boundary_dependencies`
  （strip-affiliated `SemanticPortId` endpoint を持つ
  connection の型付き inventory）。
- `variant/adapter.rs`: `VariantScope::StripFields` の
  `check_op_targets` dispatch として
  `check_strip_fields_op_targets` を追加。受理面は
  strip channel control op（`SetStripMute`/`SetStripSolo` +
  CB-04/05 と同一の processor/target/automation/modulation
  op 集合を mask・owner で再解決）+ `InsertProcessor`。
  `is_strip_fields_op` predicate を追加。
- `variant/mod.rs`: `StripFieldsAdapter`・
  `StripFieldsCapabilities`・`StripBoundaryDependency` の export。
- `tests/cb06_strip_fields.rs`（新規）: headless fixture —
  CB-STRIP-01〜05 + CB-STRIP-RCP-01。
- `evidence/sv283/`（新規）: acceptance ledger・verification
  記録（OS-L1 headless、GUI-L2/EXT-L3/実 plugin は NotRun）。

### Decisions During Implementation

- **mask gating は scope_objects を正本に pre-edit で行う**:
  `SetStripMute`/`SetStripSolo` は常に scope 内の `Strip`
  entity を指すため `require_scope` だけでは mask 外 field の
  `ParameterTarget.base` 変更を止められない。op variant ごとに
  宣言 mask を照合し、未宣言 field family は `Unsupported`、
  scope 外 owner/role は `ScopeViolation` で型付き拒否する。
  `SetParameterBase`/automation/modulation op は対象 entity が
  `scope_objects` に含まれるかで自動的に mask 判定される
  （channel target は role→field 写像済み、processor control
  closure は Inserts 所有時のみ導出）。
- **Inserts 未宣言時の `InsertProcessor` は早期 `Unsupported`**:
  post-edit 生成検査でも `ScopeViolation` になるが、capability
  報告（`fx_insert`）と一致する明示拒否を pre-edit で行う。
- **routing op は全て `Unsupported`**: Send/Connection/
  Sidechain/Bus/Send tap/gain/monitor 等の外部配線は scope
  object にせず候補編集経路を全て閉じる（CB-05 と同契約）。
  strip 自身の main route・send・sidechain feed は
  `boundary_dependencies` が typed inventory し、shared
  context digest を動かして比較 context を stale にする。
- **boundary dependency の affiliated 判定**: scope 所有
  `SemanticPort`（Inserts 時の chain processor port）に加え、
  scoped strip 自身の port（`PortOwner::Strip`）を
  scope-affiliated として扱う — strip entity は常に scope
  object であり、その入出力 port は strip の識別子的一部。
  両端が scope object の connection（完全内部 chain 配線）
  のみを boundary から除外し、strip port を含む全ての跨ぎ
  connection（main route・send・sidechain feed・strip↔
  processor 混在）を inventory する。
- **modulation assignment は effect target/depth source の
  所有で判定する**: `strip_fields_objects`（CB-01 正本）は
  `ControlOwner::Strip` の source を全 mask で所有し、
  `ProcessorChain` 配下の processor-owned source/closure も
  Inserts 所有時に導出する。ただし assignment 自体は
  「effect target が scope 所有」かつ「depth_source が
  あればそれも scope 所有」のときのみ scope 所有とする
  post-filter を StripFields 側に置く — 共有
  `control_closure` の ANY-endpoint 規則は CB-04/05 契約
  維持のため変更しない。これにより strip/processor-owned
  source から未宣言 target への既存 assignment の
  `Set`/`RemoveModulationAssignment` が `ScopeViolation`
  で型付き拒否される。
- **recording gate は strip→lane 写像を共有化**: CB-05 と同じ
  `strip_recording_gate_lanes`（track strip → 所有 track の
  lane、bus strip → 全 project lane で fail-closed）を
  preflight_adopt で使用。

## Related Experiments

### Adopted

<!-- 実装中に採用した experiment log を記録する。 -->

### Rejected

<!-- 棄却した experiment log を記録する。 -->

設計分岐の検討記録は `docs/impl/exp/cb-06-track-bus-adapter.md`
に集約（strip port の scope-object 化・routing op の部分受理は
棄却 — 正本は `scope_objects` と共有 context 分離。
`ProcessorChain` の scope object 化は初版で棄却したが、
review で positional merge が「誰も author していない順序」
を生成し得ることが判明し、`Inserts` mask 時の scope 所有へ
revert — live reorder は `scope_fingerprint` を動かして
`StaleScope` で型付き拒否される）。

## Validation

- `mise run fmt-check` — Pass（exit 0）。
- `mise run lint synvelis-project` — Pass（clippy
  `-D warnings`、exit 0）。検出・修正: test 側
  `map_unwrap_or`（`map_or_else` 化）、`needless_borrow`、
  `too_many_lines`（fixture を `track_strip_chain`/
  `return_bus`/`key_strip`/`boundary_wiring` へ、
  mask/採用 edit 列を `scoped_*_edit`/`rich_candidate_edits`
  共有 helper と reject-op builder へ、adopt assertion を
  `assert_*` helper へ分割）、`items_after_statements`
  （`NoPlugins`/`YesPlugins` を module level へ）。
  `#[allow]` は不使用。
- `mise run test synvelis-project` — Pass（exit 0、全 suite
  green。`cb06_strip_fields.rs` 19 件、`cb04` 8 件・`cb05`
  13 件の回帰なし）。review-protocol の 2 独立レビューで
  modulation assignment の effect-target/depth-source
  confinement 欠落（P1 相当）と live chain reorder の
  staleness 欠落（P2 相当）を検出・修正し、回帰 test 4 件を
  追加。
- 詳細は `evidence/sv283/verification.md`。

## Risks

- `scope_objects`（CB-01 正本）と adapter の mask 判定が
  drift すると mask 外波及を見逃す — adapter は所有権を
  再導出せず `scope_objects` membership のみを参照する
  構造で抑止。
- strip port を scope object と誤認する変更が入ると外部
  route が候補所有化し共有 context 保護が崩れる —
  `cb_strip_05_shared_strip_model_unchanged_outside_scope`
  と adopt の connections/sends/strips serialize 比較で
  回帰検出可能。
- boundary inventory の見落としは false equivalence を生む
  — main route・send・sidechain feed・strip↔processor 混在を
  fixture で網羅済みだが、新規 connection kind 追加時は
  inventory の再検討が必要。

## Follow-ups

- Routing op（send/connection/sidechain/bus 編集）の候補
  受理は別 Issue の Routing 拡張 scope — 本 slice では
  `Unsupported` が正直な verdict。
- scope entity removal（processor/target/lane/source）の
  候補編集は後続 slice。
- 実 plugin/PCM/OS/RT 受入は #286/#287/#288 系・CB-STRIP-06
  実機 lane の責務（`evidence/sv283/` で NotRun 記録）。
