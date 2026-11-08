---
id: "TASK-Xk9mQ2vR7wTzLp4nHs8dFj"
type: "task"
status: "in-progress"
title: "SV185-01: mixer.gain domain projection — master strip Fader bootstrap + Fader 接続"
created: "2026-11-08"
updated: "2026-11-08"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/185"
    - "https://github.com/scarecrow173/synvelis/issues/175"
  changes:
    added:
      - type: "file"
        path: "docs/specs/sv185-01-mixer-gain-domain-projection.md"
      - type: "file"
        path: "docs/designs/sv185-mixer-gain-projection-design.md"
      - type: "file"
        path: "docs/plans/sv185-mixer-gain-projection.md"
      - type: "file"
        path: "docs/test-specs/sv185-01-mixer-gain-fader.md"
    modified:
      - type: "file"
        path: "crates/synvelis-project/src/control.rs"
      - type: "file"
        path: "crates/synvelis-project/src/project.rs"
      - type: "file"
        path: "crates/synvelis-project/src/ops.rs"
      - type: "file"
        path: "crates/synvelis-project/src/migration.rs"
      - type: "file"
        path: "crates/synvelis-project/src/store.rs"
      - type: "file"
        path: "crates/synvelis-project/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-app/src/screens.rs"
      - type: "file"
        path: "crates/synvelis-app/src/ui_ops.rs"
      - type: "file"
        path: "crates/synvelis-app/src/shell.rs"
      - type: "file"
        path: "crates/synvelis-app/src/bin/synvelis.rs"
      - type: "file"
        path: "Cargo.lock"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/sv185-mixer-gain-projection.md"
  implemented-by: []
  depends-on:
    - "docs/specs/sv185-01-mixer-gain-domain-projection.md"
    - "docs/designs/sv185-mixer-gain-projection-design.md"
    - "docs/tasks/sv175-02-native-view-mount.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/sv185-01-mixer-gain-fader.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/sv185-01-mixer-gain-fader.md"
  references: []
  defers: []
  deferred-by: []
---

# SV185-01 実装タスク

Issue #185: `main_screen_spec` の Mixer `Fader`（`mixer.gain`）に
domain projection 元を整備し、`unsupported: true` を supported
kind に置き換える。ブランチは既存
`scarecrow173/sv175-native-view-mount`（Draft PR #183）で継続し、
ruviform pin を `769d595a` → `863a7a9d` 以降へ更新する。

## 変更点（実績）

- `crates/synvelis-project/src/control.rs`: `MASTER_FADER_RAW`
  （`WELL_KNOWN_BASE + 6`）+ 共有 constructor `master_fader_target`
  + `ControlModel::bootstrap(strip)`。
- `crates/synvelis-project/src/project.rs`: `Project::new` で
  master strip の Fader target を bootstrap + `validate_references`
  oracle（存在 + `role == Fader` + `owner == Strip(master)`）。
- `crates/synvelis-project/src/ops.rs`: `RemoveParameterTarget`・
  `AddSend`/`SetSendGainTarget`（gain adopt）が well-known target を
  拒否。`lib.rs` re-export。
- `crates/synvelis-project/src/migration.rs`/`store.rs`: schema v8 +
  v7→v8 step（canonical 補完 / issued master-strip `Fader` の
  well-known ID re-key）。
- `crates/synvelis-project/tests/`・`crates/synvelis-workspace/tests/`:
  `.keys().last()`/`.next_back()` helper を `is_issuable_raw` filter
  に修正 + bootstrap/migration/oracle/send-adopt test。
- `crates/synvelis-app/src/screens.rs`: Fader target/control_id を
  tagged param id、`unsupported: false`、`main_view_targets()`、
  module docs。
- `crates/synvelis-app/src/ui_ops.rs`: param `display_value` =
  `{"normalized": host_normalized}` + targets cap を issued のみへ。
- `crates/synvelis-app/src/shell.rs`/`src/bin/synvelis.rs`:
  ruviform `run_multi`/`SessionConfig` API 追従 — `NativeShell`
  （`new_with_data` で `DataRegistry` 共有）経由の event loop。
- `crates/synvelis-app/tests/view_mount.rs`・`tests/ui_ops.rs`・
  `crates/synvelis-workspace/tests/sv185_mixer_gain.rs`:
  fader gesture/projection/commit・targets listing test。
- `Cargo.lock` + `ui/vendor/ruviform`: pin `769d595a` → `863a7a9d`。

## 完了条件

- `mise run fmt`/`check`/`lint`/`test`/`build` green、
  `check-app-native`/`lint-app-native`/`build-app-native` 実測。
- headless: Fader controlled display が実値を反映し commit が
  domain に到達（test-spec の P/M 行）。
- 実機: Windows 実窓で Fader 描画・ドラッグ + Tabs キーボード
  （Arrow/Home/End）を証拠付きで記録。語彙は
  Passed/Failed/Blocked/NotRun/Unsupported。
- `git fetch origin main && git merge origin/main` 後に最終検証。
- push は `scarecrow173/sv175-native-view-mount` のみ。PR #183 は
  Draft 維持、検証記録はコメント投稿可。
