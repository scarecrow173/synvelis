---
id: "DESIGN-034VmNZvp2r11R0Phmdf0P"
type: "design"
status: "approved"
title: "RCD-04 product E2E design"
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
    - "docs/specs/rcd-04-product-e2e.md"
  implemented-by: []
  depends-on:
    - "docs/designs/rcd-04-comp-transition-midi-phrase-fixture.md"
    - "docs/designs/rcd-02-durability-evidence-recovery-review.md"
    - "docs/designs/vcs-04-history-visual-diff-ui-design.md"
    - "docs/designs/sv-gui-04-01-two-window-slice1-design.md"
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
    - "実 Window/OS GUI・実 device・実 plugin を要する product E2E（#114 系 GUI slice）"
    - "実 RT engine sample 一致・AudioLens native-f64 consumer gate（Ruviform #294）"
    - "MIDI comp selection の project 永続化 model（synvelis#126 event model 確定後）"
  deferred-by: []
---

# RCD-04 product E2E design

## Context

crate 層 fixture slice（PR #219）は `synvelis-project` /
`synvelis-recording` 内で Comp CAS・MIDI phrase 境界・
Take/Comp PCM・Recovery partial/cancel/retry の domain
契約を固定済み。本 slice が埋める隙間は app 層:
`synvelis-app` が公開する `syn.*` op 面
（`UiRuntime::dispatch_op` → `Service::call` →
`AppService` → `SynvelisApp`）を通したときに、同じ契約が
製品の wire JSON として観測できるか、である。

設計上の核心は「検証対象の dispatch 経路」と「fixture
構築経路」の分離である。`UiRuntime` は登録済み
`Service` を呼び出し側へ返さないため、test は
`AppService` を `Rc<RefCell>` で共有し、delegating
`Service` impl 経由で `UiRuntime` に再登録する。
これにより (a) `syn.*` 呼び出しは常に実 runtime の
文字列 dispatch を通り、(b) fixture 構築だけが
`app_mut()` → `ProjectSession::apply(SynvelisOp,
expected_revision)` の canonical mutation 経路を直接
使える。canonical 経路は undo/redo stack・revision CAS・
engine/control notify を含むため、fixture で作った状態は
製品 op で作った状態と区別がつかない。

## Scope

### In Scope

- `crates/synvelis-app/tests/rcd_04_product_e2e.rs` 単一
  test file・5 scenario（Comp Transition / MIDI Phrase /
  Recovery Review / Recovery guards / Stale+read-only）。
- headless `UiRuntime` + `AppService` 共有 harness、
  `syn.*` op 呼び出し helper、fake VCS provider
  （`VcsHistoryAdapter` 注入、`vcs04_history.rs` と同型）。
- staging file fixture（`StagingArea` + 手組み
  `StreamWriter`、`rcd_04_recovery_partial.rs` と同構成）。
- `docs/` チェーン 6 file。

### Out of Scope

- `synvelis-project`/`synvelis-recording`/その他 crate・
  `ui/`・workflow・mise 設定の変更。
- 実 Window/OS GUI・実 device・実 plugin・実 OS FS・
  実 RT engine・AudioLens f64 の実行と Pass 主張
  （NotRun/Unsupported）。
- MIDI comp 永続化（#126）・RT vs Offline PCM 差分の
  本格 golden（crate 層 fixture が担う）。

## Components and Boundaries

- `UiRuntime`（ruviform-ui）: intent → `syn.*` op 文字列
  dispatch の唯一の入口。test は `dispatch_op` のみを
  使い、service 内部へは触れない。
- `SharedService`（test 内 delegating `Service` impl）:
  `Rc<RefCell<AppService>>` への薄い転送。`ops()`/`call()`
  をそのまま委譲し、製品側の op 表・観測 counter・
  error logging を迂回しない。
- `AppService`/`SynvelisApp`: `syn.project.new/save/
  reload/undo/redo`、`syn.recording.info`、`syn.vcs.view/
  visual`、`syn.view.attach`、`syn.control.dispatch` の
  実装本体。fixture 構築は `app_mut()` が返す同一 app の
  `workspace_mut().session_mut()` に対して行う。
- `ProjectSession::apply/undo/redo`（synvelis-workspace
  経由 project controller）: fixture op・stale CAS・
  undo/redo の canonical mutation 経路。
- `FakeProvider`（test 内 `VcsReadProvider +
  VcsBytesPort`）: 2 commit（pre/post）+ live endpoint
  を供給し、`VcsHistoryAdapter` 経由で `syn.vcs.*` が
  解決する snapshot bytes を差し替える。実 Git provider
  の検出・binding は後続 slice。
