---
id: "PLAN-034VmOlfg7SLp6iUHpXla5"
type: "plan"
status: "approved"
title: "RCD-04 product E2E plan"
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
    - "docs/designs/rcd-04-product-e2e-design.md"
  implemented-by: []
  depends-on:
    - "docs/designs/rcd-04-product-e2e-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/rcd-04-product-e2e.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/rcd-04-product-e2e.md"
  references: []
  defers: []
  deferred-by: []
---

# RCD-04 product E2E plan Implementation Plan

## Goal

`synvelis-app` に headless `syn.*` op dispatch 経路の product
E2E test（Comp Transition / MIDI Phrase / Recovery Review /
Stale+read-only）を 1 file・5 scenario で追加し、crate 層
fixture の domain 契約が製品 wire JSON 面で観測できることを
固定する。

## Scope

単一 stream・単一 task。変更は
`crates/synvelis-app/tests/rcd_04_product_e2e.rs`（新規）と
`docs/` チェーンのみ。`synvelis-project` は別 Worker 占有の
ため変更しない（fixture 追加が必要になれば ask で範囲拡張
を依頼）。実 GUI/device/OS/AudioLens は NotRun。

## Upstream Documents

- `docs/specs/rcd-04-product-e2e.md`
- `docs/designs/rcd-04-product-e2e-design.md`

## Design Inputs

- `docs/designs/rcd-04-product-e2e-design.md`
- 参照実装: `crates/synvelis-app/tests/vcs04_history.rs`
  （fake VCS provider + `set_vcs_backend` 注入）、
  `crates/synvelis-app/tests/ui_ops.rs`（`UiRuntime` +
  attach/dispatch/receipt/generation）、
  `crates/synvelis-recording/tests/rcd_04_recovery_partial.rs`
  （staging writer 構成）、
  `crates/synvelis-recording/tests/rcd_04_take_comp_pcm.rs`
  （`commit_session` 用 project 登録 mirror）、
  `crates/synvelis-project/tests/rcd_04_midi_phrase.rs`
  （phrase fixture 幾何・boundary oracle）。

## File Map

- `crates/synvelis-app/tests/rcd_04_product_e2e.rs` —
  harness（`SharedService`/`Fixture`/`call`/`ok`/
  `notifies`）+ domain builders（`seed_media_substrate`/
  `seed_verified_takes`/`build_comp_fixture`/
  `seed_commit_session`/MIDI builders/staging helpers/
  `FakeProvider`/`install_vcs`/`vcs_*` helpers）+ 5
  `#[test]` scenario。
- `docs/specs/rcd-04-product-e2e.md` — spec。
- `docs/designs/rcd-04-product-e2e-design.md` — design。
- `docs/plans/rcd-04-product-e2e-plan.md` — 本 plan。
- `docs/tasks/rcd-04-task1-product-e2e.md` — task。
- `docs/test-specs/rcd-04-product-e2e.md` — test-spec。
- `docs/impl/ir/rcd-04-product-e2e-slice.md` — impl record。

## Implementation Sequence

- [x] 1. `UiRuntime` + `AppService` 共有 harness と
  `syn.*` 呼び出し helper を実装（`SharedService` delegating
  `Service`、`call`/`ok`/`notifies`/`drain_all`）。
- [x] 2. Comp Transition scenario: substrate → verified
  takes → comp ops → `syn.recording.info` → CAS stale 拒否
  → undo/redo ×7 → save/reload → VCS diff stable ID。
- [x] 3. MIDI Phrase scenario: MIDI source/clip/note →
  `compile_midi_comp` boundary oracle → undo/redo plan
  決定性 → save/reload → VCS diff MidiNote stable ID。
- [x] 4. Recovery Review scenario: project 登録 session/
  stream mirror → staging complete+partial → scan →
  Hold/Adopt → `commit_session` → `syn.recording.info` →
  undo/redo → save/reload。
- [x] 5. Recovery guards scenario: probe 無し Discard
  fail-closed + corrupt journal fail-closed。
- [x] 6. Stale+read-only scenario: 2 view attach →
  stale-generation dispatch drop → current-gen receipt →
  VCS history ops revision 不変。
- [x] 7. docs チェーン 6 file を実内容で populate。
- [x] 8. `rustfmt` + `cargo clippy -p synvelis-app
  --all-targets` で lint 適合を確認。

## Dependencies and Risks

- `UiRuntime` が登録 service を返さない → `Rc<RefCell>`
  共有 + delegating `Service` で解決（design 記載）。
- `commit_session` の project registry 解決
  （`capture_stream_id`/`lane_id`/`session_id`）→
  `seed_commit_session` で project 発行 tagged id を
  staging manifest に mirror。
- VCS fake asset digest により committed-vs-live diff の
  `overall` が `unknown` になり得る → entity-level
  `changes`/`summary.total` を主張（MIDI は asset 非依存で
  `equal` 確認可能）。
- `synvelis-project` は別 Worker 占有 → 変更しない。
  fixture 追加が必要なら ask。

## Task Breakdown

- `docs/tasks/rcd-04-task1-product-e2e.md`（本 slice 全体
  を 1 task として扱う）。

## Verification

- [x] `cargo test -p synvelis-app --test
  rcd_04_product_e2e` — 5 tests pass（補助証拠）。
- [x] `cargo clippy -p synvelis-app --all-targets` —
  warning 0（補助証拠）。
- [ ] `mise run fmt-check` — canonical。
- [ ] `mise run check` — canonical。
- [ ] `mise run lint` — canonical。
- [ ] `mise run test` — canonical（workspace 全体回帰）。
- [ ] `mise run test-rt` — canonical。
- [ ] `mise run test-msrv` — canonical。
- [ ] `mise run check-app-native` — canonical。
- [ ] `mise run lint-app-native` — canonical。

## Open Questions / Gaps

- 実 Window 2 面・実 OS GUI・実 device・実 OS FS・実 RT
  sample・AudioLens f64 — 本 slice では NotRun。
  #114 系 GUI slice / Ruviform #294 consumer gate で
  後続検証。
- MIDI comp 永続化 — #126 event model 確定後。

## Self-Review

- [x] Every spec requirement maps to a task.
- [x] No placeholders remain.
- [x] File paths, relations, status values, and terminology
  are consistent.

## Implementation Handoff

- Recommended mode: inline execution（単一 test file・
  依存直列）。
- Independent work streams: none。
- User approval required before dispatch: yes —
  review-protocol review を承認として扱う。

## Review Handoff

- `SharedService` の `Rc<RefCell>` 共有が `Service::call`
  の `&mut self` を正しく満たすか。
- fixture 構築（`app_mut()` canonical apply）と検証
  （`syn.*` dispatch）の境界が test-spec の主張と一致
  するか。
- VCS `overall: unknown` の扱い（tri-state 正直性）が
  妥当か。
