---
id: "IMPL-034UvMC7UhiwWgHviB9Qnv"
type: "impl"
status: "completed"
title: "RCD-04 Comp Transition / MIDI Phrase crate fixture slice 実装記録"
created: "2026-11-09"
updated: "2026-11-09"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/118"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-project/src/midi_comp.rs"
      - type: "file"
        path: "crates/synvelis-project/tests/rcd_04_comp_cas.rs"
      - type: "file"
        path: "crates/synvelis-project/tests/rcd_04_midi_phrase.rs"
      - type: "file"
        path: "crates/synvelis-recording/tests/rcd_04_recovery_partial.rs"
      - type: "file"
        path: "crates/synvelis-recording/tests/rcd_04_take_comp_pcm.rs"
      - type: "file"
        path: "docs/specs/rcd-04-comp-transition-midi-phrase-fixture.md"
      - type: "file"
        path: "docs/designs/rcd-04-comp-transition-midi-phrase-fixture.md"
      - type: "file"
        path: "docs/plans/rcd-04-comp-midi-phrase-fixture.md"
      - type: "file"
        path: "docs/test-specs/rcd-04-comp-midi-phrase-fixture.md"
      - type: "file"
        path: "docs/tasks/rcd-04-task1-comp-midi-fixture.md"
      - type: "file"
        path: "docs/impl/ir/rcd-04-comp-transition-midi-phrase-crate-fixture-slice.md"
    modified:
      - type: "file"
        path: "crates/synvelis-project/src/id.rs"
      - type: "file"
        path: "crates/synvelis-project/src/lib.rs"
      - type: "file"
        path: "docs/specs/README.md"
      - type: "file"
        path: "docs/designs/README.md"
      - type: "file"
        path: "docs/plans/README.md"
      - type: "file"
        path: "docs/test-specs/README.md"
      - type: "file"
        path: "docs/tasks/README.md"
      - type: "file"
        path: "docs/adr/README.md"
      - type: "file"
        path: "docs/impl/ir/README.md"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/tasks/rcd-04-task1-comp-midi-fixture.md"
    - "docs/plans/rcd-04-comp-midi-phrase-fixture.md"
    - "docs/specs/rcd-04-comp-transition-midi-phrase-fixture.md"
  implemented-by: []
  depends-on:
    - "docs/tasks/rcd-04-task1-comp-midi-fixture.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/ir/mda-takegroup-comp-effective-projection-placement.md"
    - "docs/impl/ir/mdb-effective-comp-lowering-load-revalidation-comp-dispatch.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/rcd-04-comp-transition-midi-phrase-fixture.md"
    - "docs/designs/rcd-04-comp-transition-midi-phrase-fixture.md"
    - "docs/plans/rcd-04-comp-midi-phrase-fixture.md"
  derived-by: []
  verifies:
    - "docs/test-specs/rcd-04-comp-midi-phrase-fixture.md"
  verified-by:
    - "docs/test-specs/rcd-04-comp-midi-phrase-fixture.md"
  references: []
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# RCD-04 Comp Transition / MIDI Phrase crate fixture slice 実装記録

## Summary

Issue #118（RCD-04 Comp Transition・MIDI Phrase）のうち crate 層
fixture slice を実装した。`synvelis-project` に MIDI comp の
決定的 playback plan コンパイラ `compile_midi_comp`（境界安全性・
controller chase・native tick 正確写像・stable region ID）を追加し、
`docs/` の spec→design→plan→test-spec→task チェーンに沿って
`synvelis-project`/`synvelis-recording` に独立 fixture を 4 本追加
した（project 16 test・recording 5 test）。GUI・実 Device・実 RT
engine は対象外であり、fake/mock 上の契約検証のみを行った。

## Context

- spec `docs/specs/rcd-04-comp-transition-midi-phrase-fixture.md`
- design `docs/designs/rcd-04-comp-transition-midi-phrase-fixture.md`
- plan `docs/plans/rcd-04-comp-midi-phrase-fixture.md`
- test-spec `docs/test-specs/rcd-04-comp-midi-phrase-fixture.md`
- task `docs/tasks/rcd-04-task1-comp-midi-fixture.md`

Issue #118 の crate 層受入条件（Source Handle/Fade/相関素材/
Transient/Beat Anchor/Tempo/Loop の RT↔Offline PCM 差記録、MIDI
phrase 境界安全性と Stable ID、Recovery partial+cancel+retry、
Group Comp CAS、raw PCM/Asset/元 Take 不変）を headless fixture
でカバーする。`synvelis-app`/`synvelis-workspace`/`synvelis-player`/
Ruviform は変更していない。

## Implementation

### Changes

- `crates/synvelis-project/src/id.rs`: `MidiCompRegionId`
  （`"midi-comp-region"`）を `define_id!` で追加。
- `crates/synvelis-project/src/midi_comp.rs`（新規）:
  `MidiPhraseRegion`/`MidiCompSelection`/`MidiControllerKind`/
  `MidiControllerEvent`/`MidiControllerLane(s)`/
  `MidiPlanEvent(Kind)`/`MidiRegionReport`/`MidiPlaybackPlan` と
  `compile_midi_comp` を実装。note 採用は onset が phrase window
  内のもののみ、window を越える note には仮想 note-off、境界は
  off→chase→on の rank 順、各 region 先頭で controller kind union
  の chase（未宣言 kind は default で漏洩防止）、phrase 内 CC は
  mapped tick で emit、plan 末尾は `AllNotesOff`。tick 写像は
  `delta * ppqn / division` が厳密な場合のみ許可し native tick を
  requantize しない。selection は `Serialize`/`Deserialize` で
  stable ID を保持する document 型。
- `crates/synvelis-project/src/lib.rs`: `midi_comp` module 公開 +
  re-export。
