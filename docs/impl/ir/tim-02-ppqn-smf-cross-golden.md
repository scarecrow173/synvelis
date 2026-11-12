---
id: "IMPL-TiM02PpqnSmfXg06"
type: "impl"
status: "completed"
title: "TIM-02 × TIM-01 PPQN×SMF cross-golden fixture — tim_02_ppqn_smf.rs 2 tests"
created: "2026-11-12"
updated: "2026-11-12"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/126"
    - "https://github.com/scarecrow173/synvelis/issues/125"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-project/tests/tim_02_ppqn_smf.rs"
      - type: "file"
        path: "docs/specs/tim-02-ppqn-smf-cross-golden.md"
      - type: "file"
        path: "docs/designs/tim-02-ppqn-smf-cross-golden-design.md"
      - type: "file"
        path: "docs/plans/tim-02-ppqn-smf-cross-golden-plan.md"
      - type: "file"
        path: "docs/tasks/tim-02-ppqn-smf-cross-golden-task.md"
      - type: "file"
        path: "docs/test-specs/tim-02-ppqn-smf-cross-golden.md"
      - type: "file"
        path: "docs/impl/ir/tim-02-ppqn-smf-cross-golden.md"
    modified: []
  implements:
    - "docs/tasks/tim-02-ppqn-smf-cross-golden-task.md"
  implemented-by: []
  depends-on:
    - "docs/tasks/tim-02-ppqn-smf-cross-golden-task.md"
    - "docs/impl/ir/tim-02-smf-format01-import-export.md"
    - "docs/impl/ir/tim-01-configurable-ppqn.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "docs/specs/tim-02-ppqn-smf-cross-golden.md"
    - "docs/designs/tim-02-ppqn-smf-cross-golden-design.md"
    - "docs/test-specs/tim-02-ppqn-smf-cross-golden.md"
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# TIM-02 × TIM-01 PPQN×SMF cross-golden fixture

## Summary

Issue #126 受入行「PPQN 異種の Import / Project Change /
Save / Export / 再 Import」の交差を、headless fixture
`crates/synvelis-project/tests/tim_02_ppqn_smf.rs`（新規
2 テスト）で固定した。Format 1 division-480 の SMF を
project PPQN 960 の project に import し、SMF-backed
source を読む musical clip を anchor した上で
`SetProjectPpqn` を Exact（960→480）と Round
（960→1000、inexact anchor で Exact が原子的 reject）
で適用し、save/load を経て `export_smf(Format1,480)`
の bytes が PPQN 変更・永続化を跨いで golden 一致、
export→reparse→reimport で `SmfDocument` が
`PartialEq` 一致・tempo×2/CC64×3 provenance と note
射影が保持されることを検証した。`src/` の変更は
不要だった — fixture は `ppqn.rs` の source-native
skip 不変条件を実違反なく通過した。

## Context

`docs/specs/tim-02-ppqn-smf-cross-golden.md` /
`docs/designs/tim-02-ppqn-smf-cross-golden-design.md` /
`docs/plans/tim-02-ppqn-smf-cross-golden-plan.md` /
`docs/tasks/tim-02-ppqn-smf-cross-golden-task.md` /
`docs/test-specs/tim-02-ppqn-smf-cross-golden.md` に従う。
TIM-02 merge 済み鎖（`tim_02_smf.rs`）と TIM-01 merge
済み鎖（`tim_01_ppqn.rs`）の交差 — SMF-backed source
への `SetProjectPpqn` — が未検証だったギャップを埋める。
公開 API のみを消費し、`src/` は無変更。

## Implementation

### Changes

- `crates/synvelis-project/tests/tim_02_ppqn_smf.rs`
  （新規・約 470 行）:
  - SMF byte builder: `vlq`/`ev`/`meta`/`tempo`/
    `timesig`/`trackname`/`eot`/`note_on`/`note_off`/
    `cc`/`mtrk`/`smf_file`（`tim_02_smf.rs` と同一
    wire 語彙の最小集合を file-local に複製）。
  - `format1_division480()`: Format 1・division 480 —
    conductor track に tempo 500000@0 + 400000@960
    （phrase 途中変更）・time signature・track name、
    ch0 track に CC64 127→64 sustain 変化 + note
    [240,720)、ch1 track に CC64 64 + note [120,480)。
    project 既定 PPQN 960 との「PPQN 異種」が自然に
    成立。
  - helper: `controller`/`apply`/`try_apply`/`rejected`/
    `ppqn_op`/`import_smf`/`musical_clip`/`midi_window`/
    `add_clip`/`export_golden`（Format1@480）/
    `note_tuples`（`(key,velocity,start,len)` sorted
    で `NoteId` 非依存）/`count_events`/
    `assert_provenance_present`（Tempo=2・CC64=3）/
    `assert_export_reimport_golden`（report 全 exact +
    reparse==doc + re-import doc/note tuple 一致）。
  - `ppqn_exact_change_preserves_smf_chain_end_to_end`:
    import@480 → whole-beat clip（anchor 3840・extent
    1920 @960）→ export golden 取得 → `SetProjectPpqn`
    Exact 960→480（anchor→1920・extent→960 に厳密
    再投影、playback window 不変）→ `smf_documents`/
    `midi_sources` が clone と一致 → export bytes が
    golden 一致 → save/load で ppqn/doc/source/clip
    保持 → export@480 bytes が save/load 後も golden
    一致 → reparse==doc・re-import doc/note tuple 一致。
  - `ppqn_round_change_still_preserves_smf_source_
    native_data`: 同 fixture で anchor tick 1（@960 で
    1/960 beat、@1000 で 25/24 tick の inexact）→
    `Exact` が revision・live state 不変のまま原子的
    reject → `Round(Floor)` で 960→1000 適用（anchor
    25/24→1 tick に量子化）→ `smf_documents`/
    `midi_sources`/export@480 bytes は依然不変
    （project PPQN=1000 を export が参照しない追加
    証拠）→ save/load → export golden → re-import
    doc/note tuple 一致。

