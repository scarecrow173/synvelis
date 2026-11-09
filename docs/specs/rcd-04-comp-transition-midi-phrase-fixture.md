---
id: "SPEC-RCD04Fx7kQ2mV9wX4pL8sT1u"
type: "spec"
status: "proposed"
title: "RCD-04 Comp Transition / MIDI Phrase — crate fixture slice (synvelis#118)"
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
  implements: []
  implemented-by: []
  depends-on:
    - "docs/specs/mda-takegroup-comp-effective-projection-placement-contract.md"
    - "docs/specs/rcd-02-durability-evidence-recovery-review.md"
    - "docs/specs/rcd-03-monitoring-punch-latency.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/vs-d-recording-commit-e2e-idempotency-undo-reload.md"
    - "docs/specs/x-recorded-verified-source-playback-export.md"
  refines: []
  refined-by: []
  derives-from:
    - "https://github.com/scarecrow173/synvelis/issues/118"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "https://github.com/scarecrow173/synvelis/issues/10"
    - "https://github.com/scarecrow173/synvelis/issues/11"
    - "https://github.com/scarecrow173/synvelis/issues/12"
    - "https://github.com/scarecrow173/synvelis/issues/42"
    - "https://github.com/scarecrow173/synvelis/issues/50"
    - "https://github.com/scarecrow173/synvelis/issues/114"
    - "https://github.com/scarecrow173/synvelis/issues/116"
    - "https://github.com/scarecrow173/synvelis/issues/126"
    - "https://github.com/scarecrow173/synvelis/issues/127"
  defers: []
  deferred-by: []
---

# RCD-04 Comp Transition / MIDI Phrase — crate fixture slice (synvelis#118)

## What Should Be Built

Issue #118（RCD-04 Comp Transition・MIDI Phrase）のうち、
`synvelis-recording` / `synvelis-project` 両 crate 内で完結する
headless fixture slice を実装する。実 GUI・実 Device・実 OS・
AudioLens consumer gate は対象外（NotRun）。

- `synvelis-recording`: 実 capture 経路（Mock Input → gate → RT
  handoff → staging writer → finalize → `commit_session` 単一
  mutation）で得た take/source/clip を使い、`SynvelisOp::Comp`
  経由で TakeGroup/Comp を構築する独立 fixture。Group Comp の
  expected-revision CAS・all-or-nothing・Undo/Redo・Save/Reload・
  raw media byte 不変を検証する。Effective Comp projection を
  決定的 offline renderer で PCM 評価し、capture 済み PCM
  （RT 側実測値）との差を feature 別に記録する。
- `synvelis-recording`: Recovery partial + cancel(Hold) + retry
  (Adopt) と Malformed/Corrupt/Source Missing での fail-closed・
  Raw/元 Take/Project Revision/PCM 非改変を独立 fixture で検証
  する（RCD-02 `review_stream` API を再利用）。
- `synvelis-project`: MD-A Comp の CAS/atomicity/immutability/
  Stable ID（Undo/Redo・Save/Reload・serialized diff）fixture と、
  MIDI Phrase 境界 oracle `midi_comp` module（additive・plan
  評価層）+ 境界 fixture を追加する。

## Why Now

Issue #118 の監査どおり、crate 層 fixture から着手可能
（#116 全面完成・#114 製品 GUI は着手条件ではない）。
RCD-01/02/03 で recording crate の capture/staging/recovery/
monitor 契約と project crate の MD-A comp 契約が揃ったため、
両者を跨ぐ Take/Comp/Recovery の決定的証拠を fixture として
固定できる。製品 GUI E2E（#114 系）と AudioLens f64 非回帰
（Ruviform #294 consumer gate）は後続 slice の gate であり
本 slice の完了を妨げない。

## Users and Value

- 後続 slice（app GUI E2E・workspace ControlSession 配線・
  AudioLens consumer）: crate 層で固定した Comp/Recovery/
  MIDI 境界契約をそのまま再利用できる。
- 検証者: RT capture PCM と offline comp 評価 PCM の差を
  feature（Source Handle/Fade/相関素材/Transient/Beat
  Anchor/Tempo/Loop）別に golden として確認できる。

## Scope

### In Scope

- `crates/synvelis-recording/tests/`: Take/Comp/PCM fixture・
  Recovery partial/cancel/retry fixture（新規 test file）。
- `crates/synvelis-project/tests/`: Comp CAS/Stable ID fixture・
  MIDI Phrase 境界 fixture（新規 test file 優先）。
- `crates/synvelis-project/src/midi_comp.rs`（新規 additive
  module）と `id.rs` への `MidiCompRegionId` 追加、`lib.rs`
  export。永続 schema・既存型の変更は行わない。
- `docs/` チェーン（spec/design/plan/test-spec/task/impl）。

### Out of Scope

- `synvelis-app` 製品 GUI E2E・2Window Stale Gesture/History
  view — 別 dispatch（SF-05）稼働中・後続 slice。
- `synvelis-workspace` ControlSession 配線 — 別 dispatch 稼働中。
- `synvelis-player`/`synvelis-render` の変更（player の
  `lowering.rs` は comp→graph  lowering + fade gain の既存
  権威として参照のみ。依存方向上 recording から player へは
  依存できない）。
