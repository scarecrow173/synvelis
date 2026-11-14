---
id: "TSPEC-034XmMj7mJXFMShCKT4zZW"
type: "test-spec"
status: "approved"
title: "CB-04 Instrument Slot 候補 Adapter 検証仕様（synvelis#281）"
created: "2026-11-14"
updated: "2026-11-14"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/281"
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
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/creative-branch-design.md"
  derived-by: []
  verifies:
    - "docs/specs/creative-branch.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# CB-04 Instrument Slot 候補 Adapter tests

## Purpose

synvelis#281 CB-04 スライスの headless fixture が保証すべき
振る舞いを固定する。`VariantScope::InstrumentSlot` の実
ドメイン `DomainAdapter`（`InstrumentSlotAdapter`）が、
同一 MIDI 入力を共有したまま Instrument Slot の plugin
state/control closure を候補ごとに独立保持し、semantic
diff・state 品質記録・capability 誠実性・adopt の CAS/
Undo・fail-closed reject を行う契約を検証する。実 OS/実
Plugin/実 RT/実 PCM・workspace/GPUI projection に依存する
判定は本 spec の対象外（`evidence/sv281/` で
NotRun/Deferred 記録、CB-INST-06 は全行 NotRun）。

## Feature

Instrument Slot 候補の fork → scoped edit → semantic diff
→ adopt → undo/redo ライフサイクルと、Missing Plugin・
State 品質・Recording gate の型付き reject。

## Rules

- Rule: candidate 編集は scope closure（Processor・
  processor-owned SemanticPort・`ControlOwner::Processor`
  の ParameterTarget/AutomationLane/ModulationSource/
  ModulationAssignment）内に限定され、scope 外 entity の
  生成・参照・削除は型付き reject される。
- Rule: 共有 MIDI source/clip・upstream strip/chain 構造・
  sibling processor の state は candidate 編集で不変。
- Rule: opaque plugin state は digest/`PluginRef`/binding/
  `StateCaptureQuality` の語彙のみ記録し、blob を
  deep-inspect せず品質を昇格しない。
- Rule: adopt は宣言 scope のみを 1 CAS + 1 Undo で
  materialize し、stale revision・scope 外参照・欠損・
  recording-open は fail-closed。
- Rule: capability 報告は enforcement と一致し、未対応
  surface は `Unsupported`/`Unavailable` で正直に返す。

## Examples

- Example (CB-INST-01): 同一 MIDI source を読む 2 候補で
  B の param/automation/modulation 編集が A・live・
  sibling・raw MIDI に波及せず、scope digest のみが動く。
- Example (CB-INST-02): Plugin 種類/Preset/State 差が
  Semantic Diff として返り、非公開 param は `Unavailable`、
  state 品質は `Coordinated`/`Quiescent`/`BestEffort`/
  `Unsupported`/`Failed` で verbatim 記録される。
- Example (CB-INST-03): pin した scope/shared-context
  digest に対し、共有 MIDI 編集が shared_context_digest を
  動かし stale expected revision の adopt を拒否する。
- Example (CB-INST-04): Adopt B が Slot+control closure の
  みを差し替え、Undo/Redo で採用前後の候補が維持される。
- Example (CB-INST-05): Missing Plugin・State 不能・
  Port Capability 変化・Recording 中 Adopt が型付き
  Reject/Unsupported/Pending を返し live を不変に保つ。
- Example (CB-INST-RCP-01): Missing Plugin × State/Port
  品質 matrix で Safe Edit/Adopt・Exact Artifact・
  `BitExact` regenerate の 3 区別が保たれる。

## Guarantees

- candidate の `Processor`/`plugin_instances`/control
  closure は fork clone として構造的に独立し、A/B で
  instance/state が干渉しない。
- `scope_digest` は scoped 編集のみで変化し、
  `shared_context_digest` は scope 外編集のみで変化する
  — 比較 identity として render-reuse semantics と整合。
- `PluginStateAssessment::exact_eligible()` は
  `Quiescent`/`Coordinated` のみ true — `BestEffort` は
  別環境 `BitExact` 再生成を保証しない。
- 全 reject は `VariantError` の型付き variant で返り、
  silent fallback・値の捏造・成功の偽装がない。

## Non-goals

- 実 VST3/CLAP/AU plugin の受入・外部 GUI・RT audio・
  PCM render（CB-INST-06・#288・#176 の責務）。
- Compare Orchestration 本体（CB-08 #285）・GPUI
  projection（CB-09 #286）・Action/Gateway（CB-10 #287）。
- opaque state blob の内容 diff・外部 plugin 内部状態の
  推定・3-way semantic merge・自動 blend。
- candidate 内での plugin binding/preset 差し替え op
  （`ProjectOp` surface 拡張が前提の別 Issue）。

## Risk

- fixture が headless Fake Host/mock `PluginStatePort` に
  依存するため、実 plugin host の state capture 品質・
  voice/tail 独立性・OS/Format 差は未検証のまま残る —
  `evidence/sv281/` の全 NotRun 行が後続 Gate の入力。
- `PluginInstance` の scope objects 非含有は既存
  `sync_binding` 契約への委譲であり、adopt materialize
  後の instance 整合が壊れた場合は CB-01 engine 契約の
  修正が必要になる。
