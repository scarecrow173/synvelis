---
id: "IMPL-034VCfdDgChNzOrdphfKyJ"
type: "impl"
status: "completed"
title: "TIM-02 player MIDI RT/Offline 検証 slice（既 merge コード canonical 検証 + doc chain 閉鎖）"
created: "2026-11-11"
updated: "2026-11-11"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/126"
  changes:
    added:
      - type: "file"
        path: "docs/impl/ir/tim-02-player-midi-rt-offline.md"
      - type: "file"
        path: "docs/impl/ir/tim-02-player-midi-verification-slice.md"
    modified:
      - type: "file"
        path: "docs/tasks/tim-02-task2-player-midi.md"
      - type: "file"
        path: "docs/specs/tim-02-player-midi-rt-offline.md"
      - type: "file"
        path: "docs/test-specs/tim-02-player-midi-rt-fixtures.md"
      - type: "file"
        path: "docs/plans/tim-02-player-midi-rt-offline.md"
      - type: "file"
        path: "docs/specs/README.md"
      - type: "file"
        path: "docs/plans/README.md"
      - type: "file"
        path: "docs/tasks/README.md"
      - type: "file"
        path: "docs/test-specs/README.md"
      - type: "file"
        path: "docs/impl/ir/README.md"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/tasks/tim-02-task2-player-midi.md"
  implemented-by: []
  depends-on:
    - "docs/tasks/tim-02-task2-player-midi.md"
    - "docs/impl/ir/tim-02-player-midi-rt-offline.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/ir/tim-02-player-midi-rt-offline.md"
    - "docs/impl/ir/tim-02-smf-format01-import-export.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies:
    - "docs/test-specs/tim-02-player-midi-rt-fixtures.md"
  verified-by: []
  references:
    - "crates/synvelis-player/src/midi.rs"
    - "crates/synvelis-player/src/dispatch.rs"
    - "crates/synvelis-player/src/driver.rs"
    - "crates/synvelis-player/src/process_boundary.rs"
    - "crates/synvelis-player/tests/tim02_midi_playback.rs"
    - "crates/synvelis-player/tests/tim02_midi_rt.rs"
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# TIM-02 player MIDI RT/Offline 検証 slice（既 merge コード canonical 検証 + doc chain 閉鎖）

## Summary

Issue #126 Task2（player MIDI RT/Offline 結合）はコードが PR
`#270`（+ TIM-03 追従 PR `#308`）で既に main へ merge 済みだが、
doc chain が未閉鎖のまま残っていた（task `in-progress`・
spec/test-spec `proposed`・impl record 未作成・plan の stale
filename `tim02_midi_transport.rs`）。本 slice は新規実装では
なく、**既 merge コードに対する canonical mise lane の実走
記録と doc chain の閉鎖**を行う検証 slice。`crates/` 配下は
一切変更していない（検証が失敗した場合は修正せず証拠を記録
して報告する契約）。

## Context

- task `docs/tasks/tim-02-task2-player-midi.md`
- spec `docs/specs/tim-02-player-midi-rt-offline.md`
- design `docs/designs/tim-02-player-midi-rt-offline.md`
- plan `docs/plans/tim-02-player-midi-rt-offline.md`
- test-spec `docs/test-specs/tim-02-player-midi-rt-fixtures.md`
- implementation record
  `docs/impl/ir/tim-02-player-midi-rt-offline.md`

検証対象は main（`e46bf1f`）上の
`crates/synvelis-player/src/midi.rs`（`compile_midi_events`）・
`ParamDispatch::dispatch_midi`・
`ProcessBoundarySession::dispatch_midi`・`HeadlessDriver` の
`set_loop`/`clear_loop`/`panic`/`resume_attached_transport`・
`tests/tim02_midi_playback.rs`（10 test）・
`tests/tim02_midi_rt.rs`（`rt_midi_delivery_hot_path_alloc_free`）。

## Implementation

### Changes

