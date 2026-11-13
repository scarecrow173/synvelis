---
id: "TASK-034WEVr538punRWOxqkfaf"
type: "task"
status: "in-progress"
title: "RCD-04 MIDI phrase boundary / Undo-Redo / VCS stable-ID slice"
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
    - "docs/plans/rcd-04-midi-undo-stableid-plan.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/rcd-04-midi-undo-stableid.md"
  references: []
  defers: []
  deferred-by: []
---

# RCD-04 MIDI phrase boundary / Undo-Redo / VCS stable-ID slice

## Goal

Issue #118 残件のうち headless 検証可能な部分 — MIDI phrase
境界・comp 切替 Undo/Redo・VCS stable-ID — を test fixture
で pin する。test-only slice で src 変更は契約違反時のみ。

## Upstream

- `docs/plans/rcd-04-midi-undo-stableid-plan.md`
- `docs/specs/rcd-04-midi-undo-stableid.md`
- `docs/test-specs/rcd-04-midi-undo-stableid.md`

## Dependencies

- 既存 fixture: `tim02_midi_playback.rs`、`rcd_04_midi_phrase.rs`
  （canonical selection）、`rcd_04_comp_cas.rs`、VCS harness
  （`vcs02_diff`/`vcs05_apply`/`vcs05_restore`）。

## Affected Files

- `crates/synvelis-player/tests/rcd_04_midi_phrase_boundary.rs`（新規）
- `crates/synvelis-project/tests/rcd_04_comp_cas.rs`（拡張）
- `crates/synvelis-project/tests/rcd_04_vcs_stable_id.rs`（新規）
- `docs/{specs,designs,plans,tasks,test-specs,impl/ir}/
  rcd-04-midi-undo-stableid*`（新規）

## Done When

- [x] MIDI phrase 境界: 2 region 連接 selection の wire 列・
  HeadlessDriver 配送・ledger 不変条件を pin（2 tests pass）
- [x] comp 切替: 単一 CAS/Undo/Redo + raw 不変を pin
  （comp_cas 6 tests pass）
- [x] VCS stable-ID: save/diff/restore/apply fail-closed を pin
  （4 tests pass）
- [x] doc-chain 6 文書 + 管理 index 再生成
- [x] `git fetch origin main && git merge origin/main` —
  Already up to date
- [x] canonical 9 lane 全 pass
- [ ] commit・push・PR（`Refs #118`）作成

## Verification

- [x] `cargo test -p synvelis-player --test
  rcd_04_midi_phrase_boundary` — 2 passed
- [x] `cargo test -p synvelis-project --test rcd_04_comp_cas` —
  6 passed
- [x] `cargo test -p synvelis-project --test
  rcd_04_vcs_stable_id` — 4 passed
- [x] `mise run fmt-check` / `check` / `lint` / `test` /
  `test-rt` / `check-app-native` / `lint-app-native` /
  `test-msrv` / `build` — 全 exit 0
