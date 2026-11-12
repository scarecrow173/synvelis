---
id: "DESIGN-TiM02PpqnSmfXg02"
type: "design"
status: "approved"
title: "TIM-02 × TIM-01 PPQN×SMF cross-golden fixture — 設計"
created: "2026-11-12"
updated: "2026-11-12"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/126"
    - "https://github.com/scarecrow173/synvelis/issues/125"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/tim-02-ppqn-smf-cross-golden.md"
  implemented-by: []
  depends-on:
    - "docs/specs/tim-02-ppqn-smf-cross-golden.md"
    - "docs/specs/tim-02-smf-format01-import-export.md"
    - "docs/specs/tim-01-configurable-ppqn-project-command.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/tim-02-smf-format01-core.md"
    - "docs/designs/configurable-ppqn-musical-time-architecture.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/tim-02-ppqn-smf-cross-golden.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/tim-02-ppqn-smf-cross-golden.md"
  references:
    - "https://github.com/scarecrow173/synvelis/issues/126"
  defers: []
  deferred-by: []
---

# TIM-02 × TIM-01 PPQN×SMF cross-golden fixture — 設計

## Context

TIM-02 の受入行は「PPQN 異種の Import / Project Change /
Save / Export / 再 Import」の全鎖を要求するが、merge 済み
fixture は鎖を 2 分割して検証していた:

- `tim_02_smf.rs`: SMF import→provenance→export→reimport・
  chase — `SetProjectPpqn` を通さない。
- `tim_01_ppqn.rs`: `SetProjectPpqn` Exact/Round・
  save/load・undo — `midi_sources`（AddMidiSource 由来）
  のみを検証し、`smf_documents` provenance と export
  golden を通さない。

`ppqn.rs` の `reproject_ppqn_with` は musical tick domain
（tempo/signature event・musical clip anchor/extent・
comp range）のみを走査し、`midi_sources`・`smf_documents`・
recording frame・automation `BeatPoint` を構造的に
re-projection 対象外とする。一方 `export_smf` は
`project.midi_sources` と `project.smf_documents` のみを
読み、`project.ppqn`/`tempo_map` を一切参照しない。
したがって「PPQN 変更は SMF 鎖に対して恒等」であることが
設計上の不変条件であり、本 slice はそれを end-to-end の
observable 挙動として固定する。

## Scope

### In Scope

- `crates/synvelis-project/tests/tim_02_ppqn_smf.rs`（新規）:
  SMF byte builder（`tim_02_smf.rs` と同一 wire 語彙の
  最小集合）・Format 1 division-480 fixture・controller
  helper・2 テスト。
- doc chain 6 件（`tim-02-ppqn-smf-cross-golden*`）。

### Out of Scope

- `crates/synvelis-project/src/` の変更 — fixture が実
  不変違反を露呈した場合のみ例外的に許容（根拠を impl
  IR に記録）。本 slice では変更不要と判断。
- `ops.rs`/`control.rs`・`tests/ctl_02.rs`（consume-only）。
- app/player/recording/workspace/gateway 各 crate・
  gateway lowering・app op・player clip→MIDI 配線。
- 実機/実 backend・AudioLens f64・SMPTE/Format2/MIDI2
  （NotRun 委譲）。

## Components and Boundaries

- **fixture（唯一の新規コード）**: 公開 API のみを消費する
  - `parse_smf` / `ProjectOp::ImportMidiSmf` /
    `ProjectOp::AddTrack`/`AddLane`/`AddClip` /
    `ProjectOp::SetProjectPpqn` / `export_smf` /
    `ProjectController::{apply,save,load}` /
    `InMemoryStore`。
  - SMF byte builder は `tim_02_smf.rs` の private helper
    と同じ wire 語彙を test file 内に複製する（test 間
    共有 module を新設しない — 既存 convention 通り
    file-local）。
- **検証境界**: 「source-native slice の不変性」は
  `smf_documents[source]`・`midi_sources[source]` の
  clone 比較で、「export の PPQN 非依存」は bytes の
  3 時点一致（変更前・変更後・save/load 後）で、「文書
  保存」は `parse_smf(export bytes) == doc` の
  `PartialEq` と re-import 後の stored doc 比較で固定
  する。

## Fixture 設計判断

- **division 480 vs project PPQN 960**: `Project::new`
  の既定 PPQN は 960。division 480 の Format 1 を
  import すると source-native division ≠ project PPQN
  の「PPQN 異種」が自然に成立し、追加の細工が不要。
