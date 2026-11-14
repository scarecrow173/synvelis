---
id: "IMPL-034XmKS2VgTv6sQ2onvxNY"
type: "impl"
status: "completed"
title: "CB-04 Instrument Slot 候補 Adapter — MIDI共有・独立Plugin State・A/B比較（synvelis#281）"
created: "2026-11-14"
updated: "2026-11-14"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/281"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-project/src/variant/instrument.rs"
      - type: "file"
        path: "crates/synvelis-project/tests/cb04_instrument_slot.rs"
      - type: "file"
        path: "docs/impl/ir/cb-04-instrument-adapter.md"
      - type: "file"
        path: "docs/impl/exp/cb-04-instrument-adapter.md"
      - type: "file"
        path: "docs/test-specs/cb-04-instrument-adapter.md"
      - type: "file"
        path: "evidence/sv281/acceptance-ledger.md"
      - type: "file"
        path: "evidence/sv281/verification.md"
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
        path: "docs/impl/exp/README.md"
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
    - "docs/impl/ir/cb-03-midi-clip-adapter.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies:
    - "docs/test-specs/cb-04-instrument-adapter.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted:
      - "docs/impl/exp/cb-04-instrument-adapter.md"
    rejected: []
---

# CB-04 Instrument Slot 候補 Adapter — MIDI共有・独立Plugin State・A/B比較（synvelis#281）

## Summary

`VariantScope::InstrumentSlot` の実ドメイン `DomainAdapter`
（`InstrumentSlotAdapter` + `InstrumentSlotCapabilities`）を
`crates/synvelis-project/src/variant/instrument.rs` に実装し、
共有 confinement helper（`check_op_targets` /
`check_created_within_scope`）へ processor-owned control entity
（ParameterTarget/AutomationLane/ModulationSource/
ModulationAssignment）の target 解決と新規生成検査を追加した。
同一 MIDI 入力を共有したまま Instrument Slot の plugin
binding/preset/param/automation/modulation state を候補ごとに
独立保持し、semantic diff・state 品質記録・capability 誠実性・
1 CAS + 1 Undo adopt・recording/missing-plugin/state の型付き
reject を headless fixture 8 件で検証した。

## Context

synvelis#281 CB-04 スライス。CB-00（#422）統合設計・CB-01
（#278, PR #425）VariantSet/Branch/Scope/engine・CB-02
（#279, PR #437）`AudioClipAdapter`/`TakeUsageAdapter`・CB-03
（#280, PR #440）`MidiClipAdapter`/`MidiClipCapabilities`・
RCP-00/01/02/04（#421/#431/#436/#426）の read adapter /
plugin-state 契約を前提とする。本スライスは headless の実
ドメイン Adapter + fixture に限定 — Compare Orchestrator
（#285）・GPUI（#286）・Action/Gateway 統合（#287）・実機
受入（#288・CB-INST-06）は後続 Issue。

## Implementation

### Changes

- `src/variant/instrument.rs`（新規）: `InstrumentSlotAdapter`
  — `validate_scope` は `validate_scope_structure` + scope
  variant 型照合 + processor 存在確認 + `scope_objects`
  （`processor_objects` = Processor・processor-owned
  SemanticPort・`ControlOwner::Processor` control closure）。
  scope variant 不一致は `InvalidScope`、processor 不在は
  `NotFound` で鏡像拒否（CB-02/03 と同型）。`fork_candidate`
  は `fork_candidate_project` 共用 — candidate は full project
  clone として独自の `Processor`/`plugin_instances`/control
  closure を保持し A/B state は構造的に干渉しない。
  `edit_candidate` は `BranchEditOp::Project` のみ受理し、
  instrument control op（SetParameterBase /
  Add|RemoveParameterTarget / Add|RemoveAutomationLane /
  Write|EraseAutomationRegion / Add|RemoveModulationSource /
  Add|Set|RemoveModulationAssignment）を
  `edit_scoped_project_op` + adapter 固有 confinement で
  scope closure 内に限定。routing/connection/send/bus・
  strip/track/clip/source・processor insert/remove/reorder・
  `Composite` は `Unsupported` で fail-closed。
  `preflight_adopt` は CB-02/03 先例の lane recording-open
  gate + `preflight_plugins`/`preflight_ports`/
  `preflight_assets` を既存 `adopt_branch` 経路で継承。
- `InstrumentSlotCapabilities`: param_base/automation_edit/
  modulation_edit=Supported、plugin_rebind/preset_select/
  routing/midi_input=Unsupported の誠実な能力報告
  （display truth = enforcement truth、CB-INST-02）。
- `InstrumentSlotAdapter::assess_state`: `PluginStatePort`
  mock 経由で per-processor `PluginStateBinding` +
  `PluginStateOutcome`（Captured{achieved quality}/
  Unsupported/Failed{kind}）を `PluginStateAssessment` として
  記録 — achieved quality を verbatim 保持し
  `exact_eligible()` は `Quiescent`/`Coordinated` のみ true
  （`BestEffort` は決して昇格しない、TIM-04 契約）。
  opaque blob は deep-inspect せず digest/`PluginRef`/
  binding/manifest 語彙のみを扱う。
