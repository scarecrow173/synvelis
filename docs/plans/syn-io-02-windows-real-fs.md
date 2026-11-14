---
id: "PLAN-1dCdxMBaLJixFXCNXKmkJL"
type: "plan"
status: "approved"
title: "SYN-IO-02 Windows NTFS 実機 Case lane — implementation plan"
created: "2026-11-15"
updated: "2026-11-15"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/84"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/syn-io-02-windows-real-fs.md"
  implemented-by:
    - "docs/tasks/syn-io-02-windows-real-fs.md"
  depends-on:
    - "docs/designs/syn-io-02-windows-real-fs.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/plans/syn-io-02-directory-bundle-store-stagedfile.md"
    - "docs/plans/syn-io-02-head-cas-plan.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/syn-io-02-windows-real-fs.md"
    - "docs/designs/syn-io-02-windows-real-fs.md"
  derived-by: []
  verifies:
    - "docs/test-specs/syn-io-02-windows-real-fs.md"
  verified-by:
    - "docs/test-specs/syn-io-02-windows-real-fs.md"
  references:
    - "crates/synvelis-project/tests/real_fs_probe.rs"
    - "mise.toml"
  defers: []
  deferred-by: []
---

# SYN-IO-02 Windows NTFS 実機 Case lane — implementation plan

## Goal

Issue #84 残件 slice を 1 task で実装する。

1. `real_fs_probe.rs` に `SV84-REAL-01/02/03` を pin する
   `#[ignore]` opt-in lane を追加し、`mise run
   test-project-real-fs` で実 Windows NTFS 上の
   kill→reopen→retry/Failed を実測する。
2. 実測値を `syn-io-02-windows-real-fs.md` に記録し、
   `syn-io-02-os-l1-evidence.md` 残ゲート表の Windows
   NTFS 実機 Case 行を更新する。power-loss/
   Indeterminate は `NotRun` のまま残す。

## Scope

In scope は design File Map と同一。`src/`・依存・
workflow・他 crate・他 slug docs は変更しない。
`mise.toml` は task 追加のみ。

## Upstream Documents

- `docs/specs/syn-io-02-windows-real-fs.md`

## Design Inputs

- `docs/designs/syn-io-02-windows-real-fs.md`（child
  protocol・trigger・Case 構成・volume pin）
- `docs/designs/syn-io-02-head-cas-design.md` と
  `docs/designs/syn-io-02-directory-bundle-store-
  stagedfile-design.md`（store 契約の正本）

## File Map

| File | Change |
| --- | --- |
| `crates/synvelis-project/tests/real_fs_probe.rs` | 新規 — `#[ignore]` ×3 + child protocol |
| `mise.toml` | `[tasks.test-project-real-fs]` 追加 |
| `evidence/reports/syn-io-02-windows-real-fs.md` | 新規 — 実測記録 |
| `evidence/reports/syn-io-02-os-l1-evidence.md` | 残ゲート表の該当行のみ更新 |
| `docs/*` + `docs/*/README.md` | doc chain + index 再生成 |

## Implementation Sequence

- [x] `real_fs_probe.rs` を実装（child protocol・
      trigger・kill_round・3 Case）し、
      `cargo check`/`lint`/`fmt` で静的検証する
      （補助証拠）。
- [x] `mise.toml` に `[tasks.test-project-real-fs]` を
      追加し、`SYNVELIS_REALFS_ROOT` を NTFS temp 配下に
      固定して `mise run test-project-real-fs` を実走、
      `CASE=` 行を採取する。
- [x] evidence report を実測値で作成し、残ゲート表の
      Windows NTFS 行を更新する。
- [x] doc chain 6 file を完成させ、`doc_maintenance.js
      index apply` で index を再生成する。
- [x] review-protocol で chain + diff をレビューし、
      blocker が残っていないことを確認する。
- [x] canonical 検証（`mise run fmt-check`/`check`/
      `lint`/`test` synvelis-project + `prek`）を実行し
      impl-ir に記録する。

## Dependencies and Risks

- 依存: `save`/`save_cas`/`head`/`read_snapshot`/
  `snapshot_name`（pub API）、`HEAD.lock` stale/busy
  定数、ruviform-io-file temp 命名 — 全て main 統合済み
  で本 branch base に含まれる。
- `current_exe()` self re-exec は libtest `--exact` +
  `--ignored` filter に依存 — `head_cas.rs` 先例あり。
- go-file barrier は best-effort — kill window の成否は
  trigger 観測で判定し、missed window は fresh bundle
  retry（design Risk Register と同一）。
- design Risk Register と同一。

## Task Breakdown

- `docs/tasks/syn-io-02-windows-real-fs.md` — 本 slice
  全体の単一 task。

## Verification

- [x] `mise run fmt-check` — exit 0。
- [x] `mise run check synvelis-project` — exit 0。
- [x] `mise run lint synvelis-project` — exit 0
      （clippy `-D warnings`、pedantic 含む）。
- [x] `mise run test synvelis-project` — exit 0
      （`#[ignore]` probe は 0 selected で非搭載を確認）。
- [x] `SYNVELIS_REALFS_ROOT=<NTFS temp> mise run
      test-project-real-fs` — exit 0、3 passed
      （実 Windows 11/NTFS・rustc 1.99.1・20.93s）。
- [x] `mise exec -- prek run --all-files` — exit 0
      （結果を impl-ir/PR に記録）。

## Open Questions / Gaps

- なし（受入条件確定済みの是正タスク）。

## Self-Review

- task が 1 単位で完結する粒度（probe + lane + 証拠）
  に分割し、各 step が独立に検証可能な順序にした。
- `NotRun` 残しの power-loss/Indeterminate は
  verification matrix で「実施しない」ではなく
  「実施不能・理由付き」として扱う。

## Implementation Handoff

task doc の Done When / Verification に従い実装する。
観測値の evidence 転記は実測後に行い、推測値を記録
しない。
