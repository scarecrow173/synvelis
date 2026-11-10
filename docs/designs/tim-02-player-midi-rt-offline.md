---
id: "DESIGN-126tim02playerMidiRt01"
type: "design"
status: "approved"
title: "TIM-02 player MIDI RT/Offline 結合 — crate 内完結設計"
created: "2026-11-10"
updated: "2026-11-10"
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
    - "docs/specs/tim-02-player-midi-rt-offline.md"
  implemented-by: []
  depends-on:
    - "docs/specs/tim-02-player-midi-rt-offline.md"
    - "docs/designs/tim-02-smf-format01-core.md"
    - "docs/designs/rcd-04-comp-transition-midi-phrase-fixture.md"
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
  references: []
  defers:
    - "実 MIDI Backend OS 別・実 Device/Plugin 検証（Ruviform #253 の責務）"
    - "Panic-and-continue（transport 継続しつつ発音のみ停止）— upstream に専用 command がなく、Stopped 経由の Stop のみが release-without-retrigger を表現できる"
    - "Seek 時の controller state chase — upstream `chase_span_events` の deferred contract"
    - "MidiNote への channel 追加（synvelis-project 側 model 変更・別 slice）"
  deferred-by: []
---

# TIM-02 player MIDI RT/Offline 結合 — crate 内完結設計

## Context

TIM-02 core slice（PR #225 系）が `synvelis-project` に
`MidiPlaybackPlan` oracle を確立した: `compile_midi_comp` は
phrase 採用 note の NoteOn/NoteOff・region 境界の
`ControllerChase`・終端 `AllNotesOff` を project `MusicalTick`
で全順序付きに出力し、hanging note を構造的に排除する。
本 slice はその plan を **player が実際に音にする**結合部。

既存事実（調査済み）:

- `HeadlessDriver` + `AttachedNode { node: PluginNode,
  dispatch: ParamDispatch }` が plugin node の transport を既に
  所有する: activation で `set_clock`+`Seek(cursor)`+`Play`、
  `seek()` で再 anchor。plan event の position は engine frame
  == node position_ticks（`frame_base = TimeBase::sample_rate`）。
- `PluginNode` 内蔵 `TransportScheduler` が transport command
  （Play/Pause/Stop/Seek/SetLoop/ClearLoop）と plan install を
  RT callback head で適用し、discontinuity
  （epoch/tg/pg/occurrence 変化・位置 jump）で `NoteLedger::
  release_all` + `chase_span_events`（plan prefix の発音中 note
  を offset-0 で再発火）を行う。`TransportCommand::Stop` は
  status=Stopped かつ chase が空になるため、release のみが
  配送され **panic（発音停止・再発火なし）を表現できる唯一の
  upstream primitive**。
- `ScheduledEvent.transport_generation: None`（absolute）は
  seek/Play の tg bump 後も position window で再評価される
  ため、seek 後退で event が自然に再配送される。
- `ParamDispatch` は `PluginNodeControl`・`frame_base`・
  `next_generation` を所有し、`install_plan`+receipt で plan
  世代を管理する。`record_plan` は `EventKind::ParamValue`
  のみを ledger に記録するため MIDI event を混ぜても安全。
- `MidiPlanAction::NoteOn/NoteOff` は channel を持たない
  （`MidiNote` に channel がない既存契約）。channel provenance
  は `SmfDocument` が所有する。

## Scope

### In Scope

- `crates/synvelis-player/src/midi.rs`（新規）: project
  `MidiPlaybackPlan` → engine frame 位置の `MidiEvent` 列への
  lowering（channel 解決・action → `MidiKind` 写像・
  `AllNotesOff` → channel-aware CC123 panic）。
- `ParamDispatch::dispatch_midi`: `(engine frame, MidiEvent)`
  列を generation-tagged `TransportPlan<EventKind>` として
  node に install する non-RT orchestration。
- `HeadlessDriver`: `set_loop`/`clear_loop`/`panic`/
  `resume_attached_transport` — attached node の transport
  command 転送（driver が transport を所有する既存契約の延長）。
- `crates/synvelis-player/tests/tim02_*.rs`: RT/Offline 結合
  fixture（playback・seek・loop・panic・chase・hanging voice 0・
  RT/Offline parity・rt no-alloc）。

### Out of Scope

- spec の Out of Scope と同一。加えて: `stop()`/`run()` の既存
  semantics 変更（`stop()` は driver state のみを扱い node へは
  通知しない — panic は明示 `panic()` で行う）、
  `observe_attached_block` の fabricated span の loop/occurrence
  正確化（param ledger projection のみが消費し MIDI には無関係）。