- `src/variant/adapter.rs`: `PreEditIds` に
  `control.targets`/`control.lanes`/`control.sources`/
  `control.assignments` の 4 集合を追加し、
  `check_created_within_scope` が新規 control entity id を
  scope owned set と照合。`check_op_targets` に instrument
  scope の target 解決を追加 — 既存 entity を参照する op は
  `require_scope`（scope closure 外 → `ScopeViolation`）、
  新規生成 op は owner が scope の `ProcessorId` を指す
  場合のみ許可。confinement 規則は CB-02/03 と同一 helper
  を共有し drift しない。
- `src/variant/mod.rs`: `mod instrument` +
  `InstrumentSlotAdapter`/`InstrumentSlotCapabilities`/
  `PluginStateAssessment` 系 re-export。
- `tests/cb04_instrument_slot.rs`（新規）: 8 fixture —
  CB-INST-01/02/03/04/05×3 + CB-INST-RCP-01 matrix。

### Decisions During Implementation

- **plugin/preset 変更 op は存在しない**: binding/preset を
  差し替える専用 `ProjectOp` variant は現行 model に無い。
  candidate 内の plugin/preset 差分は fork 時点の live clone
  を出発点とし、`Processor` entity 自体の書換 op も scope
  外のため `Unsupported` — plugin 種類/Preset 差は
  semantic diff 側（`diff/adapters.rs` の processor adapter
  + `PluginInstance` diff）が報告し、candidate 側での
  binding 変更は後続 op surface 拡張の課題として exp/ledger
  に記録。
- **`PluginInstance` を scope objects に追加しない**:
  `ObjectKind::PluginInstance` は keyed-synthetic kind で
  `adopt_supported_kind` 外。`PluginInstanceIdentities::
  sync_binding` が Processor write ごとに Resolved→`ensure`/
  Unresolved→`remove` で同期する既存契約に委ね、adopt
  materialize 後の `plugin_instances` 整合を fixture で
  検証（CB-INST-01/04）。scope objects への追加はこの既存
  契約との整合で判断し根拠を本書に記録。
- **scope-entity removal は `Unsupported`**: candidate は
  fork 時点の scoped content を出発点に追加・変更できるが、
  owned object の削除は adopt 時の参照整合を壊し得るため
  fail-closed（CB-INST-05 で typed reject を検証）。
- **recording gate は owning lane 解決**: adopt 対象 slot の
  owning strip→track lane で recording session が Open の
  ままの採用は `RecordingActive` で先行拒否（CB-02/03 先例
  の `lane_recording_open` を processor→strip→track で
  解決して再利用）。
- **Missing Plugin は project truth を block しない**:
  `preflight_plugins` の既存契約どおり、candidate が新規
  Unresolved binding を導入しない限り runtime availability
  は project blocker ではない — execution 解決は RCP-04
  player 側の別 receipt domain（read-only 消費）。
  `NoPlugins` availability 下でも scoped edit/adopt は
  project-truth として進行することを CB-INST-05 で検証。

## Related Experiments

### Adopted

- `docs/impl/exp/cb-04-instrument-adapter.md` — plugin/preset
  変更 op 不在の扱い・`PluginInstance` scope 外判断・
  control entity confinement の helper 共有設計（いずれも
  採用）。

### Rejected

- なし（代替案比較は exp E1〜E3 に記録）。

## Validation

Windows 11 / mise 管理 toolchain で実行（詳細は
`evidence/sv281/verification.md`）:

- `mise run fmt-check` — Pass
- `mise run check synvelis-project` — Pass（`--locked`）
- `mise run lint synvelis-project` — Pass
- `mise run test synvelis-project` — Pass（全 test binary
  green、新規 8 fixture 含む、CB-01/02/03 回帰なし）
- `mise run check synvelis-workspace` — Pass
- `mise run check synvelis-render` — Pass

## Risks

- candidate 内での plugin binding/preset 差し替え op が
  `ProjectOp` に存在しない — CB-INST-02 の semantic diff は
  fork 時点差分のみ報告可能で、candidate 側 binding 変更は
  op surface 拡張（別 Issue）が前提。現状は `Unsupported`
  が正直な verdict。
- opaque plugin state blob の内容 diff・外部 plugin 内部
  状態の推定は対象外 — digest 一致/不一致と `StateCaptureQuality`
  区分のみを記録し、blob を deep-inspect しない契約を維持。
- 実 VST3/CLAP/AU 受入（CB-INST-06）・外部 plugin GUI
  （#176）・実 PCM/RT render・voice/tail の実機独立性は
  NotRun/Deferred — headless PASS を実機 PASS に昇格しない。
  OS・Format 別受入対応表は `evidence/sv281/` に全行
  NotRun で記録。

## Follow-ups

- CB-05〜08（#282〜#285）残ドメイン Adapter・Compare
  Orchestrator — 本 adapter の scope/shared-context digest
  pin semantics を比較 context identity として消費する。
- plugin binding/preset 差し替えの `ProjectOp` surface —
  candidate 内 plugin 変更が必要になった時点で別 slice。
- engine の set 内 branch 追加 op（複数同時 candidate）—
  CB-03 と同じく後続 engine slice の課題。
- CB-INST-06 実 plugin 受入・Cross-Gate（#288）— #114・
  後続 Gate の責務。
