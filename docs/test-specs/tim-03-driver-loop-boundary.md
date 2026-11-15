---
id: "TSPEC-034XI9qZiCV70sHFCsj1s7"
type: "test-spec"
status: "approved"
title: "tim-03-driver-loop-boundary"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/127"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on:
    - "docs/designs/tim-03-driver-loop-boundary.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/tim-03-driver-loop-boundary.md"
  derived-by: []
  verifies:
    - "docs/specs/tim-03-driver-loop-boundary.md"
  verified-by: []
  references:
    - "docs/designs/tim-03-driver-loop-boundary.md"
  defers: []
  deferred-by: []
---

# Test Spec — TIM-03 driver-side loop-wrap boundary derivation

Feature: `HeadlessDriver` の sessioned attached node に対し、driver が
`WrapSim` で導出した `LoopWrap` `BoundaryPoint` を `stage_window` の
`boundaries` に供給し、oracle が評価した applied offset が実 scheduler
の wrap 位置と一致すること。

Fixture: `tests/tim03_driver_loop_boundary.rs`。recording plugin が
各 physical `process` call の folded `TransportInfo.position` と
frame 数を記録（tim03_loop_boundary の `CallPos` と同型）。call start
を frame 数で累積すると transport-span 境界（command apply・loop
wrap）に一致するため、window 内の interior cut = 実 scheduler が
実行した wrap frame として再構成できる。driver command は全て
`due = 0` head apply のため interior cut は wrap のみ。

## Rules

- **DB-01** activation anchor（Seek+Play head apply）後の steady
  loop: `set_loop(100,200)` を各 window で再 stage し、
  `attached_window_plan(node).boundaries` がその window の interior
  wrap offset と一致。連続 window・同一 window 複数 wrap を含む。
- **DB-02** window-end wrap は次 window の boundary: `seek(72)` で
  raw position を loop start 未満に置き、loop end へ window end
  ちょうどで到達させる。stage_seek の plan は interior boundary を
  持たず、次 window では head wrap（offset 0 → applied set 外）+
  offset 100 の wrap が導出される。
- **DB-03** no loop / Stopped: `clear_loop`（region 無し）と
  `panic`（Stop）で導出 boundary は空、plugin は unsplit span を観測。
- **DB-04** mid-stream `SetLoop` region 変更（position が新 start
  未満 → 新 end まで上昇）と `ClearLoop`（以降 wrap 消失・raw unfold）。
- **DB-05** active loop 中の `seek(500)`: head での same-head
  cascade が algebraic に畳まれ、head wrap（offset 0）+ interior
  wrap（offset 100）が導出される。
- **DB-06** `boundary: None` node: session 無し・`attached_window_plan`
  は `None`、legacy lenient 経路維持（scheduler 自体の wrap 分割は
  不変）。
- **DB-07**（DB-01..06 に包含）oracle legality: `stage_window` が
  受理した plan の `boundaries` は全て `[0, window_frames)` で
  sort/dedup 済み — `plan_boundaries` helper が範囲を assert。
- **DB-08**（DB-01/02/04/05 に包含）span cut 一致: 各 window で
  `plan.boundaries`（oracle が見た applied offset）== plugin 観測
  interior cut。これが「導出 offset が実 scheduler 挙動と一致」の
  直接証拠。

## Examples

- `tim03_dlb_01_activation_anchor_and_steady_loop`: [0,128) anchor →
  [128,256) wrap@72 → [256,384) wrap@44 → [384,512) wraps@{16,116}
  → [512,640) wrap@88。各 window で plan.boundaries == observed cuts。
- `tim03_dlb_02_window_end_wrap_belongs_to_next_window`: seek(72) →
  [72,200) は wrap@200（=window end）を含まず 1 span；[200,328) は
  head wrap + wrap@300（offset 100）。
- `tim03_dlb_05_seek_into_active_loop_cascades`: seek(500) → head
  cascade で pos 100、wrap@600（offset 100）。

## NotRun

実 CLAP/VST3/AU plugin、実 Host/OS backend、GUI-L2/EXT-L3、実機、
precision certification/CapabilityMatrix、consumer oracle
（AudioLens f64/native PCM）は本 slice 外 gate — headless fake
plugin のみで検証する。
