---
id: "IMPL-034VCf2QmeMPfbOyKxLlVw"
type: "impl"
status: "completed"
title: "TIM-02 player MIDI RT/Offline 結合 — 実装記録"
created: "2026-11-11"
updated: "2026-11-11"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/126"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-player/src/midi.rs"
      - type: "file"
        path: "crates/synvelis-player/tests/tim02_midi_playback.rs"
      - type: "file"
        path: "crates/synvelis-player/tests/tim02_midi_rt.rs"
      - type: "file"
        path: "docs/specs/tim-02-player-midi-rt-offline.md"
      - type: "file"
        path: "docs/designs/tim-02-player-midi-rt-offline.md"
      - type: "file"
        path: "docs/plans/tim-02-player-midi-rt-offline.md"
      - type: "file"
        path: "docs/test-specs/tim-02-player-midi-rt-fixtures.md"
      - type: "file"
        path: "docs/tasks/tim-02-task2-player-midi.md"
      - type: "file"
        path: "docs/impl/ir/tim-02-player-midi-rt-offline.md"
    modified:
      - type: "file"
        path: "crates/synvelis-player/src/dispatch.rs"
      - type: "file"
        path: "crates/synvelis-player/src/driver.rs"
      - type: "file"
        path: "crates/synvelis-player/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-player/src/process_boundary.rs"
      - type: "file"
        path: "docs/specs/README.md"
      - type: "file"
        path: "docs/designs/README.md"
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
    - "docs/plans/tim-02-player-midi-rt-offline.md"
    - "docs/specs/tim-02-player-midi-rt-offline.md"
  implemented-by: []
  depends-on:
    - "docs/tasks/tim-02-task2-player-midi.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/ir/tim-02-smf-format01-import-export.md"
    - "docs/impl/ir/tim-02-player-midi-verification-slice.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/tim-02-player-midi-rt-offline.md"
    - "docs/designs/tim-02-player-midi-rt-offline.md"
    - "docs/plans/tim-02-player-midi-rt-offline.md"
  derived-by: []
  verifies:
    - "docs/test-specs/tim-02-player-midi-rt-fixtures.md"
  verified-by:
    - "docs/test-specs/tim-02-player-midi-rt-fixtures.md"
  references: []
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# TIM-02 player MIDI RT/Offline 結合 — 実装記録

## Summary

Issue #126（TIM-02）のうち `synvelis-player` が担う RT/Offline
結合 slice。project `compile_midi_comp` が生成する
`MidiPlaybackPlan`（MIDI Take Phrase）を engine frame 位置の
`MidiEvent` 列へ lowering し（`src/midi.rs` の
`compile_midi_events`）、`ParamDispatch::dispatch_midi` で
generation-tagged `TransportPlan<EventKind>` として attached
plugin node に install する。`HeadlessDriver` に
`set_loop`/`clear_loop`/`panic`/`resume_attached_transport` を
追加し、Loop/Seek/Stop/Panic/Chase を upstream
`TransportScheduler` の discontinuity（release_all + plan prefix
chase）で実現した。配送・ledger・discontinuity の実装は
Ruviform が所有し、本 slice は plan → wire event の lowering と
driver/dispatch 結合のみを行う。

本記録は **既に main へ merge 済みのコード**（PR `#270` で
`midi.rs`/`dispatch_midi`/driver transport 4 method・両 test
file が追加され、TIM-03 PR `#308` で
`ProcessBoundarySession::dispatch_midi`・transport method の
`BoundarySessionError` 化が追従）に対する implementation
record であり、検証 slice（本 slice）で canonical mise lane を
再実行した実測を Validation に記録する。

## Context

- spec `docs/specs/tim-02-player-midi-rt-offline.md`
- design `docs/designs/tim-02-player-midi-rt-offline.md`
- plan `docs/plans/tim-02-player-midi-rt-offline.md`
- test-spec `docs/test-specs/tim-02-player-midi-rt-fixtures.md`
- task `docs/tasks/tim-02-task2-player-midi.md`
- verification slice record
  `docs/impl/ir/tim-02-player-midi-verification-slice.md`

Issue #126 の player 側受入行（Loop/Seek/Stop/Panic/Chase・
発音残り 0・RT/Offline headless 部分）を対象とする。実 MIDI
Backend OS 別・実 Device/Plugin（Ruviform `#253` の責務）・
SMPTE/Format2/MIDI2/MPE・AudioLens Native f64（`#294` 別
Gate）・MIDI clip graph lowering・seek 時 controller chase・
`MidiNote.channel` 追加は spec/design の defers どおり対象外。

## Implementation

### Changes

