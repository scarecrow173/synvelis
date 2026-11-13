---
id: "DESIGN-034WEV4lGiVlfIG0LsEBNr"
type: "design"
status: "approved"
title: "RCD-04 MIDI phrase boundary / Undo-Redo / VCS stable-ID design"
created: "2026-11-13"
updated: "2026-11-13"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/118"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-player/tests/rcd_04_midi_phrase_boundary.rs"
      - type: "file"
        path: "crates/synvelis-project/tests/rcd_04_vcs_stable_id.rs"
      - type: "file"
        path: "docs/specs/rcd-04-midi-undo-stableid.md"
      - type: "file"
        path: "docs/designs/rcd-04-midi-undo-stableid-design.md"
      - type: "file"
        path: "docs/plans/rcd-04-midi-undo-stableid-plan.md"
      - type: "file"
        path: "docs/tasks/rcd-04-task-midi-undo-stableid.md"
      - type: "file"
        path: "docs/test-specs/rcd-04-midi-undo-stableid.md"
      - type: "file"
        path: "docs/impl/ir/rcd-04-midi-undo-stableid-slice.md"
    modified:
      - type: "file"
        path: "crates/synvelis-project/tests/rcd_04_comp_cas.rs"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/rcd-04-midi-undo-stableid.md"
  implemented-by: []
  depends-on:
    - "docs/designs/rcd-04-comp-transition-midi-phrase-fixture.md"
    - "docs/designs/rcd-04-rt-engine-comp-pcm-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/rcd-04-product-e2e-design.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/rcd-04-midi-undo-stableid.md"
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# RCD-04 MIDI phrase boundary / Undo-Redo / VCS stable-ID design

## Context

Issue #118 の残件 3 項目のうち headless 検証可能な部分を固定す
る。本 slice は新規実装ではなく **既存契約の未検証経路を pin
する test-only slice** であり、設計判断は「どの経路がまだ未
固定か」の切り分けと fixture 形状に集約される。

既存カバレッジの棚卸し:

- PR#219 `compile_midi_comp`（`synvelis-project/src/midi_comp.rs`）:
  plan-level の phrase 境界安全性・virtual off・controller
  chase union・厳密 tick 写像・stable region ID は
  `rcd_04_midi_phrase.rs` で固定済み。
- PR#300 app 層 E2E: comp transition・MIDI phrase・recovery
  review の製品経路 5 tests（app 層、本 slice 範囲外）。
- PR#319 `rcd_04_rt_comp_pcm.rs`: PCM 側の 3 経路 bit-exact
  （audio comp のみ。`lower_semantic`/`RoutingEngine`/`export`
  に MIDI event は存在しない — MIDI の parity 対象はない）。
- TIM-02 `tim02_midi_playback.rs`: `compile_midi_events` →
  `dispatch_midi` → `PluginNode` 配送の wire/loop/seek/panic/
  SMF provenance は固定済みだが、**region 間境界**（off→chase→
  on 同一 tick・同一 key 再発音）は対象外。
- `rcd_04_comp_cas.rs`: comp CAS・all-or-nothing・transition
  追加の undo/redo・save/reload・serialized diff・raw 不変は
  固定済みだが、**`SetActiveComp` 切替**は未 pin。
- VCS: `CompingAdapter` が take group/comp/region/transition
  を stable ID で diff し、`restore` が comp entity を計上す
  るが、RCD-04 観点の横断 fixture はない。`apply/entity.rs`
  の `supported_kind` は comp kind を含まず fail-closed —
  これが意図的境界かどうかは pin されていなかった。

## Scope

### In Scope

- `crates/synvelis-player/tests/rcd_04_midi_phrase_boundary.rs`
  （新規）— 境界 fixture と期待 wire 列の設計。
- `crates/synvelis-project/tests/rcd_04_comp_cas.rs` への
  comp 切替テスト追加。
- `crates/synvelis-project/tests/rcd_04_vcs_stable_id.rs`
  （新規）— snapshot/diff/apply/restore 横断の stable-ID 設計。

### Out of Scope

- src 側の挙動変更（契約はすべて既存実装が満たす想定 —
  実装はテストが赤くなった場合のみ、かつ owning boundary で
  修正する）。
- comp kind の apply whitelist 拡張。
- MIDI comp 永続化・semantic/render MIDI 経路・実機経路。

## Design

### D1 MIDI 境界 fixture（player）

`tim02_midi_playback.rs` の fixture 群（host plugin recorder・
`driver_fixture`・`compile_events`・wire builders）を踏襲し、
`rcd_04_midi_phrase.rs` と同じ canonical 2 region selection を
使う:

- division 480 → project PPQN 960（厳密 ×2）、25 frames/tick。
- Region A: phrase `[480,2400)` → project `[0,3840)`。
- Region B: phrase `[2400,3840)` → project `[3840,6720)`。
- note 群: A 内 natural on/off・A 端で cut される長音・境界直
  前の tiny note・**B で同一 key64 を再発音**・B 内 natural・
  comp 末端で cut される note・phrase A 開始前の除外 note。
