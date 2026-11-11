---
id: "PLAN-126tim02playerMidiRt01"
type: "plan"
status: "approved"
title: "TIM-02 player MIDI RT/Offline 結合 — 実装 plan"
created: "2026-11-10"
updated: "2026-11-11"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/126"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/designs/tim-02-player-midi-rt-offline.md"
  implemented-by: []
  depends-on:
    - "docs/designs/tim-02-player-midi-rt-offline.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/tim-02-player-midi-rt-fixtures.md"
  references: []
  defers: []
  deferred-by: []
---

# TIM-02 player MIDI RT/Offline 結合 — 実装 plan

## File Map

| ファイル | 変更 |
| --- | --- |
| `crates/synvelis-player/src/midi.rs` | 新規 — `compile_midi_events` + `MidiCompileError` + channel 解決 |
| `crates/synvelis-player/src/dispatch.rs` | `ParamDispatch::dispatch_midi` 追加 |
| `crates/synvelis-player/src/driver.rs` | `set_loop`/`clear_loop`/`panic`/`resume_attached_transport` 追加 |
| `crates/synvelis-player/src/lib.rs` | `pub mod midi` + export 追加 |
| `crates/synvelis-player/tests/tim02_midi_playback.rs` | 新規 — RT/Offline 結合 fixture |
| `crates/synvelis-player/tests/tim02_midi_rt.rs` | 新規 — `rt_` no-alloc fixture |

## Dependency Graph

```text
midi.rs (pure lowering)
  └─ dispatch.rs::dispatch_midi (plan install)
       └─ driver.rs transport methods (loop/panic/resume)
            └─ tests/tim02_*.rs (driver + hosted plugin fixture)
```

## Task Breakdown

1. `src/midi.rs`: action 写像・channel 解決・`AllNotesOff` 展開。
2. `dispatch.rs`: `dispatch_midi`（`dispatch()` の install 経路を
   Play 無しで共有）。
3. `driver.rs`: transport command 転送 4 method。
4. `lib.rs`: module 公開。
5. tests: SMF fixture → project → `compile_midi_comp` →
   `compile_midi_events` → `dispatch_midi` → driver 駆動で
   event stream を検証。offline parity は pc_b.rs の
   `VerifiedOfflineNode` pattern を踏襲。

## Risk Register

| リスク | 対応 |
| --- | --- |
| note channel zip の前提崩壊 | 長さ不一致 → 全 note fallback channel |
| `AllNotesOff` が plan 末尾でない手作り plan | note channel 集合を全走査で収集してから展開（二段構成） |
| panic 後の resume で position 乖離 | `resume_attached_transport` が set_clock+Seek+Play で再 anchor |
| `install_plan` 世代衝突 | `next_generation` を `dispatch()` と共有し一意採番 |

## Verification Matrix

| 検証 | 手段 |
| --- | --- |
| event 順序・frame 位置・channel | `tim02_midi_playback.rs` 配送 assert |
| seek release+chase・hanging 0 | 同上 seek test（ledger feed ベースの発音判定） |
| loop wrap per-occurrence | 同上 loop test |
| panic（Stop）発音残り 0 | 同上 panic test |
| RT/Offline parity | 同上 offline render test |
| RT no-alloc | `tim02_midi_rt.rs` `StatsAlloc` window |
| canonical 検証 | `mise run fmt-check/check/lint/test/test-rt/test-msrv` |
