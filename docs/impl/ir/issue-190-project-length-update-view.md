---
id: "IMPL-034UgItAdeEhbkX8476hNc"
type: "impl"
status: "completed"
title: "Issue #190 project length → update_view 接続 — 既実装検証記録"
created: "2026-11-09"
updated: "2026-11-09"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/190"
    - "https://github.com/scarecrow173/ruviform/issues/451"
  changes:
    added:
      - type: "file"
        path: "docs/impl/ir/issue-190-project-length-update-view.md"
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/sv175-02-native-view-mount.md"
    - "docs/tasks/sv175-02-native-view-mount.md"
    - "docs/impl/ir/sv-28-ruviform-pin-863a7a9d-cc7d5296-api.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# Issue #190 project length → update_view 接続 — 既実装検証記録

## Summary

Issue #190（ruviform#451 FU-3 由来: project length 更新を
`GpuiDriver::update_view` 経由で mount 済み Seek の descriptor range に
反映する consumer 側接続）について、main 上に実装が既に存在することを
実コードで確認し、pin `cc7d5296`（PR #193 で `769d595a` 系から追従済み）
時点で全検証 lane が green であることを実測した。**コード変更なし・
検証のみ**の記録であり、再実装は行っていない。

## Context

- Issue #190 の受入条件: (1) project length 変更時に Seek range が
  remount なしで更新される、(2) 既存 contract/fake 検証で回帰なし、
  (3) 実 GPUI での確認証拠または NotRun 記録。
- 前提として探索済みの実装は SV175-02 slice2（commit `8139b28`）で
  main に取り込み済み。本タスクは pin 追従（PR #193、ruviform
  `cc7d5296`）後の再確認と Issue 完了処理のための検証記録。
- 実機証拠 G6（`seek range updated project_length_seconds=1.5` +
  thumb 追従、非再マウント）は pin `769d595a` 時点のもの
  （`docs/test-specs/sv175-02-native-view-mount.md` §2026-11-08 slice2、
  証拠 `F:/repositry/audio-products/.devin/evidence/sv175-slice2/verdict.md`）。

## Implementation

### Changes

コード変更なし。本書の追加のみ。

確認した実装箇所（HEAD `7fbd4c8`、origin/main 取込済み・差分なし）:

- `crates/synvelis-workspace/src/viz_ui.rs:3304`
  `project_length_seconds(session, media_rate)` — 秒軸の最新 clip 終端
  （`timeline_items` の max end、clip 無しは `0.0`）。
- `crates/synvelis-app/src/app.rs:303-321`
  `project_length_seconds()` / `project_length_cell()`（`Rc<Cell<f64>>`、
  seal 前に取得）/ `publish_project_length()`。
- `crates/synvelis-app/src/service.rs:250` — `Service::call` が全 op
  dispatch 後に `publish_project_length()` を呼ぶ。
- `crates/synvelis-app/src/screens.rs:74` `seek_range_seconds`
  （正の有限長 → 実長、空/非有限 → fallback `240.0` s）、
  `:222` `main_screen_spec(project_length_seconds)`、
  `:290` `mount_main_view`。
- `crates/synvelis-app/src/bin/synvelis-native.rs:191-264` — cell を
  取得して mount、driver ループで `project_length.get()` の変化を検出し
  `view_driver.update_view(window, main_screen_spec(length))` を呼ぶ
  （remount ではなく spec 更新）。

### Decisions During Implementation

- 検証のみのタスクのため spec→design→plan→task チェーンは作らず、
  本 IR 1 文書で代替（タスク Spec の指示どおり）。
- pin 差分の影響判断: cargo git checkout の実コードを
  `769d595a`（旧証拠 pin）と `cc7d5296`（現行 pin）で比較し、
  `ruviform-ui` の `ViewHost::update`（view.rs）、
  `ruviform-ui-gpui` の `GpuiDriver::update_view`（lib.rs）と
  `RvfView::update_spec`（render.rs）は **byte 同一**。
  render.rs の差分は tab focus handle・gpui-test seam・`try_input`
  の error surface であり update 経路に非関連。したがって旧 pin の
  G6 実機証拠が前提とする契約は現行 pin で維持されていると判断。

## Related Experiments

### Adopted

なし。

### Rejected

なし。

## Validation

環境: Windows、mise 管理 toolchain（rust pin は `mise.toml` 参照）。
HEAD = `7fbd4c8`（`git fetch origin main && git merge origin/main` で
Already up to date を確認）。

| lane | コマンド | 結果 |
| --- | --- | --- |
| fmt | `mise run fmt-check` (`cargo fmt --all -- --check`) | exit 0 |
| check | `mise run check` (`cargo check --workspace --all-targets`) | exit 0 |
| lint | `mise run lint` (`cargo clippy --workspace --all-targets -- -D warnings`) | exit 0、warning 0 |
| test | `mise run test` (`cargo test --workspace --locked`) | exit 0、全 suite pass・失敗 0 |
| 補助証拠 | `cargo test -p synvelis-app --test view_mount` | exit 0、13 passed（`seek_range_follows_project_length_in_place` 含む） |
| 補助証拠 | `cargo test -p synvelis-workspace --test project_length` | exit 0、2 passed |

- `view_mount.rs` の `seek_range_follows_project_length_in_place` は
  **実 `ruviform_ui::view::ViewHost`**（`mount_main_view` 生成）に対し
  `host.update(main_screen_spec(120.0), generation)` を呼び、
  全 op が `ProjectionOp::Updated`（Mount/Unmount 無し）、
  `seek.position` が更新対象、host generation 不変、spec が新長を
  反映することを検証 — Issue 受入条件 (1) の contract/fake 側証拠。
- `project_length.rs` は `project_length_seconds` の単体契約
  （空 project = 0.0、最新 clip 終端の秒換算）を検証。
- 実 GPUI 実機での pin `cc7d5296` 上の再観測は NotRun
  （実機 gate は ruviform#243/#313 の責務。旧 pin `769d595a` の G6
  Passed 記録と、上記 pin 間 byte 同一性の確認で代替判断）。

## Risks

- 実窓 GPUI での `update_view` 適用は pin `cc7d5296` では未再観測。
  契約関数は旧 pin と byte 同一であり headless 側は green だが、
  実機差分が出た場合は別途実機検証が必要。

## Follow-ups

- PR マージ後、Coordinator が Issue #190 を Close する
  （マージと Close は Coordinator の責務）。
