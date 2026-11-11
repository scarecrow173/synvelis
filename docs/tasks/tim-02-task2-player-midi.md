---
id: "TASK-126tim02playerMidiRt01"
type: "task"
status: "done"
title: "TIM-02 Task2 player MIDI RT/Offline 結合 slice"
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
    - "docs/plans/tim-02-player-midi-rt-offline.md"
  implemented-by:
    - "docs/impl/ir/tim-02-player-midi-rt-offline.md"
  depends-on:
    - "docs/plans/tim-02-player-midi-rt-offline.md"
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

# TIM-02 Task2 player MIDI RT/Offline 結合 slice

## Work

spec `tim-02-player-midi-rt-offline` /
design `tim-02-player-midi-rt-offline` /
plan `tim-02-player-midi-rt-offline` /
test-spec `tim-02-player-midi-rt-fixtures` に従い、
`synvelis-player` に MIDI event binding/playback の
RT/Offline 結合を実装する。

- `src/midi.rs`: `MidiPlaybackPlan` → engine frame 位置の
  `MidiEvent` lowering（channel 解決・`AllNotesOff` → CC123）。
- `ParamDispatch::dispatch_midi`: MIDI event 列の
  `TransportPlan<EventKind>` install。
- `HeadlessDriver`: `set_loop`/`clear_loop`/`panic`/
  `resume_attached_transport`。
- `tests/tim02_midi_playback.rs` + `tests/tim02_midi_rt.rs`。

## Acceptance mapping

- Loop/Seek/Stop/Panic/Chase・発音残り 0 → driver transport
  method + upstream discontinuity（release_all/chase）で実現、
  fixture で event stream を検証。
- RT/Offline → playback drive と offline `render()` の parity
  fixture。実 MIDI Backend OS 別・AudioLens は NotRun。

## Done When

- [x] test-spec `tim-02-player-midi-rt-fixtures` の Rule/Example
  が全て実測 Pass（`tim02_midi_playback` 10 test・
  `tim02_midi_rt` 1 test 全て Pass）
- [x] `crates/` 配下のコードは PR `#270`（+ TIM-03 PR `#308`
  追従）で main に merge 済み。本 slice はコード変更なしの
  canonical 検証 + doc chain 閉鎖
- [x] impl record
  `docs/impl/ir/tim-02-player-midi-rt-offline.md` に
  pass/fail/NotRun/Unsupported と実行した検証コマンド・
  exit status を正確に記録（検証 slice record:
  `docs/impl/ir/tim-02-player-midi-verification-slice.md`）

## Verification

最終 canonical 証拠: `git merge origin/main`（`ff33ffb`・
RCD-03/04 込み）後の merged tree で全 lane 再実行し全 Pass
（exit 0）。以下は初回 lane（`e46bf1f` + docs-only 差分）の
記録 — merged lane でも同じく全 Pass:

- [x] `mise run fmt-check` — Pass（exit 0）
- [x] `mise run check` — Pass（exit 0）
- [x] `mise run lint` — Pass（exit 0・warning 0）
- [x] `mise run test` — Pass（再実行で exit 0・workspace 全
  test binary ok。`tim02_midi_playback` 10/10・
  `tim02_midi_rt` 1/1 を含む。初回 run は exit 9 —
  `tim03_session_wiring` binary が alloc 失敗後 abort
  （TIM-02 無関係の transient 環境要因。単独再実行 10/10
  Pass・全量再実行 exit 0 で非再現を確認。詳細は impl
  record の Validation 節）
- [x] `mise run test-rt` — Pass（exit 0）
- [x] `mise run test-msrv` — Pass（exit 0・rust 1.89.0）
- [x] `mise run build` — Pass（再実行で exit 0。初回 run は
  `rustc` LLVM out of memory で exit 101 — test 初回と同じ
  transient 環境要因、再実行で非再現）

- [ ] 実 MIDI Backend OS 別・実 Device/Plugin・SMPTE/MIDI2・
  AudioLens f64 — NotRun / Unsupported（test-spec 宣言どおり
  外部 Gate。Pass 扱いしない）