- `crates/synvelis-player/src/midi.rs`（新規）:
  `compile_midi_events(project, plan, mapping, frame_base,
  fallback_channel) -> Result<Vec<(i64, MidiEvent)>,
  MidiCompileError>`。2 pass 構成: pass 1 で `ChannelResolver`
  により note event の channel を解決し発音 channel 集合を収集、
  pass 2 で plan 全順序を保持して wire event を emit
  （`at` 非減少・同 frame は plan 順）。時刻写像は
  `mapping.tick_to_media` の exact rational（checked error
  のみ、unchecked `Tick` 演算は不使用）。action 写像は
  `NoteOn`/`NoteOff`（release velocity 0）・`ControllerChase`
  （`Cc` → `ControlChange` `from_u7`、`PitchBend` →
  `bend_from_u14`）・`AllNotesOff` → 発音した channel 集合への
  CC123（`ControlChange{controller:123, value:0}`、≤16 件
  bounded）。channel provenance は `project.smf_documents[
  source_id]` の `paired_notes()` と `source.notes`（NoteId 順）
  の zip + `(key,velocity,start,len)` content 一致で復元し、
  document 非所持・drift は `fallback_channel` へ落とす。
  wire domain 違反は `MidiCompileError`
  （`Project`/`Wire`/`Domain`、`#[non_exhaustive]`）で拒否し、
  `from_u7`/`bend_from_u14` の wrap/clamp に頼る coerce は行わ
  ない。
- `crates/synvelis-player/src/dispatch.rs`:
  `ParamDispatch::dispatch_midi(&[(i64, MidiEvent)])` を追加。
  `dispatch()` と同じ install 経路（`plan_builder` で verified
  `frame_base` の builder mint・`next_generation` 採番・
  `install_plan(due=0)`・`mark_plan_replaced`・`record_plan`・
  `last_plan` 更新）で `(position, source_sequence=index)` の
  event を staging するが `Play` は要求しない — transport の
  Play/Seek anchor は driver が所有する既存契約を維持。
- `crates/synvelis-player/src/driver.rs`: `HeadlessDriver` に
  transport 転送 4 method を追加。
  `set_loop(start,end)`/`clear_loop` は attached node ごとの
  `frame_base` で `MediaRange` を構成し
  `TransportCommand::SetLoop`/`ClearLoop` を転送（loop wrap は
  `LoopOccurrence` bump → discontinuity → release + plan prefix
  chase で occurrence ごとに 1 回配送）。`panic` は
  `TransportCommand::Stop` を転送し、status Stopped のため
  release のみ配送・chase 空・plan event 不発 = 発音残り 0。
  `resume_attached_transport` は activation と同じ anchor
  （`set_clock`+`Seek`+`Play`）で position==cursor を回復する
  panic 後の 2 段操作。TIM-03 で `stage_transport`/
  `stage_node_transport`/`anchor_link` 経由の
  `Result<(), BoundarySessionError>` 形へ追従済み。
- `crates/synvelis-player/src/process_boundary.rs`:
  `ProcessBoundarySession::dispatch_midi` — TIM-03 の
  provenance gate（`check_dispatch`）を `dispatch_params` と同じ
  形で MIDI dispatch に適用する薄い wrapper（TIM-03 PR `#308`
  側の追加を本記録の context として記録）。
- `crates/synvelis-player/src/lib.rs`: `pub mod midi` +
  `compile_midi_events`/`MidiCompileError` export。
- `crates/synvelis-player/tests/tim02_midi_playback.rs`（新規・
  10 test）: SMF import → project → `compile_midi_comp` →
  `compile_midi_events` → `dispatch_midi` → real `PluginNode`
  scheduler 経路の headless fixture。lowering 写像・絶対 frame
  配送・native tick→engine frame・同 tick NoteOff→On 順・
  channel-aware chase・loop wrap の release+再発火・seek の
  release+chase（skip 区間不発）・panic 発音残り 0 + resume・
  SMF channel provenance・`RtRuntime` drive vs offline
  `render()` parity を検証。
- `crates/synvelis-player/tests/tim02_midi_rt.rs`（新規・
  1 test）: `stats_alloc` による process-global allocator
  計測で `RtRuntime::process` 経由の MIDI 配送 hot path が
  alloc/dealloc 0 であることを実測する独立 binary（loop wrap
  split・release/chase・plan refire を計測 window 内で駆動）。

### Decisions During Implementation

- `dispatch_midi` は `Play` を要求しない。`Play` は position を
  現在値へ再 anchor するため、`Stop` 後の node で裸の `Play`
  を打つと position が 0 に戻り engine cursor と乖離する。
  transport anchoring は driver（activation・`seek`・
  `resume_attached_transport`）が所有する既存契約に従い、
  dispatch は plan のみを staging する。
- panic = `TransportCommand::Stop`。release-without-retrigger
  を表現できる唯一の upstream primitive であり、transport も
  停止するため panic-and-continue は upstream 未対応として
  defer し、`panic` + `resume_attached_transport` の 2 段操作を
  API として明示した。
- `AllNotesOff` → CC123 は plan 内に実際に発音した channel
  集合（`BTreeSet` 昇順）のみを対象とする。plan oracle が全
  note の off を保証済みのため CC123 は深層防御であり、ledger
  単位の保証は upstream `release_all` が担う。
