---
id: "PLAN-Xk9mQ2vR7wTzLp4nHs8dFj"
type: "plan"
status: "approved"
title: "SV185-01 mixer.gain projection 実施 plan"
created: "2026-11-08"
updated: "2026-11-08"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/185"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/sv185-01-mixer-gain-domain-projection.md"
    - "docs/designs/sv185-mixer-gain-projection-design.md"
  implemented-by: []
  depends-on:
    - "docs/specs/sv185-01-mixer-gain-domain-projection.md"
    - "docs/designs/sv185-mixer-gain-projection-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/tasks/sv185-01-mixer-gain-fader.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# SV185-01 mixer.gain projection plan

対象 spec/design: `docs/specs/sv185-01-mixer-gain-domain-projection.md`、
`docs/designs/sv185-mixer-gain-projection-design.md`。

## 手順

1. **domain bootstrap**（`synvelis-project`）
   - `control.rs`: `MASTER_FADER_RAW` 定数 + 共有 constructor
     `master_fader_target` + `ControlModel::bootstrap`
   - `project.rs`: `Project::new` で routing から master strip を引き
     bootstrap を適用 + `validate_references` に well-known fader の
     存在・`role == Fader`・`owner == Strip(master)` oracle
   - `ops.rs`: `RemoveParameterTarget` が well-known target を拒否、
     `AddSend`/`SetSendGainTarget` の gain adopt も `!is_issuable_raw`
     で拒否（send teardown による wedge 防止）
   - `migration.rs`/`store.rs`: `SCHEMA_VERSION` v8 + v7→v8 step
     （canonical target 補完 / 既存 issued master-strip `Fader` の
     well-known ID への re-key）
   - `lib.rs`: `MASTER_FADER_RAW` re-export
   - test helper 修正（`.keys().last()`/`.next_back()` →
     `is_issuable_raw` filter）+ bootstrap 不変条件 test
2. **wire 接続**（`synvelis-app`）
   - `screens.rs`: Fader の target/control_id を tagged param id に、
     `unsupported: false`、`main_view_targets()` 追加、module docs 更新
   - `ui_ops.rs`: param row の `display_value` =
     `{"normalized": host_normalized}` + `syn.control.targets` の
     `TARGET_CAP` を issued target のみに限定（well-known は常時列挙）
   - `shell.rs`/`bin/synvelis.rs`: ruviform pin 更新に伴う
     `run_multi`/`SessionConfig`/`NativeShell` への移行
     （`HostConfig`/`run`/`HostEvent` API 廃止への追従）
3. **headless test**
   - `tests/view_mount.rs`: unsupported test を supported gesture
     test（begin/preview/commit → applied receipt + display=実値 +
     初期 projection）に置き換え、save/reload 永続化も確認
   - `ctl_01.rs`: bootstrap 不変条件・v7→v8 migration（re-key 含む）・
     oracle 拒否 test
   - `sf_02.rs`: send gain adopt の well-known 拒否 test
   - `sv185_mixer_gain.rs`（workspace）: projection/gesture/commit
     の domain 経路 test
   - `ui_ops.rs`: `syn.control.targets` が bootstrap fader を列挙する
     test
4. **ruviform pin 更新**: `769d595a` → `863a7a9d`
   （`cargo update` → `Cargo.lock` → `sync-ui-deps` →
   `check-ruviform-rev`）
5. **検証**: `mise run fmt`/`check`/`lint`/`test`/`build` +
   `check-app-native`/`lint-app-native`/`build-app-native` +
   `check-app-webview`/`lint-app-webview`/`test-app-webview`。
   最終検証前に `git fetch origin main && git merge origin/main`。
6. **実機検証**: Windows 実窓で Fader 描画・ドラッグ・Tabs
   キーボード（Left/Right/Home/End、RF#451）を確認し、
   `F:/repositry/audio-products/.devin/evidence/sv185-mixer-gain/`
   に verdict + 証拠を記録。
7. **docs/test-specs 更新・コミット・push**（既存 branch
   `scarecrow173/sv175-native-view-mount`、Draft PR #183 は維持、
   検証記録は PR コメントとして投稿可）。

## リスク

- test helper の `.keys().last()` 誤認（design D6 で洗い出し済み —
  実際には `.keys().last()`/`.next_back()` が 12 箇所 +
  `.keys().next()` 系の semantic 影響を精査）。
- v7→v8 migration の re-key は persisted 参照
  （lanes/assignments/sends）を全て張り替える必要がある — 漏れは
  oracle が検出する。
- pin 更新で ruviform API が変わる可能性 — `check`/`lint` で検出、
  synvelis 側 workaround は入れない（task 契約）。実際に
  `ruviform-ui-webview` の `HostConfig`/`run`/`HostEvent` が
  driver-neutral `run_multi`/`SessionConfig`/`WindowDriver` に
  置き換わっていたため `synvelis.rs` を `NativeShell` 構成に移行。
