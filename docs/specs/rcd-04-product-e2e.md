---
id: "SPEC-034VmNS87JaOfBSbYwh0pt"
type: "spec"
status: "proposed"
title: "RCD-04 Comp Transition / MIDI Phrase / Recovery Review — product E2E (synvelis#118)"
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
    - "docs/specs/rcd-04-comp-transition-midi-phrase-fixture.md"
    - "docs/specs/rcd-02-durability-evidence-recovery-review.md"
    - "docs/specs/vcs-04-history-visual-diff-ui.md"
    - "docs/specs/sv-gui-04-01-two-window-slice1.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/vs-d-recording-commit-e2e-idempotency-undo-reload.md"
    - "docs/specs/mda-takegroup-comp-effective-projection-placement-contract.md"
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
    - "https://github.com/scarecrow173/synvelis/issues/84"
    - "https://github.com/scarecrow173/synvelis/issues/95"
    - "https://github.com/scarecrow173/synvelis/issues/114"
    - "https://github.com/scarecrow173/synvelis/issues/116"
    - "https://github.com/scarecrow173/synvelis/issues/126"
    - "https://github.com/scarecrow173/synvelis/issues/127"
  defers: []
  deferred-by: []
---

# RCD-04 Comp Transition / MIDI Phrase / Recovery Review — product E2E (synvelis#118)

## What Should Be Built

Issue #118（RCD-04）の app 層 slice: 製品が実際に使う `syn.*`
op 文字列 dispatch 経路（`UiRuntime::dispatch_op` →
`Service::call` → `AppService` → `SynvelisApp`）を headless で
駆動し、Comp Transition・MIDI Phrase・Recovery Review の製品
レベル証拠を `crates/synvelis-app/tests/` に追加する。
crate 層 fixture（PR #219 / `9798be5b`、21 tests）は main
統合済みであり、本 slice はその上に app 層の製品 E2E のみを
積む。第二の project/recording/comp モデルは作らない。

- Comp Transition: TakeGroup→Comp→Region→Transition の構築を
  project op で行い、`syn.recording.info` 投影・Group Comp
  全体 CAS（stale expected-revision 拒否）・`syn.project.undo`/
  `redo`・`syn.project.save`/`reload`・VCS diff での Stable ID
  維持を製品 op 経路で検証する。
- MIDI Phrase: MIDI source/clip/note を project op で構築し、
  `compile_midi_comp` の plan 評価で phrase 境界の
  Hanging Note / Controller Leak 不在を検証。Undo/Redo・
  Save/Reload・VCS diff で Note stable ID が維持されること、
  plan が決定的であることを確認する。
- Recovery Review: staging partial + cancel(Hold) + retry
  (Adopt) → `commit_session` の all-or-nothing 分離
  （Complete のみ commit・Partial は recoverable）を製品
  session に対して実行し、`syn.recording.info` 投影・
  Undo/Redo・Save/Reload・corrupt journal / probe 無し Discard
  の fail-closed を検証する。
- 2Window/Stale: 2 view が同一 project session を共有する
  構成で stale-generation `syn.control.dispatch` が drop され、
  現行 generation は receipt を発行すること、VCS history view
  の一連 op が project revision を進めない read-only である
  ことを検証する。

実 Window・実 OS GUI・実 Device・実 OS FS durability・実
RT engine sample 一致・AudioLens native-f64 実測は本 slice
では実行せず `NotRun`/`Unsupported` として記録する。
MIDI comp 永続化は synvelis#126 側残件のため対象外
（plan 評価層のみ）。

## Why Now

crate 層 fixture は domain 契約を固定したが、「製品が実際に
呼ぶ `syn.*` op 面から同じ契約が観測できるか」は未検証。
`UiRuntime`/`AppService`/`SynvelisApp` の headless dispatch
経路・`syn.vcs.*`/`syn.control.*`/`syn.project.*`/
`syn.recording.info` の wire shape を通した証拠がないと、
後続の実 GUI slice（#114 系）が app 層の退行を検出できない。
本 slice は fixture 再利用のみで app 層に閉じるため、
`synvelis-project`/`synvelis-recording` の実装変更なしに
着手できる。

## Users and Value

- 後続 slice（実 GUI E2E・workspace ControlSession 配線・
  AudioLens consumer）: 製品 op 面で固定した Comp/MIDI/
  Recovery 契約をそのまま GUI 層へ持ち越せる。
- 検証者: `syn.*` wire JSON 越しに stable ID・revision・
  read-only 性・fail-closed が観測でき、fake fixture が
  実機 Pass を偽装しない境界が明示される。

## Scope

### In Scope

- `crates/synvelis-app/tests/rcd_04_product_e2e.rs`（新規）:
  上記 5 scenario を `UiRuntime` + `AppService` 共有 harness
  で実装。fixture 変異（comp/recording/MIDI op、staging
  writer、VCS backend 注入）は `AppService::app_mut()` が
  返す同一 `SynvelisApp` の canonical session apply 経路を
  使い、`syn.*` 文字列 dispatch のみを検証対象とする。
- `docs/` チェーン（spec/design/plan/test-spec/task/impl）。

### Out of Scope

- `crates/synvelis-project/`・`crates/synvelis-recording/`・
  `crates/synvelis-player/`・`crates/synvelis-workspace/`・
  `crates/synvelis-gateway/`・`crates/synvelis-library/`・
  `ui/`・`.github/workflows/`・`mise.toml` の変更
  （`synvelis-project` は別 Worker 占有。fixture 追加が
  必要になれば ask で範囲拡張を依頼する）。
- 実 Window/OS GUI・実 device・実 plugin の Pass 主張 —
  NotRun。
