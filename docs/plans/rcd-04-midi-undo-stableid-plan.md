---
id: "PLAN-034WEVk4u3tDdRv1lWmQgK"
type: "plan"
status: "approved"
title: "RCD-04 MIDI phrase boundary / Undo-Redo / VCS stable-ID implementation plan"
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
    - "docs/designs/rcd-04-midi-undo-stableid-design.md"
  implemented-by: []
  depends-on:
    - "docs/plans/rcd-04-comp-midi-phrase-fixture.md"
    - "docs/plans/rcd-04-rt-engine-comp-pcm-plan.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/rcd-04-midi-undo-stableid.md"
    - "docs/designs/rcd-04-midi-undo-stableid-design.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/rcd-04-midi-undo-stableid.md"
  references: []
  defers: []
  deferred-by: []
---

# RCD-04 MIDI phrase boundary / Undo-Redo / VCS stable-ID plan

## Upstream Documents

- `docs/specs/rcd-04-midi-undo-stableid.md`

## Design Inputs

- `docs/designs/rcd-04-midi-undo-stableid-design.md`（approved）

## File Map

- `crates/synvelis-player/tests/rcd_04_midi_phrase_boundary.rs` —
  新規。2 region 連接 MIDI comp の境界 fixture（wire 列完全一致 +
  HeadlessDriver 配送 ledger 不変条件）。
- `crates/synvelis-project/tests/rcd_04_comp_cas.rs` — 既存へ
  comp 切替の単一 CAS/Undo/Redo + raw 不変テストを追加。
- `crates/synvelis-project/tests/rcd_04_vcs_stable_id.rs` —
  新規。save/saved-snapshot/diff/restore/apply 横断の comp
  stable-ID fixture。
- `docs/specs|designs|plans|tasks|test-specs|impl/ir/
  rcd-04-midi-undo-stableid*` — doc-chain 6 文書 + 管理 index。

## Implementation Sequence

- [x] 既存カバレッジ差分の棚卸し（PR#219/#300/#319・TIM-02・
  comp_cas・VCS harness）— 未カバー経路を確定
- [x] `rcd_04_midi_phrase_boundary.rs` 作成 → 期待 wire 列の
  手導出と `compile_events`/`dispatch_midi` 配送検証
- [x] `rcd_04_comp_cas.rs` に comp 切替テスト追加
- [x] `rcd_04_vcs_stable_id.rs` 作成（save/diff/apply/restore 4
  テスト）
- [x] 対象テストの実行・修正（player 2 tests / project 10 tests
  全 pass）
- [x] `git fetch origin main && git merge origin/main` —
  Already up to date（origin/main 取り込み済み）
- [x] canonical 9 lane（fmt-check/check/lint/test/test-rt/
  check-app-native/lint-app-native/test-msrv/build）全 pass
- [ ] コミット・push・PR 作成（`Refs #118`）

## Dependencies and Risks

- 依存: 既存 `tim02_midi_playback.rs` fixture 群・
  `rcd_04_midi_phrase.rs` の canonical selection・VCS test
  harness（`AllPresentProbe`/`MapBlobs`/`src::comp` 型）。
- リスク: 期待 wire 列の手導出ミス — project 側
  `rcd_04_midi_phrase.rs` が plan 契約を独立 pin しているため
  wire 層 drift として検出される構造。
- リスク: `undo()` が新 revision を発行する既存契約と混同 —
  revision 番号ではなく「+1 step・内容完全一致」で pin する。

## Task Breakdown

- 本 plan は単一 task
  （`docs/tasks/rcd-04-task-midi-undo-stableid.md`）に対応。
  順序は fixture → 検証の線形で分割不要。

## Verification

- [x] `cargo test -p synvelis-project --test rcd_04_comp_cas`
  （6 passed）
- [x] `cargo test -p synvelis-project --test rcd_04_vcs_stable_id`
  （4 passed）
- [x] `cargo test -p synvelis-player --test
  rcd_04_midi_phrase_boundary`（2 passed）
- [x] canonical 9 lane（`mise run fmt-check` / `check` / `lint`
  / `test` / `test-rt` / `check-app-native` / `lint-app-native`
  / `test-msrv` / `build`）— 全 pass（Windows、rust
  1.99.0系 toolchain、test-msrv は rust 1.89.0）
- [x] docs index 再生成と `index plan` in-sync 確認
  （6 dir すべて）

## Open Questions / Gaps

- なし。comp kind の apply surface 拡張は別 slice の明示的
  deferred。

## Self-Review

- [x] Every spec requirement maps to a task.
- [x] No placeholders remain.
- [x] File paths, relations, status values, and terminology are consistent.

## Implementation Handoff

- Recommended mode: inline execution（worker dispatch）
- Independent work streams: none
- User approval required before dispatch: yes（review-protocol
  の blocker なしを承認として扱う）
- If approved: 既に inline 実行済み。

## Review Handoff

- レビュー観点: 期待 wire 列の手導出が plan 契約（
  `midi_comp.rs`）と矛盾しないか、undo の revision 契約の
  解釈が正しいか、apply fail-closed が現行契約の固定であり
  挙動変更でないか。
