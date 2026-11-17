---
id: "DESIGN-034ZjJTBCxSxqBr82J1mjO"
type: "design"
status: "approved"
title: "Issue #283 slice — strip-fields gateway exercise design"
created: "2026-11-17"
updated: "2026-11-17"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/283"
    - "docs/specs/issue-283-strip-gateway-exercise.md"
    - "docs/discovery/issue-283-strip-gateway-exercise.md"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/issue-283-strip-gateway-exercise.md"
  implemented-by:
    - "docs/plans/issue-283-strip-gateway-exercise.md"
  depends-on:
    - "docs/specs/issue-283-strip-gateway-exercise.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/cb-06-track-bus-adapter.md"
  refines:
    - "docs/specs/issue-283-strip-gateway-exercise.md"
  refined-by: []
  derives-from:
    - "docs/specs/issue-283-strip-gateway-exercise.md"
  derived-by:
    - "docs/plans/issue-283-strip-gateway-exercise.md"
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# Issue #283 slice — strip-fields gateway exercise design

## Context

`StripFieldsAdapter`（`synvelis-project/src/variant/strip.rs`）と
`VariantPorts::standard` への登録は main 着地済み。残る gap は
gateway commit-gate 経路での strip-fields exercise のみ。
`cb10_variant_actions.rs` は audio-clip scope、`cb11_ai_vcs_off.rs`
は audio + arrangement scope を `VariantPorts::standard` 経由で
駆動しており、strip-fields は登録済みだが commit-gate 経路で
未検証だった。

## 設計判断

### 成果物: 新規 `cb06_strip_gateway.rs`

`cb11_ai_vcs_off.rs` への追記ではなく新規 test file を選ぶ。
cb11 は VCS-off lifecycle（audio + arrangement）の関心事を担い、
strip fixture（`ProjectController` による `AddTrackStrip` +
`InsertProcessor` seed）は helper 群と異なる構築経路を持つ。
分離により既存 test の前提に波及しない。

### Fixture: test-local strip project（revision 2）

`demo_project()` は track を持つが `p.routing.strips` を構築
しない。`ProjectOp::AddTrackStrip { track_id: TrackId::new(3) }`
が strip・chain・main ports・seeded `Fader`/`Mute`/`Solo`
channel targets・master への main route を mint し、
`ProjectOp::InsertProcessor` が `Inserts` mask の owned objects
（processor + ports）を与える。2 op で head revision 2 の
project を `DirectoryBundleStore` + `MemorySnapshotSource` に
pin する。`rig/mod.rs` の共有 fixture は変更しない
（serialize 比較・connection 数前提への波及を避ける）。

### Wire 表面の実測に基づく edit 証拠の形

`lower_for` の capability 表に strip-domain inner op
（`SetStripMute`/`SetParameterBase`/`InsertProcessor` 等）へ
lower する capability は存在しない。positive mask-gated edit は
wire 上表現不能であり、edit 面の証拠は:

- 全 ordinary op（`synvelis.clip.mute`・`synvelis.track.create`・
  `synvelis.clip.move`）が candidate branch で
  `PreparationRejected`/`variant-unsupported` に typed reject
  されること
- routing/send/bus op は capability 自体が存在せず fail-closed
  であること（CB-STRIP-03 の境界 — external wiring は
  candidate-owned でない）
- reject が live revision・branch content revision・対象
  entity を一切変更しないこと

adopted branch への edit は `EditAdopted` として live へ直接
適用される設計のため、confinement 負例は candidate branch
に対してのみ構成する。

### Adopt: approval ticket + scope digest pin

`adopt_guard` は空 approval ticket を `VariantError::Unapproved`
で reject するため、positive adopt は cb10/cb11 先例どおり
`ApprovalPolicy::ApproveToApply` の agent grant で
prepare → `ApprovalRequired` → human approval → apply（ticket
消費）→ `CommitGate::commit` の経路を取る。

`expected_scope_digest`・`expected_branch_revision` は
`AdoptIntent` の pin であり、stale 値は prepare simulation の
`adopt_branch_adapted` が `variant-stale-scope` /
`variant-stale-branch-revision` に typed reject する。
scope-identical candidate でも adopt は `Applied`（1 undo
step）— adopted-pointer swap と frozen previous branch
（`Captured` payload）が実 project truth として commit される。

### Query 面: typed decode + verified compare

`synvelis.branch.{list,describe,compare}` は scope kind filter・
scope round-trip（strip tagged id + fields mask）・
`scope_digests_equal`・`verified` decode を返す。`compare` は
captured payload を shared store から decode + scope digest
再計算して `verified: true` を報告する — claim を信用しない。

### Domain-level 補助証拠

`StripFieldsAdapter::boundary_dependencies`（associated fn）
が strip→master `Main` connection のみを scope-affiliated
boundary（`source_in_scope && !destination_in_scope`）として
報告し、`capabilities` が declared mask に対し field family
Supported・routing/entity-remove/preset-select Unsupported を
報告することを別 test で pin する（CB-STRIP-02/03 の
display truth = enforcement truth）。

## 却下した代替案

- `rig/mod.rs` の `demo_project()` に strip seed を追加する案:
  既存 gateway test の serialize 比較・connection 数前提への
  波及リスクがあり、lease も「波及確定時のみ」。test-local
  fixture で同等の検証が可能。
- human apply での adopt: `adopt_guard` が空 ticket を
  `Unapproved` で reject するため不可。agent grant +
  approval receipt 経路が唯一の positive 経路。
- mask 未宣言 field への edit 負例: wire 表面に strip op
  capability が無く構成不能。ordinary op の typed reject で
  confinement の fail-closed を代替証明する。
- `cb11_ai_vcs_off.rs` への追記: 関心事が異なり、strip
  fixture の構築経路が既存 helper と混ざる。新規 file が
  小さく整合的。

## Verification Notes

- [x] `mise run check synvelis-gateway` — 新規 test の
  コンパイル
- [x] `cargo test --test cb06_strip_gateway`（補助証拠）—
  2 test Pass
- [ ] `mise run fmt-check` / `lint synvelis-gateway` /
  `test synvelis-gateway` / `check-headless-deps` /
  `check synvelis-project` / `prek run --all-files`
