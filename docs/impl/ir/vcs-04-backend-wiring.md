---
id: "IMPL-VCS04BkndWiringIr00001"
type: "impl"
status: "completed"
title: "VCS-04 production set_vcs_backend 配線 slice (synvelis#95)"
created: "2026-11-12"
updated: "2026-11-12"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/95"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-app/tests/vcs_backend_wiring.rs"
      - type: "file"
        path: "docs/specs/vcs-04-backend-wiring.md"
      - type: "file"
        path: "docs/designs/vcs-04-backend-wiring-design.md"
      - type: "file"
        path: "docs/plans/vcs-04-backend-wiring-plan.md"
      - type: "file"
        path: "docs/tasks/vcs-04-backend-wiring-task.md"
      - type: "file"
        path: "docs/test-specs/vcs-04-backend-wiring.md"
      - type: "file"
        path: "docs/impl/ir/vcs-04-backend-wiring.md"
    modified:
      - type: "file"
        path: "Cargo.lock"
      - type: "file"
        path: "crates/synvelis-app/Cargo.toml"
      - type: "file"
        path: "crates/synvelis-app/src/app.rs"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/tasks/vcs-04-backend-wiring-task.md"
    - "docs/specs/vcs-04-backend-wiring.md"
    - "docs/designs/vcs-04-backend-wiring-design.md"
    - "docs/plans/vcs-04-backend-wiring-plan.md"
  implemented-by: []
  depends-on:
    - "docs/designs/vcs-04-backend-wiring-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/ir/vcs-04-history-visual-diff-ui.md"
    - "docs/impl/ir/vcs-03-git-read-provider.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/plans/vcs-04-backend-wiring-plan.md"
  derived-by: []
  verifies:
    - "docs/test-specs/vcs-04-backend-wiring.md"
  verified-by: []
  references:
    - "crates/synvelis-app/src/app.rs"
    - "crates/synvelis-app/tests/vcs_backend_wiring.rs"
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# VCS-04 production set_vcs_backend 配線 slice (synvelis#95)

## Summary

Issue #95 の残件 slice として、headless read-only slice（PR #252）
で test 注入専用だった `SynvelisApp.vcs_backend` に production
配線を実装した。`project.new`/`project.open` が bundle を含む git
workspace を `GitVcsProvider::detect_workspace`/`open` で検出し、
`VcsHistoryAdapter` 経由で project root 単位に bind / re-bind
する。あわせて VCS-facing project file `main.json` を saved HEAD
の snapshot bytes から materialize し、committed/staged endpoint
が実 git 内容を解決できるようにした。`synvelis-app` に
`synvelis-library` 依存を 1 本追加（理由・影響は PR 本文に記録）。

## Context

Implements `docs/tasks/vcs-04-backend-wiring-task.md` per
`docs/specs/vcs-04-backend-wiring.md` and
`docs/designs/vcs-04-backend-wiring-design.md`, planned by
`docs/plans/vcs-04-backend-wiring-plan.md`, verified by
`docs/test-specs/vcs-04-backend-wiring.md`（synvelis#95 の
production wiring scope。native mount・実 OS gate・VCS write 系は
後続/対象外）。

## Implementation

### Changes

- `crates/synvelis-app/Cargo.toml` — `synvelis-library` path 依存
  を追加。`GitVcsProvider`/`GitVcsConfig` は library の git
  provider 実装であり、app が実 backend を bind するには直接依存
  が必要。依存方向 `synvelis-app → synvelis-library →
  synvelis-project` は既存 DAG に辺を足すのみで他 consumer に
  影響なし。`Cargo.lock` は `cargo build` で再生成（synvelis-app
  deps に `synvelis-library` 1 行追加のみ・手編集なし）。
- `crates/synvelis-app/src/app.rs`:
  - `VCS_PROJECT_FILE = "main.json"` — VCS-facing project file。
    `DirectoryBundleStore` の `snapshots/r<N>.json`+`HEAD` layout
    は revision 連動名で安定 path を持たないため、bundle 直下に
    HEAD snapshot bytes の mirror を置く規約とした。
  - `bind_vcs_backend(bundle_path)` — `vcs_backend = None` に
    クリア後、`ProjectPath::new` → `GitVcsProvider::detect_workspace`
    → `GitVcsProvider::open` → `VcsHistoryAdapter::new(provider,
    &info, "<project_root>/main.json")` を実行。失敗は closure に
    集約して `tracing::warn!` のみで `None` を維持
    （infallible-by-contract、project new/open を失敗させない）。
    `project_root` が `""`（bundle = worktree root）なら
    `main.json`、それ以外は `<project_root>/main.json`。
  - `refresh_project_file(store, bundle_path)` — `store.head()`
    → `read_snapshot` の bytes を `<bundle>/main.json.tmp` に
    書き `rename` publish。`project_new_inner`/
    `project_open_inner`/`project_save_inner` の成功直後に呼び、
    VCS-facing file が saved HEAD を lag しないことを保証。
  - `project_new_inner`/`project_open_inner` 末尾に
    `bind_vcs_backend(path)` を追加（`project_reload` は open
    経由で同じ re-bind を通る）。`set_vcs_backend` は test 注入点
    として維持。
