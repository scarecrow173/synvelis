---
id: "TSPEC-034ZxKpQ2mNv8sLw4tYh6b"
type: "test-spec"
status: "approved"
title: "CB-05 FX Processing Range 候補 Adapter 検証仕様（synvelis#282）"
created: "2026-11-14"
updated: "2026-11-14"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/282"
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

# CB-05 FX Processing Range 候補 Adapter tests

## Purpose

synvelis#282 CB-05 スライスの headless fixture が保証すべき
振る舞いを固定する。`VariantScope::ProcessingRange` の実
ドメイン `DomainAdapter`（`ProcessingRangeAdapter`）が、
安定 semantic `ProcessorId` 境界で宣言した FX chain 連続
区間を候補ごとに独立保持し、外部 upstream/downstream・
共有 routing・strip/ bus 状態を不変に保ちつつ、processor-
owned control closure の scoped edit・外部配線の型付き
fail-closed・boundary dependency inventory・capability/
enforcement 一致・adopt の 1 CAS/Undo・runtime publish
receipt 分離を行う契約を検証する。実 OS/実 Plugin/実
RT/実 PCM・workspace/GPUI projection に依存する判定は
本 spec の対象外（`evidence/sv282/` で NotRun 記録、
CB-FX-06 の実 plugin/PCM/PDC/RT lane は全行 NotRun）。

## Feature

FX Processing Range 候補の fork → scoped edit → adopt →
undo/redo → save/reload ライフサイクルと、外部配線・
Missing Plugin・State 品質・Recording gate・stale scope の
型付き reject。

## Rules

- Rule: range は 1 chain 内の連続 `ProcessorId` 区間として
  解決され、`start`/`end` は安定 semantic ID のみ —
  ordinal・runtime `NodeId`/`PortId` を永続 identity に
  しない。`end` は初期契約で chain tail のみ受理
  （mid-chain end は `Unsupported`）。
- Rule: scope objects は range processor・その semantic
  port・processor-owned control closure（ParameterTarget/
  AutomationLane/ModulationSource/ModulationAssignment）・
  両端 port が scope 内の connection のみ。境界を跨ぐ
  connection・send・sidechain・外部 routing は共有
  context であり `boundary_dependencies` が型付き列挙する
  が scope object にはしない。
- Rule: 候補編集は processor-owned control op 集合 +
  `InsertProcessor`（post-edit scope 再導出が範囲内と
  判定した場合のみ）に限定。外部配線・remove/reorder・
  foreign chain 構造・strip/bus・共有 context 編集は
  `Unsupported`/`ScopeViolation` で型付き拒否され、
  candidate/live の部分変更を残さない。
- Rule: adopt は宣言 range のみを 1 CAS + 1 Undo で
  materialize し、stale scope/project revision・未解決
  lineage・欠損 plugin・外部依存違反・recording-open は
  fail-closed。runtime publish は別 receipt domain —
  失敗/pending は project commit を rollback しない。
- Rule: capability 報告（`ProcessingCapability`）は
  enforcement と一致し、未対応 surface は
  `Unsupported`/`Unavailable` で正直に返す。

## Examples

- Example (CB-FX-01): chain 入口→末尾と任意 processor
  境界→末尾の 2 scope が fork 可能。mid-chain end・
  reversed order・未知 processor ID は型付き拒否。
  重複 range は `ScopeConflict`。
- Example (CB-FX-02): 共有 upstream（range 前 processor）
  の live 編集が両 candidate の shared-context digest を
  同一に動かし、B の scoped 編集は A・live・共有
  downstream を不変に保つ。
- Example (CB-FX-03): range 外への FX 追加は scope digest
  を動かさず shared-context のみ stale。境界削除は
  `NotFound`（Unresolved）、非境界 member 削除は scope
  digest 移動 → adopt `StaleScope`。reorder は順序非依存
  の scope identity を保ち、boundary 跨ぎは membership
  変化として scoped digest が動く。
- Example (CB-FX-04): sidechain feed が boundary
  dependency として検出・共有のまま。境界 connection・
  新規外部配線・send・foreign insert・strip mute の全
  candidate op が型付き拒否。adopt は connection を
  一切生成/変更しない。
- Example (CB-FX-05): adopt が range の control state +
  範囲内 insert のみを materialize（1 CAS/1 Undo）、
  save/reload で variant state が round-trip。stale
  scope digest・recording-open・runtime publish 失敗は
  それぞれ `StaleScope`/`RecordingActive`/別 receipt
  `Failed` で project commit を守る。
- Example (CB-FX-06): candidate は独立 `plugin_instances`
  identity を保持。`assess_range_state` が chain 順に
  quality を verbatim 記録（`BestEffort` は
  `exact_eligible()=false`、`Failed` は型のまま）。
  plugin unavailable は adopt を `MissingPlugin` で
  fail-closed、available なら project-truth adopt は
  進行。実 plugin/PCM/PDC/RT は NotRun。
- Example (CB-FX-RCP-01): captured payload digest が
  store key として不変（live 編集で candidate bytes は
  変わらない）。render pin は scope/shared-context digest
  の合成で RCP cutset 非依存。semantic diff が scoped
  変更を型付き `Modified`/`Added` として報告。
