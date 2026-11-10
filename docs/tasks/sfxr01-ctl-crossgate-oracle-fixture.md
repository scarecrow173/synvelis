---
id: "TASK-G4oEdDOSfbQ4uy5d0HrbKh"
type: "task"
status: "in-progress"
title: "SF-XR-01 — CTL-01/CTL-02 cross-gate oracle fixture 実装 task"
created: "2026-11-10"
updated: "2026-11-10"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/109"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/sfxr01-ctl-crossgate-oracle-fixture.md"
  implemented-by: []
  depends-on:
    - "docs/plans/sfxr01-ctl-crossgate-oracle-fixture.md"
    - "docs/test-specs/sfxr01-ctl-crossgate-oracle-fixture.md"
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
    - "docs/test-specs/sfxr01-ctl-crossgate-oracle-fixture.md"
  references:
    - "docs/plans/sfxr01-ctl-crossgate-oracle-fixture.md"
    - "docs/test-specs/sfxr01-ctl-crossgate-oracle-fixture.md"
  defers: []
  deferred-by: []
---

# SF-XR-01 — CTL-01/CTL-02 cross-gate oracle fixture 実装 task

## Tasks

- [x] 既存 coverage 棚卸し（ctl_01/ctl_02/sf_02/sf06_oracle/
      sf03_l1）と重複回避の確定
- [x] `crates/synvelis-player/tests/sf_xr_01.rs` 新規作成
      （`sfxr01_01_*`・`sfxr01_02_*` の 2 test）
- [x] focused test green・fmt/clippy clean
- [x] `evidence/reports/sf-06-cross-gate-ledger.md` SF-XR-01 行
      更新 + headless evidence row 追加
- [x] docs チェーン作成（spec/adr/design/plan/task/test-spec/impl-ir）
- [ ] `git fetch origin main && git merge origin/main` 後の
      `mise run fmt-check` / `check` / `lint` / `test` 最終検証
- [ ] push + PR 作成（`Refs #109`）

## Notes

- fixture commit: `43511f0`（merge commit — test file は
  `af1cac1` で追加、review 修正の tautology 除去を merge に fold）。
- src 変更なし・他 crate 非接触を diff で確認済み。