- `crates/synvelis-app/tests/vcs_backend_wiring.rs`（新規・2 test）
  — real git CLI fixture（`synvelis-library/tests/common` の
  `git_fixture`/`vcs_git_fixture` と同規約: git 不在時 honest
  skip・hooksPath/lfs/gpgsign neutralize・Windows verbatim path
  対処）。`project_new_binds_real_backend_and_serves_committed_diff`
  は実 commit/index を跨ぐ committed/staged A/B diff を、
  `backend_rebinds_per_root_and_clears_on_non_repo` は非 repo での
  `None`/`no-backend`・repo A→B re-bind の `repository_id` 変化・
  repo→非 repo clear・`project.open` re-bind を検証。
- doc chain 一式（spec/design/plan/task/test-spec/IR）。

### Decisions During Implementation

- **`main.json` mirror 方式**: `snapshots/r<N>.json` は revision
  連動で VCS の固定 `project_path` に使えない。bundle 直下の
  `main.json` に HEAD bytes を mirror する方式を採用 — snapshot
  resolver が読む canonical bytes と同一内容で、committed/staged
  endpoint の `SnapshotEnvelope` 解決と一致する。store publish
  機構は使わず tmp+`fs::rename`（mirror は派生物で crash 一貫性の
  要件が store 本体より弱く、次回 refresh で修復される）。
- **bind は常に再検出**: `vcs_backend` は new/open で reset され
  ないため、`bind_vcs_backend` 冒頭で `None` にクリアしてから
  検出する。非 repo・検出失敗・bind 失敗は `None` → `NoBackend`
  を正直に返し、fake backend は bind しない。
- **`set_vcs_backend` 温存**: `vcs04_history.rs`・
  `vcs06_ui_matrix.rs`・`rcd_04_product_e2e.rs` が fake backend を
  注入する test 経路として維持。production 経路は
  `bind_vcs_backend` のみ。
- **unborn HEAD は fixture で回避し既知制限として記録**:
  `GitVcsProvider::head_oid`/`resolve_oid` が `--quiet` 付き
  `git rev-parse --verify` の unborn HEAD（exit 1 + 空 stderr）を
  `TransientIo` に誤分類する `synvelis-library` 側の既存欠陥を
  発見。0-commit repo で `syn.vcs.visual refresh` が
  `service_error`（view は `Failed` status で誠実表示）になる
  ことを実測で確認し、Coordinator 判断により本 slice では
  fixture を born-HEAD 状態に揃え、spec/design/test-spec/PR の
  既知制限・未対応の作業に再現手順つきで記録した
  （library 修正は別 Issue、Coordinator が統合時に起票）。

## Related Experiments

### Adopted

<!-- none -->

### Rejected

<!-- none -->

## Validation

Windows worktree・pinned rustc 1.99.0（MSRV lane は rust
1.89.0）で canonical mise lane を実走:

- `mise run fmt-check` — Pass（exit 0。初回は新規 test file の
  整形差分 2 件を `mise run fmt` で修正後に Pass）
- `mise run check` — Pass（exit 0）
- `mise run lint` — Pass（exit 0・`-D warnings`）
- `mise run test` — Pass（exit 0・workspace 全 test binary ok。
  `vcs_backend_wiring.rs` 2/2 を含む。`vcs04_history.rs`（app）
  9/9・（workspace）13/13・`vcs06_ui_matrix.rs` 4/4 等の既存
  consumer 回帰なし）
- `mise run test-rt` — Pass（exit 0）
- `mise run check-app-native` — Pass（exit 0）
- `mise run lint-app-native` — Pass（exit 0）
- `mise run test-msrv` — Pass（exit 0・rust 1.89.0。
  `vcs_backend_wiring.rs` 2/2 を MSRV toolchain でも実走確認）
- `mise run build` — Pass（exit 0）

## Risks

- 実 GPUI Window・実 OS window/dock・IME/keyboard/a11y・実機
  2Window は `NotRun`（#114 gate・VCS-UI-05）。headless Pass を
  実 GUI Pass と偽装しない。
- unborn HEAD の provider 分類バグは未修正（`synvelis-library`
  側・別 Issue 予定）。0-commit repo では history refresh が
  `service_error` + view `Failed` status になる — staged/status
  read は動作し、誠実な失敗として記録済み。
- `main.json` は非 repo bundle にも書かれる冗長 mirror。
  条件付き書込みより「常に HEAD と一致」の不変条件が単純で、
  VCS 化した時点で既に最新 mirror が存在する利点を取った。
- `project.new`/`open` 毎に git subprocess が追加で走る
  （detect + open の probe）。non-RT 境界の project 操作であり
  許容範囲と判断。

## Follow-ups

- **unborn HEAD provider 分類バグ**（`synvelis-library`
  `GitVcsProvider::head_oid`/`resolve_oid` が exit 1 + 空 stderr
  を `TransientIo` に誤分類）— 再現: `git init` repo 内に bundle
  を `project.new` し commit 前に `syn.vcs.visual
  {action:"refresh"}`。Coordinator が別 Issue 起票予定。
- `VcsHistory` view の native mount 配線（`dock_view_info`・
  composition-root attach 駆動）— 後続 slice。
- 実 OS/GPUI two-window dock・実機計測（VCS-UI-05）— synvelis#114
  gate の実機 lane。
- Overlay / Evolution Map / Rendered Audio Diff・media/waveform
  byte 配送 — spec 非目標、後続 slice。
- Issue #95 は上記後続 slice 残存のため Open 維持（Close は
  Coordinator 判断）。