## Components and Boundaries

### `midi.rs` — plan → wire event lowering

```rust
pub fn compile_midi_events(
    project: &Project,
    plan: &MidiPlaybackPlan,
    mapping: &TickTimeMapping,
    frame_base: TimeBase,
    fallback_channel: u8,
) -> Result<Vec<(i64, MidiEvent)>, MidiCompileError>
```

- 時刻写像: `mapping.tick_to_media(event.at,
  frame_base.numerator(), frame_base.denominator()).value()`
  で project `MusicalTick` → engine frame（`lowering.rs` の
  `range_span`/`point_frame` と同一 convention、checked error
  のみ・unchecked `Tick` 演算は使わない）。`frame_base` は
  dispatch の verified attach-time base をそのまま渡す契約で、
  出力 `i64` は `dispatch_midi` が解釈する frame base の tick
  数と一致する。
- action 写像（1 plan event → 1 または複数 `MidiEvent`、
  `group = MidiGroup::ZERO`）:
  - `NoteOn{key,velocity}` → `MidiKind::NoteOn`、
    `velocity = MidiValue::from_u7(velocity)`。
  - `NoteOff{key}` → `MidiKind::NoteOff`、release velocity は
    plan が model しないため `0`。
  - `ControllerChase{channel,Cc(n),value}` →
    `MidiKind::ControlChange`、`value = MidiValue::from_u7`。
  - `ControllerChase{channel,PitchBend,value}` →
    `MidiKind::PitchBend`、`value = MidiValue::bend_from_u14`。
  - `AllNotesOff` → plan 内に出現した note channel 集合の各
    channel へ `ControlChange{controller: 123, value: 0}`
    （MIDI All Notes Off）。note channel のみを対象とし、
    発音しない controller channel には投げない。件数は
    ≤16 で bounded。
- note channel 解決（fallback つき）:
  `project.smf_documents[source_id]` が存在すれば
  `doc.paired_notes()` と `source.notes.values()`（NoteId 順
  = import 時の spec 順と一致）を index 対応させて
  `NoteId → channel` を構築する。`MidiSource` の note は
  SMF import 由来では immutable guard により改変不能なため
  この対応は安定。document 非所持（手作成 source・detach
  copy）・長さ不一致の defensive case は `fallback_channel`
  （binding 指定、既定 0）。
- 出力は plan の全順序を保持（`(at, action rank, …)` ソート済み
  → frame 非減少・同 frame は plan 順）。`dispatch_midi` が
  `(position, source_sequence=index)` で builder に渡すため
  同時刻 NoteOff→On の順序は upstream merge の note-off
  priority と合わせて保たれる。
- `MidiCompileError`: `Project(synvelis_project::Error)` /
  `Wire(MidiRangeError)` の typed error。`#[non_exhaustive]`。

### `ParamDispatch::dispatch_midi`

```rust
pub fn dispatch_midi(
    &mut self,
    events: &[(i64, MidiEvent)],
) -> Result<Arc<TransportPlan<EventKind>>, DispatchError>
```

- `dispatch()` と同じ install 経路（`plan_builder` で
  verified frame_base の builder を mint、`next_generation`
  採番、`install_plan(due=0)`、`mark_plan_replaced`、
  `record_plan`（ParamValue のみ記録 → MIDI plan では 0 件）、
  `last_plan` 更新）だが **`Play` は要求しない**。
  理由: `Play` は position を現在値へ再 anchor するため、
  `Stop` 後の node で裸の `Play` を打つと position が 0 へ
  戻り engine cursor と乖離する。transport の Play/Seek
  anchor は driver が所有する（`anchor_attached_transport`・
  `seek`）既存契約に従い、dispatch は plan のみを staging
  する。standalone 利用では caller が `control().request(
  Play)` で transport を開始する。
- node の live plan は単一（upstream `install_plan` が
  wholesale replace）。param plan と MIDI plan を同一 node
  に逐次 install すると後勝ち — instrument node = MIDI
  plan、processor node = param plan の住み分けを doc で
  明示する。

### `HeadlessDriver` transport 拡張

- `set_loop(start: u64, end: u64)` / `clear_loop()`: 各
  attached dispatch の `frame_base` で `MediaRange` を構成し
  `TransportCommand::SetLoop`/`ClearLoop` を `due=0` で転送。
  `start < end` は upstream `validate` が `InvalidCommand`
  receipt で拒否する前提を doc に明記。戻り値は
  `Result<(), QueueFull>`。
