---
id: "DESIGN-43KJVuVRlioN1JtUFVuqtW"
type: "design"
status: "approved"
title: "SV#28 Ruviform pin 更新 設計 — lockfile pin 更新と API 追従の境界"
created: "2026-11-09"
updated: "2026-11-09"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/28"
  changes:
    added:
      - type: "file"
        path: "docs/designs/sv28-ruviform-pin-cc7d5296.md"
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated:
      - type: "file"
        path: "ui/vendor/ruviform"
  implements:
    - "docs/specs/sv28-ruviform-pin-cc7d5296.md"
  implemented-by: []
  depends-on:
    - "docs/specs/sv28-ruviform-pin-cc7d5296.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/tasks/sv-gui-02-ruviform-rev-rust-syn-ui-op.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/sv28-ruviform-pin-cc7d5296.md"
  derived-by:
    - "docs/plans/sv28-ruviform-pin-cc7d5296.md"
    - "docs/test-specs/sv28-ruviform-pin-cc7d5296.md"
    - "docs/tasks/sv28-ruviform-pin-update.md"
  verifies: []
  verified-by:
    - "docs/test-specs/sv28-ruviform-pin-cc7d5296.md"
  references:
    - "Cargo.toml"
    - "Cargo.lock"
    - "tools/check-ruviform-rev.mjs"
    - "tools/sync-ruviform-ui.mjs"
  defers: []
  deferred-by: []
---

# SV#28 Ruviform pin 更新 設計

## 概要

ruviform git 依存は既定ブランチ追従で、解決 revision の正本は
`Cargo.lock` の `git+...#<sha>` fragment のみ（`Cargo.toml` には
rev 指定なし）。pin 更新は lockfile の再解決で行い、`mise run
sync-ui-deps` で vendored `ui/*` を同一 SHA へ再生成する。
`check-ruviform-rev` が lockfile / vendor REV / frontend manifest の
三者一致を fail-closed で検証する既存契約をそのまま使う。

## 差分調査結果（863a7a9d..cc7d5296）

ruviform 側 15 commits。`crates/` 配下の変更は以下に限定される。

| crate | 変更内容 | 互換性 |
| --- | --- | --- |
| `ruviform-io-file` | `rooted.rs` doc comment のみ（canonical 前提の明記、RF#468） | なし |
| `ruviform-ui` | `ViewHost::deliver_event` 追加、`attach_epoch()` accessor 追加、`AttachEpoch` に serde derive、RF#465 棚卸し doc comment | 追加のみ |
| `ruviform-ui-gpui` | `GpuiGuiTest`（`gpui-test` feature、RF#464）、Tab/Shift-Tab focus traversal 配線（RF#451 FU-1b）、render/observe 内部の `deliver_event` 利用への整理、example/test 追加 | 追加のみ |
| `ruviform-ui-test` | `golden` module（RF#314 slice1/2: key/diff/image/report/store）、`scenario` module（RF#464: SemanticGuiTest/FakeGuiTest/VisualGuiTest）、`png`/`base64` 依存追加 | 追加のみ |
| `ruviform-ui-webview` | 変更なし | — |
| その他（audio/graph/rt/media-*/plugin-*/viz/observability/exec/ui-theme） | 変更なし | — |

公開 API の削除・シグネチャ変更は diff 上確認されない
（`pub fn`/`pub struct`/`pub enum`/`pub trait`/`pub use` の削除行
ゼロ）。`ruviform-ui-webview` は無変更のため、synvelis webview
feature が使う `SessionConfig`/`WebViewWindowConfig`/`run_multi`/
`detect_backend`/`EmbedAssets` の面は不変。RF#465 の legacy API
（`HostConfig`/`run`/`HostEvent` 等）の物理削除は P2 であり本 pin
には含まれない。

## 設計判断

- **pin 更新方法**: `cargo update -p ruviform-<pkg>` を全 ruviform
  workspace 依存に対して実行する。git 既定ブランチ追従のため
  `--precise` は使わず、解決先は ruviform `main` HEAD
  `cc7d5296` となる。他の crates.io 依存は `-p` 指定により更新
  されない（lockfile 差分で確認）。
- **API 追従の境界**: `mise run check`（workspace・全 target）と
  `check-app-webview`/`check-app-native` でコンパイル契約を検証し、
  壊れた箇所のみ最小修正する。公開 API 削除が無いため修正は
  原則不要の見込みだが、feature-gated lane（webview/native）は
  既定 `check` の対象外なので個別に検証する。
- **workaround 禁止**: ruviform 側の疑わしい破壊・非互換を発見
  した場合、SV 側に恒久 workaround や第二 ledger を実装せず、
  ruviform Issue 起票で報告する（AGENTS.md の Ruviform 運用規約）。
- **legacy API の扱い**: synvelis `synvelis` bin は既に
  `SessionConfig`/`run_multi` へ移行済み（fa98f41）。RF#423/#465
  系の削除済み API 使用が見つかった場合は修正せず発見として
  報告する（受入条件どおり。対処は webview lane 退役/移行の
  別 Issue）。

## 検証戦略

正規入口は `mise.toml` タスク: `check` / `lint` / `test` /
`test-rt` / `check-headless-deps` / `fmt-check` を必須とし、
feature lane として `check-app-webview` / `check-app-native` /
`lint-app-native` / `test-app-webview` を補助証拠として実行する
（実窓の起動・目視は範囲外）。`check-ruviform-rev --require-vendor`
で lockfile・vendor・manifest の一致を確認する。

## リスクとロールバック

- リスク: pin 更新で推移依存（`png`/`base64` は既に lock tree 内
  解決済みと ruviform 側が確認）や feature 組合せでのみ顕在化する
  非互換。対策: 全 feature lane の check を実行。
- ロールバック: `Cargo.lock` の pin 差分を revert すれば旧 rev へ
  戻せる（`cargo update` は lockfile のみの変更で他ファイルを
  伴わない想定）。
