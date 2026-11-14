---
id: "TASK-6n3cvKDnkV96GBepu6YwTj"
type: "task"
status: "done"
title: "SYN-IO-02 Windows NTFS 実機 Case lane slice"
created: "2026-11-15"
updated: "2026-11-15"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/84"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-project/tests/real_fs_probe.rs"
      - type: "file"
        path: "docs/specs/syn-io-02-windows-real-fs.md"
      - type: "file"
        path: "docs/designs/syn-io-02-windows-real-fs.md"
      - type: "file"
        path: "docs/plans/syn-io-02-windows-real-fs.md"
      - type: "file"
        path: "docs/tasks/syn-io-02-windows-real-fs.md"
      - type: "file"
        path: "docs/test-specs/syn-io-02-windows-real-fs.md"
      - type: "file"
        path: "docs/impl/ir/syn-io-02-windows-real-fs.md"
      - type: "file"
        path: "evidence/reports/syn-io-02-windows-real-fs.md"
    modified:
      - type: "file"
        path: "mise.toml"
      - type: "file"
        path: "evidence/reports/syn-io-02-os-l1-evidence.md"
    deleted: []
    renamed: []
    moved: []
    generated:
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
  implements:
    - "docs/plans/syn-io-02-windows-real-fs.md"
  implemented-by: []
  depends-on:
    - "docs/plans/syn-io-02-windows-real-fs.md"
    - "docs/tasks/syn-io-02-head-cas-e2e-task.md"
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
    - "docs/test-specs/syn-io-02-windows-real-fs.md"
  references:
    - "crates/synvelis-project/tests/real_fs_probe.rs"
    - "mise.toml"
    - "evidence/reports/syn-io-02-windows-real-fs.md"
  defers: []
  deferred-by: []
---

# SYN-IO-02 Windows NTFS 実機 Case lane slice

## Work

`crates/synvelis-project/tests/real_fs_probe.rs` に
`SV84-REAL-01`（kill-mid-publish → reopen 不変条件・
5 trigger window）・`SV84-REAL-02`（crash 後 `save_cas`
retry 回復・stale `HEAD.lock` 実 break）・
`SV84-REAL-03`（Failed publish・CAS `Conflict` で
HEAD 非進行 + retry 回復）を `#[ignore]` で実装する。
`mise.toml` に `[tasks.test-project-real-fs]` を追加し、
実 Windows NTFS で `mise run test-project-real-fs` を
実走して `CASE=` 観測値を
`evidence/reports/syn-io-02-windows-real-fs.md` に記録、
`evidence/reports/syn-io-02-os-l1-evidence.md` 残ゲート
表の Windows NTFS 実機 Case 行を更新する。
power-loss・Indeterminate は `NotRun` 維持。
`src/` 差分ゼロ。

## Classification

- `normal-plan-task`

## Upstream Documents

- `docs/plans/syn-io-02-windows-real-fs.md`
- `docs/specs/syn-io-02-windows-real-fs.md`
- `docs/designs/syn-io-02-windows-real-fs.md`
- `docs/test-specs/syn-io-02-windows-real-fs.md`

## Dependencies

- `docs/tasks/syn-io-02-head-cas-e2e-task.md`
  （self re-exec + go-file barrier + marker protocol の
  先例 slice）
- main 統合済みの `DirectoryBundleStore`/
  `save_cas`/`HEAD.lock` 実装（PR #178・#342）

## Affected Files

- `crates/synvelis-project/tests/real_fs_probe.rs` —
  新規（`#[ignore]` ×3 + child protocol）
- `mise.toml` — `[tasks.test-project-real-fs]` 追加のみ
- `evidence/reports/syn-io-02-windows-real-fs.md` — 新規
- `evidence/reports/syn-io-02-os-l1-evidence.md` —
  残ゲート表の Windows NTFS 行のみ更新
- `docs/impl/ir/syn-io-02-windows-real-fs.md` — 新規
- `docs/*/README.md` — 管理 index 再生成

## Done When

- [x] `SV84-REAL-01/02/03` が独立 Case ID で pin され、
      kill-mid-publish → reopen で「旧 HEAD or 完結新
      HEAD + orphan 保存」・`save_cas` retry 回復・
      Failed/`Conflict` HEAD 非進行が実 FS で検証される
- [x] `mise run test-project-real-fs` が `#[ignore]`
      test のみを実行し、既定 `cargo test`/CI に
      混入しない
- [x] 実 Windows NTFS での実測値が evidence report に
      Case ID・Requirement×Result・SHA・環境付きで
      記録され、power-loss/Indeterminate は `NotRun`
      のまま残る
- [x] 残ゲート表の Windows NTFS 実機 Case 行が実測
      結果で更新される
- [x] `src/` 差分ゼロ・既存 test 回帰なし

## Verification

- [x] `mise run fmt-check` / `check synvelis-project` /
      `lint synvelis-project` / `test synvelis-project`
- [x] `SYNVELIS_REALFS_ROOT=<NTFS> mise run
      test-project-real-fs`（opt-in 実測・exit 0・
      3 passed）
- [x] `mise exec -- prek run --all-files`（commit hook
      として実施）