- note channel は `MidiPlanAction` が channel を持たない既存
  契約のため SMF provenance で復元し、zip が崩れた defensive
  case は誤 channel で鳴らすより `fallback_channel`（binding
  指定・既定 0）が安全側として選択した。
- コードは PR `#270` で merge 済みであり本記録は事後記録。
  TIM-03（PR `#308`）で `BoundarySessionError` 化・
  `ProcessBoundarySession::dispatch_midi` 追加の追従が入って
  いるため、記録は現行 main の形に合わせた。

## Related Experiments

### Adopted

<!-- 実験 log なし（既 merge コードの検証記録 + 設計確定済みの
結合実装）。 -->

### Rejected

<!-- なし -->

## Validation

canonical 検証（`mise.toml` タスク。検証 slice の実走・
Windows worktree・rust 1.99.0 / MSRV 1.89.0）:

最終 lane は `git merge origin/main`（`ff33ffb`、RCD-03/04
込み）後の merged tree で全 lane 再実行し全 Pass。初回 lane
は `origin/main` `e46bf1f` + 本 slice の docs-only 差分で
実行し、そこで観測された transient 失敗（下記）も正直に
記録する。

merged tree（`ff33ffb` + 本 branch）での最終結果:

- `mise run fmt-check` — Pass（exit 0）
- `mise run check` — Pass（exit 0・`cargo check --workspace
  --all-targets`）
- `mise run lint` — Pass（exit 0・`cargo clippy --workspace
  --all-targets -- -D warnings`・warning 0）
- `mise run test` — Pass（exit 0・`cargo test --workspace
  --locked`。対象 test: `tim02_midi_playback` 10/10 ok・
  `tim02_midi_rt` 1/1 ok を含む全 binary ok・failure 0）
- `mise run test-rt` — Pass（exit 0・`cargo test --workspace
  --locked rt_`。`rt_midi_delivery_hot_path_alloc_free`・
  `tim02_rt_offline_parity` も含め全 ok）
- `mise run test-msrv` — Pass（exit 0・`mise exec rust@1.89.0
  -- cargo test --workspace --locked`）
- `mise run build` — Pass（exit 0・`cargo build --workspace
  --locked`）

初回 lane（`e46bf1f` + docs-only 差分）で観測された transient
環境要因（merged lane では再現せず）:

- `mise run test` 初回 run は exit 9: `tim03_session_wiring`
  binary が `memory allocation of 33558588 bytes failed` を
  出力後 `exit code 0xc0000409 (STATUS_STACK_BUFFER_OVERRUN)`
  で abort。TIM-02 と無関係な TIM-03 binary の transient 環境
  要因で、同 binary の単独再実行（補助証拠: `mise exec --
  cargo test -p synvelis-player --test tim03_session_wiring
  --locked`、10/10 Pass）および `mise run test` 全量再実行
  （exit 0）で再現しないことを確認。
- `mise run build` 初回 run は exit 101: `rustc` が
  `synvelis-workspace` compile 中に `rustc-LLVM ERROR:
  out of memory` で `0xc0000409` abort。同じく memory 圧力の
  transient 要因で、単独再実行で exit 0・非再現を確認。

NotRun / Unsupported（test-spec 宣言どおり、Pass 扱いしない）:

- 実 MIDI Backend OS 別・実 Device/Plugin — NotRun
  （Ruviform `#253` の外部 gate。headless fixture の成功を
  実機成功にしない）
- SMPTE division・Format2・MIDI 2.0/MPE — Unsupported
  （project 層 `SmfError::Unsupported` で明示拒否）
- AudioLens Native f64 非回帰 — NotRun（`#294` 別 Gate）
- panic-and-continue — upstream に専用 command なし、
  `Stop` 経由の 2 段操作のみ
- seek 時 controller chase — upstream `chase_span_events` の
  deferred contract（本 slice でも複製しない）

## Risks

- note channel の index 対応は `paired_notes()` と NoteId 順の
  一致に依存し、仮に崩れた場合は `(key,velocity,start,len)`
  不一致を検出して当該 note を `fallback_channel` に落とす。
  誤 channel で鳴らさない安全側だが、provenance が取れない
  source（手組み・Detach copy）は全 note が declared channel
  に寄る。
- node の live plan は単一で `install_plan` が wholesale
  replace するため、同一 node に param plan と MIDI plan を
  逐次 install すると後勝ち。instrument node = MIDI plan、
  processor node = param plan の住み分けが前提。
- `panic` は transport を停止するため、`panic()` 単独では
  transport 再生は再開しない。呼び出し側が
  `resume_attached_transport` を明示する 2 段操作であり、
  resume 忘れは Stopped のまま残る。

## Follow-ups

- Issue #126 残件: 実 MIDI Backend OS 別・実 Device/Plugin
  （Ruviform `#253`）・SMPTE/MIDI2（`#129`/`#130` future）・
  AudioLens f64（`#294`）・MIDI clip graph lowering・
  `MidiNote.channel` 追加・MIDI comp selection 永続化・
  panic-and-continue / seek 時 controller chase の upstream
  contract。Issue は残件の外部 Gate があるため open 維持。