- controller lanes: ch0 CC64/CC1/PitchBend・ch1 CC64/CC10 —
  chase union 5 pair、ch1 を混ぜて channel 分離を検証。

検証は 2 層:

1. `compile_events` 出力（`Vec<(i64, MidiEvent)>`）が手導出の
   期待列と完全一致 — 境界 frame の `NoteOff(virtual) →
   ControllerChase×5 → NoteOn(同一 key)` 順序、chase 値
   （latest ≤ phrase_start、未宣言は default）を固定。
2. `HeadlessDriver` + attached fixture plugin の配送 log が同
   一列を `abs_frame` 付きで配送 — 加えて配送列に対する
   ledger 走査（active `(channel,key)` set）で「全 note-on が
   一意の note-off と対・stream 終端で active 空・重複発音な
   し」を独立に検査する。期待列との一致だけに依存しない第 2
   の不変条件 pin。

semantic/render parity は存在しない（MIDI は engine 配送経路
のみ）ため、本 fixture が MIDI 境界の最下位意味層となる。

### D2 comp 切替の単一 CAS/Undo（project）

`rcd_04_comp_cas.rs` の seeded/build_comp を再利用し、同一
take group に 2 つ目の comp（1 region の Silence）を作成して
`SetActiveComp` で comp1→comp2 に切替える。pin 内容:

- 切替 apply がちょうど +1 revision（1 CAS step）。
- `comp.comps`/`regions`/`transitions` の JSON が切替前後で
  byte 同一（active pointer のみが動く）。
- 単一 `undo()`（+1 revision — undo は新 revision を発行する
  既存契約）で comp registry 全体が切替前に完全一致復元、
  `redo()` で comp2 が同一 ID のまま再適用。
- 切替系列全体を通じて `recording.takes`/`sources`/`assets`
  が seeded POD と一致し、take state が `Verified` のまま。

### D3 VCS stable-ID（project）

`vcs02_diff.rs`/`vcs05_apply.rs`/`vcs05_restore.rs` の harness
（`AllPresentProbe`・`snap_live`・`compare`・`Src`/`sources_for`
・`MapBlobs`・`tmp_dir`）を踏襲し、comp_cas と同じ POD seed
（asset digest は restore の sha256 検証を通すため実 digest）
+ `build_comp(with_transition)` で fixture を構成。pin 内容:

1. **save → saved-HEAD → diff**: `c.save(InMemoryStore)` →
   `resolve_saved_head` → `comp_key_sets`（4 map の tagged ID
   列）が live と一致、live vs saved の `compare_snapshots`
   が `Equal`・changes 0。
2. **diff の独立 identity**: transition 有/無の 2 controller
   （同一 op prefix → 共有 entity の ID が一致）を比較し、
   changeset が `CompTransition` の `Added`/`Removed` 1 件のみ
   を含み、`object.stable_id == transition.to_tagged()`
   （`comp-transition-<32hex>`）であることを双方向で固定。
   親 `Comp.transition_ids` は `ordered_list_class` の
   membership Skip で change を生じない — transition の identity
   が親に従属しないことの裏付け。
3. **apply fail-closed**: `CompTransition` change を選択した
   intent で `target_fingerprint` が
   `ApplyError::Unsupported{kind: CompTransition}` を返し、
   `apply_selected` も同じく fail-closed（live revision・comp
   JSON 不変）— 現行の apply surface 境界を契約として固定。
4. **restore_as_copy**: 実 sha256 digest の asset blob を
   `MapBlobs` で供給し `MissingAssetsPolicy::Fail` で発行。
   receipt の `stable_ids.counts` が take-group/comp/
   comp-region/comp-transition を計上し、発行された
   `snapshots/<HEAD>` の `SnapshotEnvelope.project.comp` の
   4 map key 集合が live と完全一致。

### 却下した代替案

- **comp kind の apply 対応追加**: 受入条件は stable-ID 保存
  の検証であり apply ではない。`entity.rs` への typed
  extraction/transplant/membership reconcile 追加は別 slice
  の設計事項 — 現行 fail-closed を pin する方が正直。
- **`MidiCompSelection` の永続化**: #126 待ちの明示的範囲外。
  stable-ID 検証は永続 comp entity（`CompTransition` 等）で行
  うのが受入条件の文言とも一致。
- **semantic/render 経路の MIDI parity**: その経路に MIDI
  event が存在しないため比較対象がない — 無い parity を
  捏造しない。

## Risks

- 期待 wire 列は手導出 — plan 実装と同じ誤読を共有する可能性
  は残るが、project 側 `rcd_04_midi_phrase.rs` が plan 契約を
  独立に pin しており、wire 層の drift は本 fixture が検出す
  る構造。
- restore fixture は実 sha256 digest を使うが FS 耐久性認定で
  はない（`vcs05_restore` 同様、published layout と ID 保存の
  headless 検証に留まる）。