- `crates/synvelis-project/tests/rcd_04_comp_cas.rs`（新規・5 test）:
  Group Comp stale-revision CAS 拒否・invalid op all-or-nothing・
  Take/Source/Asset 非破壊・Undo/Redo・Save/Reload・serialized
  diff での Stable ID 維持・`project_comp` 決定性と provenance。
- `crates/synvelis-project/tests/rcd_04_midi_phrase.rs`（新規・
  11 test）: region overlap/order/empty・重複 region id・
  同一 source の phrase 窓重複・zero division・非 MIDI/欠落
  source・厳密/非厳密 tick 写像・native tick 保持・決定的
  ordering・hanging note/leak なし・controller union chase +
  phrase 内 CC・stable selection ID・Undo/Redo・Save/Reload。
- `crates/synvelis-recording/tests/rcd_04_take_comp_pcm.rs`
  （新規・1 test）: 実 recording 経路（MockInput → gate →
  staging writer → finalize → `commit_session`）で commit した
  3-pass loop take を comp に構成し、`project_comp` の
  `EffectiveComp` を file 内の決定的 offline renderer
  （player `lowering.rs` と同一の Natural 1:1・overlap
  double-play・`FadeWindow::gain_at` curve — 依存方向の都合で
  player を import しない）で評価。identity 域は bit-exact、
  transition overlap は解析 gain どおりの crossfade と unity
  double-play との差、Loop/`SimpleStretch`/Silence は lowering の
  skip 規則どおり無音であることを記録。comp ops + render 後の
  media byte/Takes/Sources/Assets 不変と決定性も検証。
- `crates/synvelis-recording/tests/rcd_04_recovery_partial.rs`
  （新規・4 test）: partial stream の verified prefix のみ commit・
  Hold/cancel で media/journal bytes 不変・Adopt/retry の冪等性・
  corrupt journal fail-closed・Discard の clear reference probe
  要件・group commit での partial/held 分離。
- `docs/` チェーン 5 文書 + 各 README index 追加。README index
  には main 上に既存の merge conflict marker が残っていたため、
  union merge で解消し RCD-03 行を復元した（本 slice の文書追加と
  同一 diff で整理）。

### Decisions During Implementation

- MIDI comp は plan 層の純粋投影として実装し、永続 comp model
  （selection の project への登録 op）は #126 の native-tick/chase
  event model 確定まで deferred とした（test-spec の Unsupported
  宣言どおり）。
- RT↔Offline PCM 比較は player crate を import せず、lowering と
  同一規則を test file 内に再現した。依存方向
  （`recording → project` のみ、player は上位）のためであり、
  実 RT engine の PCM 一致を主張するものではない。
- controller chase は「その時点までに宣言された kind の union」
  を各 region 先頭で snapshot し、未宣言 kind は default 値で
  chase して region 間の state 漏洩を防ぐ設計とした。
- `compile_midi_comp` は selection 検証 → controller 入力検証 →
  region ごとの emit → 全 event の決定的 sort（tick, action rank,
  region, note, source_tick）という単一 pass で構成し、失敗時は
  plan を返さない（all-or-nothing）。
- review-protocol で検出した契約 gap を修正: region id 一意性・
  同一 source の phrase 窓非重複（onset が 2 region に採用され
  `(source_id,note_id)` 重複発音になるのを構造的に防止 —
  test-spec F5 の Rule を実装で担保）・`MidiDivision::Ppqn(0)`
  の fail-closed（`% 0` panic 防止）。design 文書の validate
  記述も同期した。

## Related Experiments

### Adopted

<!-- 実験 log なし（既知解の機械的実装 + fixture 作成）。 -->

### Rejected

<!-- なし -->

## Validation

canonical 検証（`mise.toml` タスク、全て exit 0）:

- `mise run fmt-check` — Pass（`mise run fmt` 適用後）
- `mise run check` — Pass
- `mise run lint` — Pass（`cargo clippy --workspace --all-targets
  -- -D warnings`、warning 0）
- `mise run test` — Pass（`cargo test --workspace --locked`、
  新規 21 test を含む全 test が Pass）
- `mise run test-rt` — Pass
- `mise run build` — Pass

新規 fixture の内訳: `rcd_04_comp_cas` 5 test・
`rcd_04_midi_phrase` 11 test・`rcd_04_recovery_partial` 4 test・
`rcd_04_take_comp_pcm` 1 test、計 21 test 全て Pass。

NotRun / Unsupported（test-spec 宣言どおり、fake fixture は
以下を証明しない）:

- 実 OS/Device/FS durability・実 GUI・2-window stale
  gesture/history view — NotRun（後続 slice）
- AudioLens native-f64/raw-recording 非回帰 — NotRun
  （RF#294 consumer gate 側）
- 実 RT engine/player rendering による comp PCM 一致 — NotRun
  （本 fixture は lowering と同一規則の offline 再現との比較）
- MIDI selection の project 永続化・legato 継続・fractional
  tick 写像・非対応 event type — Unsupported/deferred

## Risks

- offline renderer は lowering.rs の規則を test 内に複写して
  おり、lowering 側の将来変更に追従しない。player 実装との
  drift は本 fixture では検出できない（実 RT engine 検証は
  後続 slice の責務）。
- `midi_comp` の plan 型はまだ永続化されない。selection document
  の Save/Reload は serialized bytes の stable ID 保持までを
  検証しており、project への comp 登録経路は未実装。

## Follow-ups

- Issue #118 残件: `synvelis-app` 製品操作 E2E・2-window stale
  gesture/history view（別 dispatch）、#114 GPUI 実 Window、
  AudioLens f64/raw-recording 非回帰（consumer gate）。
- MIDI comp selection の永続 model 登録は #126 の event model
  確定後に別 slice で扱う。
