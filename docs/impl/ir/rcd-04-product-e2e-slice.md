---
id: "IMPL-034VnGKTfvMA41aVr0ctLN"
type: "impl"
status: "in-progress"
title: "RCD-04 Comp Transition / MIDI Phrase / Recovery Review product E2E app slice 実装記録"
created: "2026-11-10"
updated: "2026-11-10"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/118"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-app/tests/rcd_04_product_e2e.rs"
      - type: "file"
        path: "docs/specs/rcd-04-product-e2e.md"
      - type: "file"
        path: "docs/designs/rcd-04-product-e2e-design.md"
      - type: "file"
        path: "docs/plans/rcd-04-product-e2e-plan.md"
      - type: "file"
        path: "docs/tasks/rcd-04-task1-product-e2e.md"
      - type: "file"
        path: "docs/test-specs/rcd-04-product-e2e.md"
      - type: "file"
        path: "docs/impl/ir/rcd-04-product-e2e-slice.md"
    modified:
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
    - "docs/tasks/rcd-04-task1-product-e2e.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/ir/rcd-04-comp-transition-midi-phrase-crate-fixture-slice.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers:
    - "実 Window/OS GUI・実 device・実 plugin を要する product E2E（#114 系 GUI slice）"
    - "実 RT engine sample 一致・AudioLens native-f64 consumer gate（Ruviform #294）"
    - "MIDI comp selection の project 永続化 model（synvelis#126）"
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# RCD-04 Comp Transition / MIDI Phrase / Recovery Review product E2E app slice 実装記録

## Summary

`synvelis-app` に headless `syn.*` op dispatch 経路の product
E2E test `crates/synvelis-app/tests/rcd_04_product_e2e.rs`
（5 scenario）を追加した。`UiRuntime` + `AppService`
共有 harness で `UiRuntime::dispatch_op` → `Service::call`
→ `AppService` → `SynvelisApp` の実 dispatch を駆動し、
Comp Transition / MIDI Phrase / Recovery Review /
Recovery guards / Stale+read-only を wire JSON 面で検証
する。crate 層 fixture slice（PR #219）の domain 契約を
再利用し、第二の project/recording/comp モデルは作らない。
実 Window/OS GUI/device/OS FS/実 RT sample/AudioLens
f64 は NotRun/Unsupported として記録する。

## Context

task `rcd-04-task1-product-e2e` / plan
`rcd-04-product-e2e-plan` / spec `rcd-04-product-e2e` /
design `rcd-04-product-e2e-design` に従う。Issue #118
（RCD-04）の残件である app 層 product E2E を実装する
slice。crate 層 fixture は main 統合済み（PR #219
`9798be5b`）で、本 slice は `synvelis-app` test と docs
チェーンに閉じる。`synvelis-project` は別 Worker 占有の
ため変更していない。

## Implementation

### Changes

- `crates/synvelis-app/tests/rcd_04_product_e2e.rs`（新規、
  約 1700 行）:
  - harness: `SharedService`（`Rc<RefCell<AppService>>`
    への delegating `Service` impl）で `UiRuntime` に
    `AppService` を再登録し、`call`/`ok`/`notifies`/
    `drain_all` helper で `syn.*` dispatch と outbox
    notify を観測。
  - fixture 構築: `with_live`/`apply_synvelis`/
    `apply_project`/`apply_recording`/`apply_comp` が
    `AppService::app_mut()` → `workspace_mut().
    session_mut().apply(SynvelisOp, rev)` の canonical
    mutation 経路を使う（undo/redo/CAS 対象の同一経路）。
  - domain builders: `seed_media_substrate`（track/lane/
    epoch/asset/committed source）・`seed_verified_takes`
    （finalized session + verified take）・
    `build_comp_fixture`（TakeGroup/Comp/Region/
    ActiveComp/Transition）・`seed_commit_session`
    （Closed session + lane 毎 stream 登録、
    `commit_session` の registry 解決用）・MIDI
    builders（source/clip/note/selection/controller
    lanes）・staging helpers（`StagingArea` + 手組み
    `StreamWriter` + `build_handoff`、
    `rcd_04_recovery_partial` と同構成）。
  - `FakeProvider`（`VcsReadProvider` + `VcsBytesPort`）+
    `install_vcs` で `VcsHistoryAdapter` を
    `set_vcs_backend` 注入（`vcs04_history.rs` と同型、
    committed pre/post + live endpoint）。
  - 5 `#[test]` scenario（下記 Decisions 参照）。
- docs チェーン 6 file を実内容で populate
  （spec/design/plan/task/test-spec/impl record）。

### Decisions During Implementation

- **dispatch 経路と fixture 構築の分離**: `UiRuntime` は
  登録済み service を呼び出し側へ返さないため、
  `AppService` を `Rc<RefCell>` で共有し delegating
  `Service` impl 経由で再登録した。`syn.*` 呼び出しは
  常に実 runtime の文字列 dispatch を通り、fixture
  構築のみ `app_mut()` の canonical apply を使う。
  これにより「検証対象 = `syn.*` dispatch」「構築 =
  canonical mutation 経路」の境界が明確になった。