- 実 OS filesystem durability（power-loss/実 disk fsync
  意味論）— mock/staging file は契約証拠であり実機 Pass
  に昇格しない。
- 実 RT engine（ruviform graph）との sample 一致 —
  headless offline 評価のみ。NotRun。
- AudioLens native-f64/raw-recording 非回帰 — Ruviform #294
  consumer gate・NotRun。
- MIDI comp selection の project 永続化 — #126 event model
  確定後の deferred concern。本 slice は plan 評価層の
  境界安全性のみ。
- Source Handle 範囲/Fade 区間/相関素材/Transient/Beat
  Anchor/Tempo/Loop での RT PCM vs Offline PCM 差の実測 —
  headless で記録可能な構造的証拠（stable ID・plan 決定性・
  境界安全性）のみ。本格的な PCM 差分 golden は crate 層
  fixture（`rcd_04_take_comp_pcm`）が担う。

## Requirements

- 全ての製品操作は `syn.*` 文字列 op として `UiRuntime`
  dispatch を通す。test 専用の第二 dispatch・第二モデルを
  作らない。fixture 構築に必要な domain op は
  `ProjectSession::apply(SynvelisOp, expected_revision)` の
  canonical 経路（undo/redo/CAS の対象となる同一 mutation
  経路）を使う。
- Comp/Take/Recovery の domain 意味論は crate 層 fixture
  （PR #219）と既存 MD-A/B 契約を再利用し、ここで再定義
  しない。
- VCS 証拠は `syn.vcs.visual`/`syn.vcs.view` の wire JSON
  （endpoint freshness・diff summary・changes の
  `object_kind`/`stable_id`）で観測し、project revision が
  VCS op で不変であることを確認する。
- Recovery Review は `recovery::scan` + `review_stream`
  （Hold/Adopt/Discard）+ `commit_session` を staging file
  fixture で実行し、committed/recoverable 分離と
  `syn.recording.info` 投影を確認する。実 OS FS・実 device
  の成功を主張しない。
- 失敗分類は正直に行う: headless で実行したものだけを
  Passed とし、実 GUI/device/OS/AudioLens/RT sample は
  `NotRun` または `Unsupported` と記録する。

## Acceptance Criteria

- [x] Comp Transition: `syn.*` op 経路で TakeGroup/Comp/
  CompRegion/CompTransition が構築・Undo/Redo・Save/Reload
  され、stable ID が VCS diff（committed pre vs committed
  post で Added、committed post vs live で変更 0 件）に
  現れる。Group Comp 全体の stale expected-revision が
  `StaleRevision` で原子的に拒否される。
- [x] MIDI Phrase: `compile_midi_comp` の plan が phrase
  境界で NoteOn/NoteOff を 1:1 に保ち（hanging note・
  controller leak なし）、terminal `AllNotesOff` で終了。
  Undo/Redo・Save/Reload 後も Note stable ID と plan が
  決定的に一致し、VCS diff に MidiNote Added stable ID が
  現れる。
- [x] Recovery Review: partial stream の Hold（file byte
  同一・review record 永続・project revision 不変）→
  Adopt retry（verified prefix のみ Partial finalize・
  冪等）→ `commit_session` で Complete のみ commit・
  Partial は `partial-finalize` recoverable。commit 全体が
  1 Undo で戻り Redo で復元され、Save/Reload で entity・
  stable ID が維持される。corrupt journal / probe 無し
  Discard は fail-closed。
- [x] 2Window/Stale: stale-generation dispatch が receipt
  なしで drop・revision 不変、現行 generation は applied
  receipt。VCS history ops が project revision を進めない
  read-only であること（実 GUI の 2 Window は NotRun）。
- [x] 実 Window/実 OS GUI/実 device/実 OS FS/実 RT sample/
  AudioLens f64 は `NotRun`/`Unsupported` として文書・
  test-spec・PR に明記され、fake fixture の成功を実機
  Pass に昇格していない。
- [x] 既存 MD-A/B・crate 層 RCD-04 fixture の回帰なし
  （`mise run test` で workspace 全体 green）。

## Deferred Design Concerns

- 実 Window 2 面・実 OS GUI gesture・実 plugin を要する
  行 — reason: headless dispatch のみが本 slice の対象 |
  re-engagement: #114 系 GUI slice 着手時 | risk if
  ignored: GUI 固有の stale/gesture 差異を見逃す。
- MIDI comp selection の project 永続化 — reason: #126
  event model 未確定 | re-engagement: #126 完了後 |
  risk if ignored: plan 評価と永続 model の齟齬。
- 実 RT engine sample 一致・AudioLens f64 — reason:
  Ruviform #294 consumer gate 未統合 | re-engagement:
  consumer gate 実装後 | risk if ignored: offline/headless
  証拠を実音声品質の保証と誤認する。

## Sources and Relations

- Upstream: crate 層 fixture slice（`docs/specs/
  rcd-04-comp-transition-midi-phrase-fixture.md`、PR #219
  `9798be5b` で main 統合済み）、RCD-02 durability/
  recovery review spec、VCS-04 history/visual diff spec、
  SV-GUI-04 two-window slice spec、MD-A TakeGroup/Comp
  contract spec。
- 関連 Issue: #10/#11/#12（Take/Comp/editing 契約）、
  #42（MD-A）、#50（MD-B Effective Comp lowering）、
  #84（staged HEAD/Publish）、#95（VCS-04）、#114（GUI
  E2E）、#115/#116/#117（RCD-01/02/03）、#126（MIDI tick/
  chase fidelity・event model）、#127（TIM-03）。
