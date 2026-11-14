---
id: "IMPL-034ZxKpQ2mNv8sLw4tYh6bJ"
type: "impl"
status: "completed"
title: "CB-05 FX Processing Range 候補 Adapter — 安定境界・外部Routing fail-closed・A/B比較（synvelis#282）"
created: "2026-11-14"
updated: "2026-11-14"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/282"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-project/src/variant/processing_range.rs"
      - type: "file"
        path: "crates/synvelis-project/tests/cb05_fx_range.rs"
      - type: "file"
        path: "docs/impl/ir/cb-05-fx-range-adapter.md"
      - type: "file"
        path: "docs/impl/exp/cb-05-fx-range-adapter.md"
      - type: "file"
        path: "docs/test-specs/cb-05-fx-range-adapter.md"
      - type: "file"
        path: "evidence/sv282/acceptance-ledger.md"
      - type: "file"
        path: "evidence/sv282/verification.md"
    modified:
      - type: "file"
        path: "crates/synvelis-project/src/variant/adapter.rs"
      - type: "file"
        path: "crates/synvelis-project/src/variant/audio.rs"
      - type: "file"
        path: "crates/synvelis-project/src/variant/instrument.rs"
      - type: "file"
        path: "crates/synvelis-project/src/variant/mod.rs"
      - type: "file"
        path: "crates/synvelis-project/src/variant/scope.rs"
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
    - "docs/impl/ir/cb-04-instrument-adapter.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies:
    - "docs/test-specs/cb-05-fx-range-adapter.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted:
      - "docs/impl/exp/cb-05-fx-range-adapter.md"
    rejected: []
---

# CB-05 FX Processing Range 候補 Adapter — 安定境界・外部Routing fail-closed・A/B比較（synvelis#282）

## Summary

`VariantScope::ProcessingRange` の実ドメイン `DomainAdapter`
（`ProcessingRangeAdapter`）を `synvelis-project` に実装した。
Track Strip FX chain の連続 processor 区間を、永続位置ではなく
安定 semantic `ProcessorId`/`SemanticPortId` の `start`/`end`
境界で宣言し、range 内 processor・port・processor-owned
control closure・完全内部 connection のみを scope として所有
する。Send/Return/Sidechain/複数出力/外部 Connection・
zero-delay cycle が境界を跨ぐ場合は型付き inventory
（`boundary_dependencies`）として検出し、外部配線を触る全
`ProjectOp` は `Unsupported`/`ScopeViolation` で fail-closed
する。candidate 編集は CB-04 と同一の processor-owned control
op 集合 + `InsertProcessor`（post-edit scope 再導出で所有権を
判定）に限定し、adopt は既存 plan/preflight/CAS/Undo 機構で
宣言範囲のみを 1 CAS で materialize する。schema/永続 model
は非変更（v14 据置）。実 plugin/PCM/OS/RT 受入は本スライス
対象外（`evidence/sv282/` で NotRun 記録）。

## Context

- Issue: synvelis#282（CB-05 FX Processing Range）。
  `Stable StripId / ProcessorId / SemanticPortId + Before/After
  境界`、`初期Start=Chain入口またはProcessor間、End=
  Chain末尾`、`Send/Return/Sidechain/複数出力/外部Connection
  やzero-delay cycleが境界を跨ぐ場合はPreflight closureで
  検証`、`Adoptは指定範囲のみ1 CAS/Undoで更新`。
- AC: `CB-FX-RCP-01`、`CB-FX-01`〜`CB-FX-06`。
- CB-01（#278）が `VariantScope`/`DomainAdapter`/candidate
  clone/`edit_scoped_project_op`/adopt CAS を、CB-04（#281）
  が processor-owned control op の confinement と
  `PluginStatePort` 品質記録を確立済み。本 adapter はそれらを
  再利用し、range 固有の境界解決・外部配線拒否・boundary
  dependency inventory を追加する。

## Implementation

### Changes

- `variant/processing_range.rs`（新規）: `ProcessingRangeAdapter`
  — `DomainAdapter` 実装。`validate_scope`（scope kind・
  chain 解決・`range_processors` による安定 ID の連続区間解決・
  `end` が chain tail であることの初期契約検証・外部配線の
  安全性検査）、`fork`/`edit_candidate`/`preflight_adopt`
  （recording gate）、`capabilities`（`ProcessingCapability`
  報告 = enforcement と一致）、`boundary_dependencies`
  （境界を跨ぐ connection の型付き inventory）、
  `assess_range_state`（range 全 processor の plugin state
  品質を chain 順に verbatim 記録）。
