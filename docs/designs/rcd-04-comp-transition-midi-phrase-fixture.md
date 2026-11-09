---
id: "DESIGN-RCD04Fx2nB8cY6dF5hJ9kM3q"
type: "design"
status: "approved"
title: "RCD-04 Comp Transition / MIDI Phrase fixture — crate 内完結設計"
created: "2026-11-10"
updated: "2026-11-10"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/118"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/rcd-04-comp-transition-midi-phrase-fixture.md"
  implemented-by: []
  depends-on:
    - "docs/designs/mda-comp-effective-projection-placement-design.md"
    - "docs/designs/rcd-02-durability-evidence-recovery-review.md"
    - "docs/designs/rcd-03-monitoring-punch-latency.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/vs-d-recording-commit-e2e-idempotency-undo-reload.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers:
    - "MIDI comp selection の project 永続化 model（#126 event model 確定後）"
    - "実 RT engine（ruviform graph）との sample 一致（後続 integration slice）"
  deferred-by: []
---

# RCD-04 Comp Transition / MIDI Phrase fixture — crate 内完結設計

## Overview

spec `rcd-04-comp-transition-midi-phrase-fixture` の crate 層
fixture slice を、既存契約の再利用 + 最小 additive module で
実現する。新しい永続 schema・第二 writer・第二 Comp/MIDI/
Recovery model は導入しない。

```
Mock Input ──gate──▶ RT handoff ──▶ StreamWriter ──▶ staging
   (.pcm f32le)                                    (.journal)
        │                                              │
        │                                    finish_all / recover
        ▼                                              ▼
   Capture PCM (RT側実測)              finalized.json / canonical receipt
        │                                              │
        │                              commit_session (単一 mutation)
        ▼                                              ▼
              Project: takes/sources/clips ──SynvelisOp::Comp──▶ TakeGroup/Comp
                                                              │
                                    project_comp (純粋 projection)
                                                              ▼
                                    EffectiveComp ──▶ offline PCM renderer (test)
                                                              │
                                    capture PCM ──▶ RT側実測 PCM ──▶ 差分記録
```

MIDI 側は `MidiSource`（native tick）+ plan 層の phrase
selection から `compile_midi_comp` が決定的 playback plan
（event 列 + boundary report）を生成する。境界安全性
（hanging note / controller leak）は plan の構成で保証し、
fixture が ledger を検査する。

## Detailed Design

### 1. `synvelis-project/src/midi_comp.rs`（新規・additive）

永続 schema を変更しない plan 評価 module。`EffectiveComp`
（保存されない純粋 projection）と同じ位置付け。

#### 型

- `MidiCompRegionId` — `id.rs` に `define_id!(..., "midi-comp-region")`
  で追加。plan 文書内の region stable ID。
- `MidiPhraseRegion` — `{ id, source_id, phrase: MidiRange,
  project_range: MusicalRange }`。`phrase` は source-native
  tick の半開区間（onset が内部にある note を選択）、
  `project_range` は project musical tick。
- `MidiCompSelection` — `{ regions: Vec<MidiPhraseRegion> }`。
  `validate(&Project)` で sorted・非重複（隣接は可・gap は
  silence 区間として合法）・region id 一意・`source_id` が
  `midi_sources` に存在・phrase/project_range 非空・
  **同一 source の phrase 窓は非重複**（onset が 2 region に
  採用され `(source_id,note_id)` 重複発音になるのを構造的に
  防ぐ）・写像長一致を検査。
- `MidiControllerKind` — `Cc(u8)` / `PitchBend`。
- `MidiControllerEvent` — `{ tick: MidiTick, kind, value: i32 }`
  （CC は 0–127、PitchBend は 0–16383 に validate）。
- `MidiControllerLane` — `{ source_id, events }`。
  `MidiControllerLanes` — selection に付随する controller 入力。
  `MidiSource` は event を保持しないため、controller 系は
  plan 入力として fixture/呼出側が供給する（#126 の event
  model 確定までの接続点）。
- `MidiPlanEvent` — `{ at: MusicalTick, kind: MidiPlanEventKind,
  region: MidiCompRegionId, note: Option<(SourceId, NoteId)>,
  source_tick: Option<MidiTick> }`。
  `MidiPlanEventKind` = `NoteOn{key,velocity}` /
  `NoteOff{key}` / `ControllerChase{kind,value}` /
  `AllNotesOff`。
- `MidiRegionReport` — region ごとの boundary 証跡:
  `virtual_offs`（phrase 末端で切られた note）、`chase`
  （region 開始時の controller snapshot）、`notes`（採用
  note 数）。
- `MidiPlaybackPlan` — `{ comp_regions, events,
  region_reports }`。events は `(at, kind-rank, region, note)`
  で決定的に sort 済み。

#### `compile_midi_comp(project, selection, controllers)`

1. `selection.validate(project)`。
2. 時間写像: `src_delta → project tick` を
   `delta * project_ppqn / division` の正確な有理数で計算し、
   剰余が 0 でなければ `Error::Validation`（暗黙 quantize
   しない）。`MidiDivision::Ppqn` のみ対応（non_exhaustive
   のため将来 variant は個別対応）。
