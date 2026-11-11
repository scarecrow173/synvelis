---
id: "DESIGN-VCS04BkndWiringDsn0001"
type: "design"
status: "approved"
title: "VCS-04 production backend wiring — module 設計"
created: "2026-11-12"
updated: "2026-11-12"
owners: []
relations:
  source:
    - "SPEC-VCS04BkndWiringSpec001"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/vcs-04-backend-wiring.md"
  implemented-by:
    - "docs/plans/vcs-04-backend-wiring-plan.md"
  depends-on:
    - "docs/specs/vcs-04-backend-wiring.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/vcs-04-history-visual-diff-ui-design.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/vcs-04-backend-wiring.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/vcs-04-backend-wiring.md"
  references: []
  defers: []
  deferred-by: []
---

# VCS-04 production backend wiring — module 設計

Spec: `docs/specs/vcs-04-backend-wiring.md`。`synvelis-app` 内の
配線詳細設計。新規 module は作らず `app.rs` に閉じる
（composition root の責務であり、他 crate の変更を伴わない）。

## 依存追加

`crates/synvelis-app/Cargo.toml` に
`synvelis-library = { path = "../synvelis-library" }` を追加。

- 理由: `GitVcsProvider`/`GitVcsConfig` は `synvelis-library` の
  git provider 実装であり、`VcsReadProvider`/`VcsBytesPort` 契約は
  `synvelis-project` が所有する。app 層が実 provider を bind する
  には library への直接依存が必要（従来は test fixture のみが
  使っていた）。
- 影響: 依存方向は `synvelis-app → synvelis-library →
  synvelis-project` で既存 DAG に辺を 1 本足すだけ。library 側・
  project 側の public API は不変で、他 consumer（library 自身の
  consumer・workspace・player 等）への影響はない。`Cargo.lock`
  は `cargo build` で再生成（synvelis-app deps に
  `synvelis-library` 1 行追加のみ）。

## `bind_vcs_backend`

`SynvelisApp::bind_vcs_backend(&mut self, bundle_path: &str)` —
`project_new_inner`/`project_open_inner` の store/workspace 確定後・
`Ok` 返却前に呼ぶ。

```text
vcs_backend = None                                  // 常に再検出
root   = ProjectPath::new(bundle_path)?             // InvalidProjectPath → warn
info   = GitVcsProvider::detect_workspace(&root)?   // NotRepository 等 → warn
provider = GitVcsProvider::open(&GitVcsConfig::new(bundle_path))?
project_file = info.project_root.is_empty()
    ? "main.json"
    : "{info.project_root}/main.json"               // repo-relative file path
adapter = VcsHistoryAdapter::new(provider, &info, project_file)?
vcs_backend = Some(Arc::new(adapter))
```

- 失敗は全て `Err(VcsError)` として closure 内に集約し、
  `tracing::warn!` で記録して `vcs_backend = None` を維持する。
  project new/open は失敗しない（bind は infallible-by-contract）。
- `detect_workspace` と `open` は同じ `probe_workspace` を通るため
  `info.project_root`（repo-relative bundle dir、`""` = worktree
  root）と provider の bound scope は一致する。
- `set_vcs_backend` は test 注入点として残す（`vcs04_history.rs`・
  `vcs06_ui_matrix.rs`・`rcd_04_product_e2e.rs` が fake backend を
  使う）。production 経路は `bind_vcs_backend` のみ。

## `refresh_project_file` + `VCS_PROJECT_FILE`

`DirectoryBundleStore` は `<bundle>/snapshots/r<N>.json` +
`<bundle>/HEAD` を持ち、snapshot 名は `ProjectRevision` 連動で
**安定 path を提供しない**。VCS endpoint は固定
`project_path` を必要とするため、bundle 直下の `main.json` を
VCS-facing mirror として新設する。

- `VCS_PROJECT_FILE = "main.json"`。
- `refresh_project_file(store, bundle_path)`: `store.head()` →
  `store.read_snapshot(head)` の canonical `SnapshotEnvelope`
  bytes を `<bundle>/main.json.tmp` に書き `rename` で publish。
  HEAD 不在（未 save）なら no-op。
- 呼出点: `project_new_inner`/`project_open_inner`/
  `project_save_inner` の save/load 成功直後。これにより
  `<project_root>/main.json` は常に saved HEAD と一致し、
  committed/staged endpoint が pre-save 内容を返して Saved
  endpoint と fake equality になることを防ぐ。
- mirror は派生物であり store の authoritative bytes は
  `snapshots/` + `HEAD` のまま。`main.json` が欠落・stale でも
  次回 save/open で修復される。store publish 機構
  （`StagedFile`/`SealedFile`）は使わず `fs::write`+`fs::rename`
  の tmp+rename とした — mirror の crash 一貫性は store 本体より
  弱い要件で十分（部分書込みは次回 refresh で上書きされる）。

## re-bind / clear 契約

- `project.new`/`open` は workspace/store/routing_engine/
  monitoring を reset するが `vcs_backend` は reset 対象外。
  そのため **bind は呼出ごとに必ず再検出**し、前 project root の
  backend を保持しない（冒頭 `vcs_backend = None`）。
- 非 repo・検出失敗・open/bind 失敗 → `None` → `syn.vcs.*` は
  `NoBackend`（`syn.vcs.view` は `status: "no-backend"`）。
  fake/placeholder backend は bind しない。
- `project.reload` は `project_open` 経由で同じ re-bind を通る。

## 既知制限（設計上の記録）

- unborn HEAD（commit 0 件）では `GitVcsProvider::head_oid` が
  `git rev-parse --verify --quiet` の exit 1 + 空 stderr を
  `TransientIo` に誤分類し、`list_history` が
  `VcsError::Unavailable` を返す。view は `last_error` →
  `Failed` status で誠実に表示し、`syn.vcs.visual refresh` は
  `service_error` を返す。staged/status read は動作する。
  `synvelis-library` 側の別欠陥として本 slice では修正しない
  （spec 既知制限節・IR Follow-ups に再現手順を記録）。
- `main.json` は bundle dir に常に書かれる（非 repo でも）。
  VCS 不在時は単なる冗長 mirror であり害はない — 条件付き書込み
  より「常に HEAD と一致」の不変条件が単純で壊れにくい。

## スコープ外

- `synvelis-library`/`synvelis-project`/`synvelis-workspace` の
  コード変更、VCS write 系、native mount/dock 配線、
  `docs/*/README.md` index 更新。