- **tempo×2 + CC64×3**: conductor track に tempo
  500000@0 と 400000@960（phrase 途中の tempo 変更）、
  ch0 に CC64 127→64 の sustain 変化、ch1 に CC64 64 —
  provenance 保持を event kind 集計（Tempo=2・
  `Cc{controller:64}`=3）で observable にする。
- **clip anchor は whole-beat**: Exact chain では
  anchor 3840@960（=4 beat）・extent 1920（=2 beat）で
  960→480 が厳密に成立。Round chain では anchor tick 1
  （=1/960 beat、@1000 で 25/24 tick の inexact）を使い
  Exact の原子的 reject と Round(Floor) の宣言済み
  量子化を両方示す — いずれも SMF 側には無関係。
- **export golden は Format 1 @480**: source の native
  division と同一の export division を選び、変換が
  全て exact（`rounded_count=0`・`max_error=0`）である
  ことを `ConversionReport` で確認した上で bytes 一致
  を主張する。Format 0 merge・非等倍 division の
  rounding は `tim_02_smf.rs` の守備範囲であり本
  slice では重複検証しない。
- **note 射影比較は内容 tuple**: `NoteId` は controller
  の id generator が draw する実装詳細であり、契約は
  `(key, velocity, start, len)` の multiset —
  `NoteId` 非依存の tuple 比較で re-import の等価性を
  主張する。

## Data and Control Flow

```
parse_smf(format1_division480)          // division 480 doc
  → ImportMidiSmf                        // midi_sources + smf_documents
  → AddTrack/AddLane/AddClip             // musical anchor on project grid
  → export_smf(Format1, 480)  [golden A] // source-native only
  → SetProjectPpqn(960→480, Exact)       // clip anchor/extent halve
       └─ smf_documents/midi_sources unchanged (assert)
  → export_smf(Format1, 480)  [golden B] // bytes == golden A
  → save → load                          // snapshot round-trip
  → export_smf(Format1, 480)  [golden C] // bytes == golden A
  → parse_smf(golden C) == doc           // tempo/CC64/order preserved
  → ImportMidiSmf(reparsed)              // doc equality + note tuples
```

Round chain は同じ流れで anchor を inexact にし、
`Exact` reject → `Round(Floor)` 適用 → 同一の不変条件
assert を行う（project PPQN=1000 でも export@480 は
byte 不変 — export が `project.ppqn` を参照しない
ことの追加証拠）。

## Non-Functional Constraints

- 公開 API のみ消費 — `src/` への依存は既存の
  `synvelis-project` 公開面に限定し、private 領域に
  触れない。
- 決定性 — fixture bytes は手組みの deterministic
  builder、export は deterministic writer、id は
  `CounterIdGenerator` で replay 可能。
- 検証の比例性 — 2 テストに全鎖を集約し、既存
  `tim_01_ppqn.rs`/`tim_02_smf.rs` と重複する単体
  契約（parse 異常系・chase・undo 等）は再検証しない。

## Risks and Trade-offs

| リスク | 対策 |
| --- | --- |
| fixture が実不変違反（実バグ）を露呈する | その場合のみ `src/{project,smf,ppqn}.rs` 変更を許容し根拠を impl IR に記録 — 本 slice では発生せず |
| golden bytes が実装詳細に過剰依存する | bytes 一致は「PPQN 変更・save/load を跨いだ不変性」の証明であり、wire 語彙自体の正しさは `tim_02_smf.rs` の parse/export golden が担保 — 本 fixture は同一 exporter の再現性のみを主張 |
| `NoteId` 比較による偽陽性/偽陰性 | note 投影は `(key, velocity, start, len)` tuple の sorted 比較で `NoteId` 非依存に |

## Deferred Design Concerns

- player/app 層の PPQN×SMF 結合（clip→MIDI 配線・
  gateway lowering `synvelis.midi.smf.import`・app op
  `syn.project.import_smf`/`export_smf`）— 他 Worker
  占有・別 slice。
- 実機 MIDI timing・AudioLens f64・SMPTE/Format2/MIDI2
  — NotRun/Unsupported 委譲。

## Verification Notes

- [x] `cargo test -p synvelis-project --test
      tim_02_ppqn_smf` — 2 件 green
- [x] `cargo clippy -p synvelis-project --all-targets --
      -D warnings` — clean
- [ ] `mise run` 全検証 lane — 結果は
      `docs/impl/ir/tim-02-ppqn-smf-cross-golden.md` に記録
