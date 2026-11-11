---
id: "PLAN-SY804uxbfTEGFF4irB0NVO"
type: "plan"
status: "approved"
title: "RCD-02 Unsynced-Write Loss Fixture + Real-FS Probe Plan"
created: "2026-11-11"
updated: "2026-11-11"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/116"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/rcd-02-unsynced-loss-real-fs-probe.md"
  implemented-by:
    - "docs/tasks/rcd-02-task-unsynced-loss-real-fs-probe.md"
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/plans/rcd-02-durability-recovery-review.md"
    - "docs/plans/rcd-02-stagedfile-publish-migration-plan.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/rcd-02-unsynced-loss-real-fs-probe.md"
    - "docs/designs/rcd-02-unsynced-loss-real-fs-probe-design.md"
  derived-by: []
  verifies:
    - "docs/test-specs/rcd-02-unsynced-loss-real-fs-probe.md"
  verified-by:
    - "docs/test-specs/rcd-02-unsynced-loss-real-fs-probe.md"
  references: []
  defers: []
  deferred-by: []
---

# RCD-02 Unsynced-Write Loss Fixture + Real-FS Probe Plan

## Goal

Issue #116 残存 slice を 1 task で実装する。

1. `recovery_review.rs` に unsynced-write 喪失 fixture
   （sync frontier への truncate）2 test を追加し、verified
   prefix 以外が `Complete` にならないことを検証する。
2. `recording-poc realfs` + `#[ignore]` opt-in test +
   `test-recording-real-fs` manual task で Windows/NTFS の
   real-FS 挙動を実測し、ledger に実測行を追記する。

## Scope

In scope は design File Map と同一。`synvelis-recording/src/`・
`Cargo.toml`/依存・他 crate・workflow は変更しない。

## Upstream Documents

- `docs/specs/rcd-02-unsynced-loss-real-fs-probe.md`
- `docs/designs/rcd-02-unsynced-loss-real-fs-probe-design.md`

## Design Inputs

- unsynced 喪失モデル: `checkpoint()` 直後の実 file 長 =
  最終 sync frontier。post-checkpoint byte を `set_len` で
  除去して power-loss 相当状態を作る（design §1-2）。
- probe 観測点と `#[ignore]` opt-in 運用（design §3-4）。
- ledger 実測行の status 語彙（design §5）。

## File Map

| File | Change |
| --- | --- |
| `crates/synvelis-recording/tests/recovery_review.rs` | +2 test（対称/非対称 unsynced 喪失） |
| `crates/recording-poc/src/main.rs` | `realfs` dispatch・`realfs_probe`・`realfs_write_unsynced` |
| `crates/recording-poc/tests/real_fs_probe.rs` | 新規 `#[ignore]` test 2 件 |
| `mise.toml` | `[tasks.test-recording-real-fs]` 1 件 |
| `evidence/reports/rcd-02-durability-recovery-review-ledger.md` | 実測行・fixture 追記 |
| `docs/*` + `docs/*/README.md` | doc チェーン + index 再生成 |

## Implementation Sequence

1. `recovery_review.rs` に truncate helper + 2 test を追補し、
   `cargo test -p synvelis-recording --test recovery_review`
   で確認（補助証拠）。
2. `main.rs` に `realfs` サブコマンドを実装し、
   `real_fs_probe.rs` を新規作成。`--ignored` 実行で観測値を
   確認する。
3. `mise.toml` に task を追加し `mise run test-recording-real-fs`
   を本機で実行、観測値を確定する。
4. ledger へ実測行 + fixture 追記。
5. 全 lane 検証 → impl/ir 記録 → index 再生成。

## Dependencies and Risks

- `recording-poc` は `synvelis-recording` へ path 依存済みの
  ため新規依存なし。`staging::detect_dir_sync`/
  `publish_atomic*` は pub module 経由で利用。
- `write-unsynced` の ready 検出は stdout 行で行い sleep に
  頼らない（決定性）。child.kill 後は `wait` で回収。
- design Risk Register と同一。

## Task Breakdown

- `docs/tasks/rcd-02-task-unsynced-loss-real-fs-probe.md` —
  本 slice 全体の単一 task。

## Verification

- [ ] `mise run fmt-check` / `check` / `lint` / `test` /
      `test-rt` / `test-msrv`
- [ ] `mise run test-recording-real-fs`（opt-in・本機実測）
- [ ] 補助: `cargo test -p synvelis-recording --test
      recovery_review`、`cargo test -p recording-poc --test
      real_fs_probe -- --ignored`

## Open Questions / Gaps

- なし（spec/design で確定済み）。

## Self-Review

- task が 1 単位で完結する粒度（fixture + probe + ledger）
  に分割し、各 step が独立に検証可能な順序にした。
- `NotRun` 残しの power-loss は verification matrix で
  「実施しない」ではなく「実施不能・理由付き」として扱う。

## Implementation Handoff

task doc の Done When / Verification に従い実装する。
観測値の ledger 転記は実測後に行い、推測値を記録しない。