- `StagingArea`/`StreamWriter`/`review_stream`/
  `commit_session`（synvelis-recording）: Recovery
  fixture の file 面。crate 層 fixture と同じ writer
  構成を使い、project 側は `RecordingOp` で
  session/stream を事前登録して `commit_session` の
  registry 解決を満たす。

## Data and Control Flow

1. `fixture()`: `SynvelisApp::new` → `AppService` →
   `Rc<RefCell>` → `SharedService` → `UiRuntime::
   register_service`。`event_outbox` を test が保持。
2. `syn.project.new` で bundle を tmp dir に作成
   （実 `syn.*` dispatch）。
3. substrate 構築: `seed_media_substrate`（track/lane/
   epoch/asset/source）→ `seed_verified_takes`（session/
   pass/stream/segment/take を `RecordingOp` で登録・
   finalize）→ `build_comp_fixture`（TakeGroup/Comp/
   Region/ActiveComp/Transition を `CompOp` で登録）。
   全て `session.apply(SynvelisOp, rev)` 経由。
4. 検証: `syn.recording.info`・`syn.project.undo/redo`・
   `syn.project.save/reload`・stale `session.apply`
   （`ProjectRevision(rev+1000)` → `StaleRevision`）・
   `install_vcs` + `syn.vcs.visual/view` で diff 観測。
5. MIDI: `AddMidiSource`/`AddClip`/`AddMidiNote` を
   `ProjectOp` で構築 → `compile_midi_comp`（live project
   参照）で plan を評価 → undo/redo/save/reload 後に
   plan・NoteId を再評価して一致を確認 → VCS diff で
   MidiNote Added stable ID を確認。
6. Recovery: `seed_commit_session`（Closed session +
   lane 毎 stream 登録）→ staging manifest が project
   発行の tagged id を mirror → stream0 Complete /
   stream1 crashed partial → `recovery::scan` →
   `review_stream` Hold/Adopt → `commit_session` →
   `syn.recording.info`・undo/redo・save/reload。
7. Stale/2Window: `syn.view.attach` で 2 view を別
   window に attach → generation 0 の
   `syn.control.dispatch`（drop・receipt 無し）→
   generation 1（applied receipt）→ VCS history ops
   列で revision 不変を確認。

## Non-Functional Constraints

- 決定性: tmp dir は pid+case 名で分離、VCS fixture は
  固定 oid・固定 expr、MIDI fixture は固定 tick・
  division（480→960 PPQN で native×2 の厳密写像）。
  wall-clock・乱数・実 device に依存しない。
- 正直性: fake/mock fixture の成功を実機 Pass に
  昇格しない。test file 冒頭と各 scenario に
  NotRun/Unsupported の境界を明記する。
- 非干渉: test は `synvelis-app` の test target のみ。
  workspace 他 crate・`ui/`・生成物に変更を加えない。
- lint: workspace `clippy::pedantic` + `-D warnings`
  に適合（`too_many_lines` は sequential scenario の
  既存慣行どおり justification 付き `#[allow]`、
  `doc_markdown` は backtick で解消）。

## Risks and Trade-offs

- `UiRuntime` が service を返さない → `Rc<RefCell>` 共有
  + delegating `Service` で解決。`Service::call` の
  `&mut self` を `RefCell` で満たし、 borrow は call
  単位に限定する。
- fixture 変異が「test 専用経路」に見える risk →
  `ProjectSession::apply` は製品 undo/redo/CAS が通る
  同一経路であり、`syn.*` 検証対象とは別層（構築 vs
  検証）として test-spec に明記する。
- VCS committed endpoint の snapshot decode で asset
  content が解決できない（fake digest）→ diff `overall`
  は `unknown`（tri-state）を返し得る。entity-level
  の `changes`/`summary.total` を主張し、`overall` は
  `different` でないことのみを確認する（MIDI fixture
  は asset 非依存のため `equal` を確認できる）。
- `commit_session` は project registry の
  `capture_stream_id`/`lane_id`/`session_id` 解決を
  要求 → `seed_commit_session` で project 発行 id を
  staging manifest に mirror する（take_comp_pcm と
  同型）。

## Deferred Design Concerns

- 実 Window 2 面・実 OS GUI gesture・実 plugin —
  headless dispatch のみが本 slice。#114 系 GUI slice
  で再検証。
- 実 RT engine sample 一致・AudioLens native-f64 —
  Ruviform #294 consumer gate 待ち。
- MIDI comp 永続化 — synvelis#126 event model 確定後。

## Verification Notes

- [x] `cargo test -p synvelis-app --test
  rcd_04_product_e2e`（補助: 5 tests pass）
- [ ] `mise run fmt-check` / `check` / `lint` / `test` /
  `test-rt` / `test-msrv` / `check-app-native` /
  `lint-app-native`（canonical 検証として実施）
- [x] `cargo clippy -p synvelis-app --all-targets`
  （補助: warning 0）