### Decisions During Implementation

- **`src/` 無変更で完結**: `reproject_ppqn_with` の
  site 列挙（tempo/signature event・musical clip
  anchor/extent・comp range）に `midi_sources`/
  `smf_documents` が含まれず、`export_smf` が
  `project.ppqn` を参照しないことをコード確認の上、
  fixture は不変条件を実違反なく通過 — 変更許容条件
  （実不変違反の露呈）に該当しなかったため `src/`
  は無変更。
- **golden は bytes 3 時点一致**: 「export が source-
  native slice のみに依存する」を、PPQN 変更前・
  変更後・save/load 後の `export_smf(Format1,480)`
  bytes 一致で固定 — wire 語彙自体の正しさは
  `tim_02_smf.rs` の parse/export golden が担保する
  ため、本 fixture は同一 exporter の再現性のみを
  主張する。
- **note 射影は内容 tuple 比較**: `NoteId` は
  `CounterIdGenerator` が draw する実装詳細であり、
  re-import の等価性は `(key, velocity, start, len)`
  の sorted multiset で主張（`NoteId` 非依存）。
- **fixture は tempo×2 + CC64×3**: conductor の
  phrase 途中 tempo 変更（500000→400000@960）と
  ch0/ch1 の channel-aware CC64 sustain 変化で、
  provenance 保持を event kind 集計で observable に
  した。

## Related Experiments

### Adopted

<!-- なし -->

### Rejected

<!-- なし -->

## Validation

- `cargo test -p synvelis-project --test
  tim_02_ppqn_smf` — 2 passed（補助証拠。canonical
  は `mise run test`）
- `cargo clippy -p synvelis-project --all-targets --
  -D warnings` — clean
- `mise run fmt-check` — pass
- `mise run check` — pass（`cargo check --workspace
  --all-targets`）
- `mise run lint` — pass（`cargo clippy --workspace
  --all-targets -- -D warnings`）
- `mise run test` — pass（`cargo test --workspace
  --locked`、全 crate green。`tim_02_ppqn_smf` 2/2
  含む）
- `mise run test-rt` — pass
- `mise run check-app-native` — pass
- `mise run lint-app-native` — pass
- `mise run test-msrv` — pass
- `mise run build` — pass
- OS/toolchain: Windows（worktree
  `sv126-tim02-ppqn-smf`）、rust ツールチェーンは
  mise 管理（pin 1.99.0 / MSRV 1.89.0）
- NotRun: 実 MIDI backend・OS 実機・AudioLens f64・
  SMPTE/Format2/MIDI2・player/app/gateway 層の
  PPQN×SMF 結合（本 slice 対象外・別 Worker 占有）

## Risks

- 並行 Worker（sv103/sv115/sv107-s8/sv107-s10/sv102/
  sv101）が workspace test lane を共有 — mbx cache
  の compile contention で `mise run test` の wall
  time が伸びたが、結果は全 green。
- `src/` 無変更のため実不変違反は検出されなかった
  — もし将来 `reproject_ppqn_with` の site 列挙や
  `export_smf` の入力面が変われば本 fixture が
  検出する設計。

## Follow-ups

- player/app/gateway 層の PPQN×SMF 結合
  （`synvelis.midi.smf.import` lowering・
  `syn.project.import_smf`/`export_smf`・clip→MIDI
  配線）— 別 slice・他 Worker 占有。
- 実機 MIDI timing・AudioLens f64・SMPTE/Format2/
  MIDI2 — NotRun/Unsupported 委譲（#253/#294/#129/
  #130）。
- managed docs index（`docs/*/README.md`）の再生成
  — coordinator 統合作業（本 slice の編集範囲外）。