3. region ごと（project 順）:
   - `phrase` 長の写像値が `project_range` 長と一致すること
     を要求（MIDI comp は Musical+Musical Natural 1:1。
     stretch/loop は本 slice 対象外）。
   - onset（`start`）が `phrase` に含まれる note を採用
     （半開 `[start,end)`）。phrase 開始前に始まる note は
     採用しない（暗黙 chase-in しない）。
   - NoteOn を写像 project tick に、NoteOff を
     `min(写像 note.end, region.project_range.end)` に emit。
     末端で切られる note は `virtual_offs` に記録
     （hanging note 防止）。
   - region 開始時に、その source の controller lane の
     `phrase.start` 時点の effective state（tick ≤ phrase.start
     の最後の event）を `ControllerChase` として emit。
     全 lane で宣言されたことのある kind の和集合を対象に、
     未宣言 kind は default（CC=0 / PitchBend=8192）を emit
     して leak を構造的に排除する。
   - region 終了時: その region で鳴っている全 note に
     NoteOff（virtual 含む）を emit。隣接 region では
     off → on の順（同一 tick では kind-rank で off 優先）
     — 同一 `(source_id, note_id)` の重複発音は構造的に
     起きない（legato 継続は deferred、re-articulation
     policy）。
4. plan 末尾で `AllNotesOff` を emit。active note 集合は
   構成上必ず空になる（各 region 終了で全 note off）。
5. `MidiPlaybackPlan` を返す。`region_reports` が境界証跡
   （chase snapshot・virtual offs・採用 note）を保持し、
   fixture が「hanging なし・leak なし」を検査できる。

#### 非目標（deferred）

- MIDI comp selection の project 永続化・`CompOp` 統合・
  Undo/Redo/Save/Reload への組込み（#126 event model 確定後）。
- legato 継続（同一 note が境界を跨ぐ場合の再発音抑制）、
  note chase-in、fractional tick 写像（剰余 != 0 は error）。
- CC/Program/Pitch 以外の event kind、MPE、portamento。

### 2. `synvelis-project` fixture（tests 新規）

- `tests/rcd_04_comp_cas.rs`: MD-A seeded project（md_a.rs
  と同じ POD seeding）で TakeGroup/Comp を `CompOp` 経由で
  構築し、(a) stale `expected_revision` で `apply` が
  `Err`・live revision/comp registry/undo 不変、(b) 不正 op
  （重複 region・非隣接 transition・範囲外 candidate）が
  all-or-nothing に拒否され部分適用が残らない、(c) undo/redo
  で CompId/RegionId/TransitionId/TakeGroupId が完全一致、
  (d) `InMemoryStore` save→別 controller load で全 stable
  ID と `project_comp` projection が一致、(e) serialize→
  deserialize→serialize の semantic diff が comp 関連で
  差分なし（VCS-diff 相当）、(f) `project_comp` の決定性
  （2 回呼出で `PartialEq` 一致）・provenance 保持、
  (g) Take/Source/Asset の field 不変。
- `tests/rcd_04_midi_phrase.rs`: `ProjectOp` で MidiSource
  + note + clip を構築し、`compile_midi_comp` の
  (a) phrase onset 選択・note end 切断（virtual off）、
  (b) region 境界での off→on 順序・重複発音なし、
  (c) controller chase snapshot と kind 和集合による
  leak 排除、(d) comp 末尾 `AllNotesOff`・active 集合空、
  (e) native tick 保存（3840 division の note が plan の
  `source_tick` provenance に残り project tick は正確写像）、
  (f) 不正 selection（重複 region・他 lane source・
  長不一致・非 MIDI source_id）の拒否、
  (g) Undo/Redo・Save/Reload・serialized diff で
  SourceId/MidiSource.id/NoteId/ClipId と selection の
  region ID が維持される。

### 3. `synvelis-recording` fixture（tests 新規）

- `tests/rcd_04_take_comp_pcm.rs`:
  - vs_d.rs と同じ helper（`controller_with`/`open_session`/
    `request`/`record_and_finalize`/`record_close_commit`）で
    2 lane record → commit。lane あたり take×source×clip が
    得られる。
  - TakeGroup（owner lane・take_ids・target_range・
    source_rate=RATE）→ Comp → regions
    （`Take`/`ExistingSource`/`Silence` candidate・
    `Natural`/`Loop` mapping）→ `AddCompTransition`
    （overlap・FadeShape Linear/EqualPower）→ `SetActiveComp`
    を `SynvelisOp::Comp` で構築。
  - CAS/atomicity: stale revision apply 拒否・live 不変、
    不正 op 拒否で部分適用なし、失敗 apply が ID watermark
    を消費しないこと。
  - PCM: `project_comp` → test-local offline renderer
    （下記）で comp span の f32 PCM を生成。各 staging
    `media_path` の f32le-interleaved bytes を source PCM
    として読む。capture 前後で media/journal bytes の
    hash が不変であることを検証。
  - RT↔Offline 差記録: identity mapping・transition なし
    の comp 出力が対応 source PCM slice と bit-exact
    （diff=0）、transition ありでは overlap 域が解析的
    crossfade（player `lowering.rs` `FadeWindow::gain_at`
    と同じ curve: Linear=x, EqualPower=sin(x·π/2)、
    direction In/Out、domain=overlap span）と一致、
    `Loop` mapping は RT lowering の `UnsupportedMapping`
    skip 規則に整合する差分（offline は loop 再生・RT 側
    は skip=無音）として記録する。
  - feature 別 fixture: Source Handle=ExistingSource の
    `handle`/`play_from` 範囲、Fade=overlap+FadeShape、
    相関素材=2 take の相関/非相関 PCM（MockPcm seed 差）、
    Transient=source 境界に impulse 状の急変（MockPcm の
    決定的値で検証）、Beat Anchor/Tempo=tempo map 変更下で
    musical range→media frame 写像、Loop=Loop mapping
    region。各々で差分を assert + 記録。