- Ruviform 共通基盤（Device/File/Clock/MIDI/RT/Plugin）の変更。
- AudioLens native-f64/raw-recording 非回帰 — Ruviform #294
  consumer gate・NotRun。
- #114 GPUI 実 Window review — NotRun（実機 gate）。
- MIDI comp selection の project 永続化 model — 本 slice は
  plan 評価層のみ。永続 model は #126（Native MIDI Tick/Chase
  Fidelity）側の event model 確定後に判断する deferred
  concern として記録する。
- 実 OS/Device/FS durability・実 GUI の Pass 主張 — fake/mock
  fixture の成功を実機 Pass に昇格しない。

## Requirements

- Take/Comp/Recovery fixture は既存契約を再利用する:
  `RecordingOp`/`SynvelisOp::Comp(CompOp)`/`ProjectController`
  （expected-revision CAS・clone-apply-validate-commit）・
  `project_comp`（純粋決定的 projection）・`review_stream`
  （Adopt/Hold/Discard fail-closed）。第二の store/writer や
  第二の Comp/MIDI/Recovery model を作らない。
- Group Comp の atomicity は controller の per-apply
  clone+validate+commit と expected-revision CAS で検証する。
  失敗した op は revision・comp registry・ID watermark・
  undo stack のいずれも進めない（all-or-nothing）。
- raw PCM（staging `streams/*.pcm` f32le-interleaved）・
  Asset・元 Take の byte/field は comp 操作・undo/redo・
  save/reload の前後で不変であること。
- MIDI Phrase 境界は Hanging Note（phrase/comp 終端を越える
  note が off されない）と Controller Leak（region 間で
  controller 状態が暗黙持ち越しされる）を plan 層で
  fail-closed に検証する。`MidiSource` の native tick/
  division を暗黙 quantize しない（project tick へ写せない
  境界は error）。
- fake FS/mock PCM の成功を実 OS/Device/GUI Pass として
  報告しない。実行不能な検証は NotRun/Unsupported と記録。

## Acceptance Criteria

- [ ] Source Handle 範囲/Fade 区間/相関素材/Transient/Beat
      Anchor/Tempo/Loop の各 fixture で、capture PCM（RT 側）
      と offline comp 評価 PCM の差が決定的に記録される
      （identity 域 bit-exact・transition 域は解析的 crossfade・
      Loop/非対応 mapping は RT lowering の skip 規則と整合）。
- [ ] MIDI phrase 境界で Hanging Note/Controller Leak がなく、
      Undo/Redo・Save/Reload・serialized diff で Stable ID
      （SourceId/MidiSource.id/NoteId/ClipId/Comp*/TakeGroupId/
      selection region ID）が維持される。
- [ ] Recovery partial+cancel+retry が headless で検証され、
      Group Comp CAS が stale revision・不正 op で
      all-or-nothing に拒否される。Malformed/Corrupt/Source
      Missing で Raw/元 Take/Project Revision/PCM が非改変。
- [ ] 既存 MD-A/B・raw PCM/Asset 不変の fixture 検証があり、
      AudioLens native-f64 非回帰は consumer gate 側で
      NotRun と記録される。
- [ ] `mise run fmt-check`/`check`/`lint`/`test`/`test-rt`/
      `build` が全て Pass する。

## Deferred Design Concerns

- MIDI comp selection の永続化（project schema への
  `MidiComp`/`MidiCompRegion` 追加・CompOp 統合）| #126 の
  Native MIDI Tick/Chase Fidelity と event model が未確定で
  あり、本 slice で先行確定すると両者の設計を縛る | #126
  event model 確定後 | phrase comp の Undo/Save/Reload が
  plan 文書レベルの round-trip に留まる
- 実 RT engine（ruviform graph）での comp 再生との PCM 一致
  | player/render crate は本 slice の編集範囲外・lowering は
  既存権威 | 後続 integration slice | fixture の offline
  renderer と実 engine の sample 一致は未検証のまま残る
- numeric fade duration・gain curve 係数 | `FadeShape` は
  Linear/EqualPower の形状のみで時間量を持たない（MD-A
  deferred）| fade duration 契約の確定後 | fixture は overlap
  範囲全体を fade domain とする既存 lowering 規則に従う
- 実 OS/FS durability・製品 GUI・AudioLens f64 | 実機 gate・
  別 dispatch | 実機環境整備後 | fake fixture を実機 Pass と
  誤認するリスク（文書で明記して抑止）

## Sources and Relations

- Issue: <https://github.com/scarecrow173/synvelis/issues/118>（RCD-04）
- 前提契約: MD-A spec/design（TakeGroup/Comp/Effective Comp）、
  VS-D spec（commit 単一 mutation）、RCD-02 spec（Recovery
  Review）、RCD-03 spec（monitor/punch/latency）、#52 MIDI
  Source/Note editing contract
- 関連 Issue: #10/#11/#12（Take/Comp/Editing）、#42（MD-A
  TakeGroup/Comp）、#50（MD-B effective comp lowering）、#114
  （製品 GUI review）、#116（durability/recovery）、#126
  （Native MIDI Tick/Chase）、#127（Plugin/Tempo/Event
  precision）、Ruviform #294（AudioLens consumer gate）