- `docs/plans/tim-02-player-midi-rt-offline.md`: stale filename
  `tim02_midi_transport.rs` → 実在名 `tim02_midi_playback.rs`
  に修正（file map・verification matrix の 2 箇所）。
- `docs/tasks/tim-02-task2-player-midi.md`: Work 節の stale
  filename を同様に修正し、status を実測結果に整合させる。
- `docs/specs/tim-02-player-midi-rt-offline.md` /
  `docs/test-specs/tim-02-player-midi-rt-fixtures.md`: status を
  実測結果に整合させる。
- `docs/impl/ir/tim-02-player-midi-rt-offline.md`（新規）:
  Task2 の実装内容・canonical 検証結果・NotRun/Unsupported を
  記録する implementation record。
- `docs/*/README.md`: 管理 index の再生成。

### Decisions During Implementation

- コード変更は行わない前提の検証 slice のため、canonical
  検証は `mise run` タスクのみを正規証拠とし、cargo 直接実行
  は補助証拠に留める方針を採用。
- spec/test-spec の status は Task1（
  `tim-02-smf-format01-*` が `proposed` のまま settled）の先例
  ではなく、本 slice の受入条件「status を実態に整合させる」
  に従い、全 lane Pass なら `approved`、task は `done` に
  引き上げる。失敗があれば status は上げず理由を記録する。

## Related Experiments

### Adopted

<!-- 実験 log なし（検証実行 + doc 更新の機械的 slice）。 -->

### Rejected

<!-- なし -->

## Validation

canonical 検証（`mise.toml` タスクの実走。`origin/main`
`e46bf1f` + 本 slice の docs-only 差分・Windows worktree・
rust 1.99.0 / MSRV 1.89.0）:

- `mise run fmt-check` — Pass（exit 0）
- `mise run check` — Pass（exit 0）
- `mise run lint` — Pass（exit 0・warning 0）
- `mise run test` — Pass（再実行で exit 0。`tim02_midi_playback`
  10/10・`tim02_midi_rt` 1/1 を含む全 binary ok。**初回 run
  exit 9**: `tim03_session_wiring` binary が `memory
  allocation of 33558588 bytes failed` → `0xc0000409
  (STATUS_STACK_BUFFER_OVERRUN)` で abort。TIM-02 無関係の
  transient 要因であり、同 binary 単独再実行（補助証拠:
  `mise exec -- cargo test -p synvelis-player --test
  tim03_session_wiring --locked`、10/10 Pass）+ `mise run
  test` 全量再実行（exit 0）で非再現を確認）
- `mise run test-rt` — Pass（exit 0）
- `mise run test-msrv` — Pass（exit 0・rust 1.89.0）
- `mise run build` — Pass（exit 0）

doc-maintenance / audit（本 slice 作業の検証）:

<!-- DOC_AUDIT -->

NotRun / Unsupported（本 slice の対象外。Pass 扱いしない）:

- 実 MIDI Backend OS 別・実 Device/Plugin — Ruviform `#253`
  の外部 gate
- SMPTE/Format2/MIDI2/MPE — Unsupported（project 層で明示拒否）
- AudioLens Native f64 非回帰 — `#294` 別 Gate
- panic-and-continue・seek 時 controller chase — upstream
  deferred contract
- MIDI comp selection 永続化・MIDI clip graph lowering・
  `MidiNote.channel` — 別 slice / project model 変更

## Risks

- canonical lane は全 workspace を対象とするため、TIM-02 と
  無関係な既存 flaky（`synvelis-library` webdav の Windows
  socket error 10053 が Task1 時点で報告済み）が混入し得る。
  その場合は TIM-02 対象 test の個別結果と失敗原因を区別して
  記録する。

## Follow-ups

- Issue #126 残件は implementation record の Follow-ups と同一
  （実 MIDI Backend/Plugin・SMPTE/MIDI2・AudioLens f64・
  MIDI comp selection 永続化等）。Issue は open 維持前提。