- `tests/rcd_04_recovery_partial.rs`:
  - vs_c/recovery_review.rs の fake FS helper を再利用し、
    partial stream（unverified tail 残し）を作る。
  - `scan()` → `ReviewState::Pending` → `review_stream`
    `Hold`（cancel/defer: file 不変・判定記録 durable）→
    再 `scan` で `Held` → `review_stream` `Adopt`
    （retry: verified prefix を finalize・冪等）→
    `finalized.json` 出現・review 記録 `Adopted`。
  - `Adopt` 後の再 `Adopt` が既存 ref を冪等返却
    （duplicate なし）。`Hold` 中は media/journal bytes・
    project revision・take 数が不変。
  - Malformed/Corrupt（journal 破損 byte 注入）で
    `StreamClass::Corrupted`/`ReviewState::Unreadable`
    → `review_stream` が fail-closed（`Corrupt`/`Refused`）。
  - `Discard` は probe 未指定/`Referenced`/`Unknown` で
    拒否、`Clear` のみ削除（raw byte 検証は review で
    file 削除後の不存在確認）。
  - commit 済み manifest 存在 session への `Discard` 拒否
    （commit manifest 存在チェック）。

### 4. 既存契約の維持

- `synvelis-recording` の `ops.rs`/`commit.rs`/`recovery.rs`/
  `writer.rs`/`staging.rs` は変更しない（fixture は公開 API
  のみ使用）。
- `synvelis-project` の `comp.rs`/`project.rs`/`store.rs`/
  `control.rs`/`midi.rs`/`ops.rs` は変更しない。追加は
  `id.rs` の `define_id!` 1 行・`midi_comp.rs` 新規・
  `lib.rs` の module/export のみ。
- `Cargo.toml`/`Cargo.lock` の依存変更なし（serde・既存
  crate 内型のみ使用）。

## File Map

| file | 変更 |
| --- | --- |
| `crates/synvelis-project/src/id.rs` | `MidiCompRegionId` 追加 |
| `crates/synvelis-project/src/midi_comp.rs` | 新規 module |
| `crates/synvelis-project/src/lib.rs` | module + re-export |
| `crates/synvelis-project/tests/rcd_04_comp_cas.rs` | 新規 |
| `crates/synvelis-project/tests/rcd_04_midi_phrase.rs` | 新規 |
| `crates/synvelis-recording/tests/rcd_04_take_comp_pcm.rs` | 新規 |
| `crates/synvelis-recording/tests/rcd_04_recovery_partial.rs` | 新規 |
| `docs/{specs,designs,plans,test-specs,tasks,impl}/...` | 本 chain |

## Risk Register

| risk | 対応 |
| --- | --- |
| offline renderer が player lowering と乖離 | `FadeWindow::gain_at` と同一 curve・同一 domain 規則を fixture に明記。乖離発見時は fixture を lowering に合わせる（lowering が権威）|
| fractional tick 写像の扱い | 剰余 != 0 を `Error::Validation` で fail-closed。fixture は exact division（480/960/1920）のみ使用し native fidelity を保持 |
| MIDI comp 永続化の先行実装への誤解 | spec/design/impl record で plan 層限定・deferred を明記。PR 本文にも記載 |
| fake FS/PCM を実機 Pass と誤認 | 全 fixture doc コメントと impl record に NotRun 境界を明記 |
| pre-existing broken doc relations で graph router が blocked | main 既存の他 slice 起因。RCD-02/03 と同じく chain 文書で進行し impl record に記録 |

## Verification Matrix

| 検証 | 方法 | 期待 |
| --- | --- | --- |
| fmt | `mise run fmt-check` | Pass |
| check | `mise run check` | Pass |
| lint | `mise run lint` | Pass |
| test | `mise run test` | Pass（新規 4 test file 含む） |
| test-rt | `mise run test-rt` | Pass（存在する場合） |
| build | `mise run build` | Pass |
| 実 GUI/Device/OS | — | NotRun |
| AudioLens f64 | — | NotRun（consumer gate） |