- **Comp Transition**: substrate（committed source +
  finalized session の verified take ×2）→ comp ops
  7 個 → `syn.recording.info` 投影 → stale CAS
  （`ProjectRevision(rev+1000)` → `StaleRevision`、
  comp registry 不変）→ `syn.project.undo`/`redo` ×7
  で stable ID 維持・recording snapshot 不変 →
  `syn.project.save`/`reload` → VCS diff で
  TakeGroup/Comp/CompRegion/CompTransition の Added
  stable ID、committed(post)→live で entity-level
  変更 0 件。
- **MIDI Phrase**: `compile_midi_comp` の plan boundary
  oracle（NoteOn/NoteOff 1:1・active 空・terminal
  `AllNotesOff`・virtual offs・chase 非空）を live
  project 参照で評価。undo/redo・save/reload 後に
  NoteId・plan が決定的に一致。VCS diff に MidiNote
  Added 7 件の stable ID、committed(post)→live
  `equal`（asset 非依存のため真の equal）。comp
  registry が空のままであることを確認（MIDI comp
  永続化は #126 deferred）。
- **Recovery Review**: `seed_commit_session` で project
  発行の `RecordingSessionId`/`CaptureStreamId`/
  `LaneId` tagged id を staging manifest に mirror
  （`commit_session` が registry 解決するため）。
  stream0 Complete / stream1 crashed partial →
  `recovery::scan` → Hold（media/journal byte 同一・
  `Held` record・revision 不変）→ Adopt retry
  （`Partial` finalize・verified prefix のみ・冪等）→
  `commit_session` Applied・takes=1・clips=1・
  recoverable=[stream1 `partial-finalize`] →
  `syn.recording.info` 投影 → undo/redo・save/reload。
- **Recovery guards**: probe 無し Discard が `Refused`
  で media 保持・revision 不変、corrupt journal で
  scan が `Finalized` を報告せず review action が
  fail-closed。
- **Stale+read-only**: `syn.view.attach` で 2 view を
  別 window に attach → generation 0 dispatch が
  receipt 無しで drop・revision 不変 → generation 1
  が applied receipt → VCS history ops 列で revision
  不変・comp entity 残存。
- **VCS `overall` の扱い**: fake asset digest により
  committed-vs-live diff の `overall` が `unknown`
  （tri-state）を返し得るため、entity-level
  `changes`/`summary.total` を主張し `overall` は
  `different` でないことのみ確認した（R7）。MIDI
  fixture は asset 非依存のため `equal` を確認できた。
- **lint**: workspace `clippy::pedantic` + `-D
  warnings` に適合。`too_many_lines` は sequential
  e2e scenario の既存慣行（`recording-poc` 等）どおり
  justification 付き `#[allow]`、`doc_markdown` は
  backtick で解消、`needless_pass_by_value` は
  `&Value` 化で解消。

## Related Experiments

### Adopted

- なし（experiment log は作成していない）。

### Rejected

- なし。

## Validation

- `cargo test -p synvelis-app --test
  rcd_04_product_e2e` — 5 tests pass（補助証拠:
  `mise exec -- cargo test`）。
- canonical mise 検証（Windows 11 x64 / Git Bash、
  toolchain rust 1.99.0 pin、merge `origin/main`
  @ `9a75336` 取込後）— すべて exit 0:
  - `mise run fmt-check` — pass
  - `mise run check` — pass
  - `mise run lint` — pass（`-D warnings`）
  - `mise run test` — pass（workspace 全件。
    初回実行で `synvelis-library` webdav test が
    os error 10053 で 5 件 flake → 単独再実行 42/42
    pass・全体再実行も pass。RCD-04 5 tests 含む）
  - `mise run test-rt` — pass
  - `mise run test-msrv` — pass（rust 1.89.0、
    workspace 全件）
  - `mise run check-app-native` — pass
  - `mise run lint-app-native` — pass

## Risks

- `SharedService` の `Rc<RefCell>` 共有は `Service::
  call` の `&mut self` を call 単位の短い borrow で
  満たす。長時間 borrow すると dispatch 中の
  `app_mut()` と競合し得るが、test は call 完了後に
  のみ fixture 変異を行うため問題ない。
- fixture 構築（`app_mut()` canonical apply）と検証
  （`syn.*` dispatch）の境界は test-spec R1 で明示
  したが、「test 専用経路」と誤認される余地は残る。
- 実 Window/OS GUI/device/OS FS/実 RT sample/
  AudioLens f64 は未検証（NotRun/Unsupported）。
  fake fixture の成功を実機 Pass に昇格していない。

## Follow-ups

- 実 Window 2 面・実 OS GUI gesture・実 plugin を要する
  行 — #114 系 GUI slice で再検証。
- 実 RT engine sample 一致・AudioLens native-f64 —
  Ruviform #294 consumer gate 待ち。
- MIDI comp selection の project 永続化 — synvelis#126
  event model 確定後。
- 実 Git provider の検出/binding — fake provider 注入
  のみ。実 provider は後続 slice。