- `variant/adapter.rs`: instrument 専用だった processor-owned
  control op 検査を `check_processor_control_op_targets` と
  して共有化し `InstrumentSlot`/`ProcessingRange` 双方へ
  dispatch。`is_processing_range_op`（control op 集合 +
  `InsertProcessor`）と `check_processing_range_op_targets`
  を追加 — `InsertProcessor` は range の chain のみ許可し、
  post-edit scope 再導出で範囲内に収まる場合のみ受理。
  `PreEditIds` に control entity 4 集合を追加し
  `check_created_within_scope` が新規生成 entity の scope
  内包を fail-closed 検証。
- `variant/scope.rs`: `range_processors` を `pub(crate)` 化
  （adapter からの再利用）。`processing_range_objects` は
  既存実装をそのまま利用 — range processor・semantic port・
  `control_closure`・両端 port が scope 内の connection のみ
  を所有集合とする。
- `variant/audio.rs`: `strip_recording_gate_lanes` を
  `pub(crate)` 共有 helper として追加（track strip → 所有
  track の lane、bus strip → 全 project lane で fail-closed）。
- `variant/instrument.rs`: 局所 `recording_gate_lanes` を
  共有 helper 呼出しへ置換（挙動不変）。
- `variant/mod.rs`: `ProcessingRangeAdapter`・
  `ProcessingCapability`・`ProcessingBoundaryDependency`・
  `ProcessingRangeRef` 等の export。
- `tests/cb05_fx_range.rs`（新規）: headless fixture 13 件 —
  CB-FX-01〜06 + CB-FX-RCP-01 + 境界検証 fail-closed。

### Decisions During Implementation

- **`end` は chain tail のみ受理**: Issue は「End=Chain末尾。
  任意EndはSchema表現可能だが本Issueでは初期受入不要」と
  規定。`ProcessingRangeRef` は任意 endpoint を schema 表現
  できるが、`end` が tail でない scope は `Unsupported` で
  型付き拒否（silent widen しない）。
- **`InsertProcessor` のみ構造 op として受理**: chain 内挿入は
  post-edit scope 再導出で「range の chain かつ
  `start..=end` 内に収まる」ことを判定できるため受理。
  別 strip/chain への挿入・範囲外位置は `ScopeViolation`。
  `RemoveProcessor`/`ReorderProcessor`/全 connection/send/
  sidechain op は外部配線・共有 chain 状態を不確定に触るため
  `Unsupported`（exp E2/E4 参照）。
- **境界を跨ぐ配線は共有 context として inventory のみ**:
  `boundary_dependencies` が sidechain feed 等を型付き列挙
  するが、scope objects には含めない — 候補が外部配線を
  変更する経路自体が全て reject のため、共有のまま安全に
  封じ込められる。配線変更は shared-context digest を動かし
  比較 context を stale にする（false equivalence なし）。
- **scope digest は順序非依存の object 集合**: range 内
  processor の並べ替えは scope identity を変えない
  （安定 ID が位置でなく実体を指すため）。ただし boundary
  を跨ぐ reorder は membership を変え scoped digest が動く
  — いずれも pin を stale にし silent equivalent にしない。
- **plugin availability は typed adopt gate**: scoped processor
  の resolved binding が unavailable なら `MissingPlugin` で
  adopt 前に fail-closed（部分変更なし）。project-truth の
  scoped edit 自体は runtime availability に依存しない
  （CB-04 と同契約）。
- **recording gate は strip→lane 写像を共有化**: range が
  複数 processor を跨ぐため、gate は range の chain が属する
  strip の lane で判定。bus strip 由来は全 lane を検査して
  fail-closed。

## Related Experiments

### Adopted

- `docs/impl/exp/cb-05-fx-range-adapter.md` E1（任意 End は
  `Unsupported` で defer）、E2（`InsertProcessor` のみ構造
  op 受理・post-edit scope 再導出）、E3（processor-control
  confinement の adapter.rs 共有化）、E4（boundary
  dependency は inventory のみ・配線 op 全 reject）、
  E5（recording gate lane 写像の共有）。

### Rejected

- 任意 mid-chain `end` の初期受理 — Issue が初期受入を
  明示的に除外。schema 表現は残し adapter が `Unsupported`
  を返す。
- `RemoveProcessor`/`ReorderProcessor` の候補受理 —
  chain membership/順序は共有 context であり、候補側での
  安全な confinement 表現がない（exp E2）。
- 境界跨ぎ connection を scope object 化 — 両端 port の
  片方が scope 外の connection は共有であり、候補編集経路を
  全て閉じることで安全側に倒した（exp E4）。