- `panic()`: 各 attached node に `TransportCommand::Stop` を
  転送。次の processed block で tg bump → discontinuity →
  `release_all` が offset-0 の NoteOff を配送し、status
  Stopped のため chase は空・plan event も発火しない
  = panic。戻り値 `Result<(), QueueFull>`。
- `resume_attached_transport()`: `anchor_attached_transport`
  （`set_clock(epoch,cursor)`+`Seek(cursor)`+`Play`）を公開
  wrapper 化。panic 後の復帰で position==engine frame を
  回復し、tg bump の discontinuity が plan prefix を chase
  して発音中 note を正しく再構成する。
- `stop()`/`run()` は変更しない（既存 semantics 保持）。

### Data and Control Flow

```text
Project { midi_sources, smf_documents, tempo_map }
  │ compile_midi_comp(selection, lanes)          [synvelis-project]
  ▼
MidiPlaybackPlan { events: [(MusicalTick, MidiPlanAction)] }
  │ compile_midi_events(project, plan, mapping, frame_base, fallback_ch)
  ▼
Vec<(i64 engine_frame, MidiEvent)>               [synvelis-player::midi]
  │ ParamDispatch::dispatch_midi → plan_builder → install_plan
  ▼
PluginNodeControl → PluginNode.scheduler (RT)
  │ run_callback: apply controls → spans → merge
  │   evict ++ release ++ chase ++ plan ++ live
  ▼
EventKind::Midi → plugin endpoint (EventList, offset-relative)
```

## Non-Functional Constraints

- RT 経路（`dispatch_midi` 呼出し後の `rt.process`）は
  allocation-free — upstream `PluginNode` の既存契約に依存し、
  `rt_` fixture で `StatsAlloc` 実測する。`compile_midi_events`/
  `dispatch_midi`/driver transport method は non-RT orchestration。
- deterministic: plan order 保存・channel 集合は `BTreeSet`
  昇順・同 frame 順序は `(position, source_sequence)` で安定。
- fail-closed: tick 変換失敗・wire domain 違反・
  `paired_notes` 失敗は typed error。未知 event を実行可能
  MIDI に変換しない（plan が既に typed action のみを持つ）。
- no implicit quantization: `tick_to_media` は exact rational、
  失敗は `Error::Overflow` として顕在化。

## Risks and Trade-offs

- note channel の index 対応は `paired_notes()` と NoteId 順の
  一致に依存 → immutable guard + import の連続採番で保証される
  が、仮に崩れた場合は長さ不一致を検出して全 note を
  `fallback_channel` に落とす defensive 分岐を置く
  （誤 channel で鳴らすより既定 channel の方が安全側）。
- `AllNotesOff` → CC123 は plugin が All Notes Off を解釈する
  前提 — plugin 非依存の絶対保証ではない。plan oracle が全
  note に off を保証済みのため CC123 は深層防御であり、
  ledger 単位の保証は upstream `release_all` が担う。
- panic = Stop は transport も停止する（panic-and-continue は
  upstream 未対応）→ `resume_attached_transport` で復帰する
  2 段操作を API として明示する。
- doc-driven-dev-graph router は main 既存の broken relation
  で blocked（TIM-02 core・RCD-04 と同じ状況）→ chain 文書で
  進行し impl record に記録する。

## Deferred Design Concerns

- panic-and-continue / dedicated upstream Panic command —
  ruviform 側の transport command 拡張が必要 | panic が
  transport を止めない要件が出た時 | Stop 経由の 2 段操作が
  UX に見える | 中
- seek 時の controller chase — upstream `chase_span_events`
  の deferred contract | ruviform#126 系で controller chase が
  実装された時 | seek 後に sustain/pitch bend が stale のまま | 中
- `MidiNote.channel` 追加 — project model 変更 | note-level
  channel が plan に載った時 | provenance zip が不要になる | 低
- MIDI clip playback（`MidiSource` clip → instrument node
  routing）— audio graph に MIDI port 概念がない | MIDI clip
  再生要件が出た時 | Take Phrase のみが再生経路 | 中

## Verification Notes

- [ ] `cargo test -p synvelis-player tim02` で fixture 全緑
- [ ] `cargo test -p synvelis-player rt_` で no-alloc 実測
- [ ] `mise run fmt-check` / `check` / `lint` / `test` /
  `test-rt` / `test-msrv` / `build`
- [ ] review-protocol で blocker なしを確認
