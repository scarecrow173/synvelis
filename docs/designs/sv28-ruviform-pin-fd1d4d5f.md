---
id: "DESIGN-AyDQMCLnAv4MbV3s4qnT8D"
type: "design"
status: "approved"
title: "SV#28 Ruviform pin 更新 設計 — lockfile pin 更新と API 追従の境界（9865cd57 → fd1d4d5f）"
created: "2026-11-11"
updated: "2026-11-11"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/28"
  changes:
    added:
      - type: "file"
        path: "docs/designs/sv28-ruviform-pin-fd1d4d5f.md"
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated:
      - type: "file"
        path: "ui/vendor/ruviform"
  implements:
    - "docs/specs/sv28-ruviform-pin-fd1d4d5f.md"
  implemented-by: []
  depends-on:
    - "docs/specs/sv28-ruviform-pin-fd1d4d5f.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/tasks/sv-gui-02-ruviform-rev-rust-syn-ui-op.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/sv28-ruviform-pin-fd1d4d5f.md"
    - "docs/discovery/sv28-ruviform-pin-fd1d4d5f.md"
  derived-by:
    - "docs/plans/sv28-ruviform-pin-fd1d4d5f.md"
    - "docs/test-specs/sv28-ruviform-pin-fd1d4d5f.md"
    - "docs/tasks/sv28-ruviform-pin-fd1d4d5f.md"
  verifies: []
  verified-by:
    - "docs/test-specs/sv28-ruviform-pin-fd1d4d5f.md"
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

## Context

SV#28 は Ruviform 継続追従の tracker であり、本設計は pin 更新
1 回分（`9865cd57` → `fd1d4d5f`、6 commits）の境界を定める。
consumer が消費する公開面は差分調査（discovery doc）で全件
additive と評価済み（`pub` 削除 0 件）。注目点は PR #567 の
WASAPI-native input path が `synvelis-recording` の
`frame_position` 優先経路を実機で本線化することで、これは
`real-device` opt-in feature 経路内の挙動改善であり既定 build の
契約は不変。

## Scope

### In Scope

- `Cargo.lock` の ruviform 解決 revision 更新と差分検査。
- `ui/vendor/ruviform/` の `sync-ui-deps` 再生成と
  `check-ruviform-rev --require-vendor` による三者一致確認。
- headless lane・webview/native feature lane・UI lane の検証。
- 文書チェーンと impl record。

### Out of Scope

- `Cargo.toml`・`.github/`・`.devin/`・`mise.toml` の変更。
- `crates/` の変更（別 Worker 占有範囲。pin 起因の破壊は修正せず
  API 差分として報告する契約）。
- 新規 ruviform crate・新 API の consumer 採用。
- 実 OS/実 GUI/実 plugin の検証（NotRun 記録）。

## Components and Boundaries

- `Cargo.lock`: ruviform 解決 revision の唯一の正本。pin 更新の
  唯一の永続化対象。
- `tools/check-ruviform-rev.mjs`: lockfile/vendor/manifest の
  三者一致を fail-closed で検証する既存 gate。
- `tools/sync-ruviform-ui.mjs`: cargo checkout から `ui/vendor/
  ruviform` を lock rev で再生成する既存 tool。
- `crates/*`: 本 dispatch では変更しない（別 Worker 占有）。
  API 非互換が検出された場合は修正せず差分を報告する。
- `docs/*`: 本タスクの文書チェーン。

## Data and Control Flow

- pin 更新: `cargo update -p ruviform-<pkg>`（全 27 ruviform
  workspace 依存）→ `Cargo.lock` の `git+...#<sha>` fragment が
  `fd1d4d5f` に再解決 → `git diff Cargo.lock` で差分検査。
- vendor 同期: `mise run sync-ui-deps` → cargo checkout
  `~/.cargo/git/checkouts/ruviform-*/fd1d4d5/` から `ui/*` を
  `ui/vendor/ruviform` へ複製し `REV` を記録 →
  `check-ruviform-rev --require-vendor` で一致確認。
- 検証: `mise run` の各 lane が lockfile-primary の依存木で
  workspace を compile/test する。

## 差分調査結果（9865cd57..fd1d4d5f）

6 commits（PR #566〜#571）。synvelis 消費の 27 crate 中 4 crate
に変更あり（詳細は discovery doc）。`pub` item の削除・
シグネチャ変更は 0 件。`ruviform-audio-device` は private
`#[cfg(windows)] mod wasapi_input` の新設と `pub(crate)` 内部経路の
配線変更のみで公開面追加なし。`ruviform-audio-plugin-bridge` の
計数 field/getter、`ruviform-ui-gpui` の `OsInputOutcome`、
`ruviform-audio-core` の `mock::inject_unsupported` は全て追加型の
additive。lockfile の依存リスト追加は `windows 0.62.2`・
`windows-core 0.62.2`（audio-device・cfg(windows)）と `x11rb`
（ui-gpui・linux-gated optional）の 3 件で、いずれも lockfile
既存 version の参照追加。

## 設計判断

- **pin 更新方法**: `cargo update -p ruviform-<pkg>` を全
  ruviform workspace 依存に対して実行する。git 既定ブランチ
  追従のため `--precise` は使わず、解決先は ruviform `main`
  HEAD `fd1d4d5f` となる。他の crates.io 依存は `-p` 指定に
  より更新されない（lockfile 差分で確認）。
- **API 追従の境界**: `mise run check`（workspace・全 target）
  と `check-app-webview`/`check-app-native` でコンパイル契約を
  検証する。公開 API 削除が無いため修正は不要の見込みで、
  `crates/` は本 dispatch の編集範囲外のため、破壊が出た場合は
  修正せず API 差分を PR に記録する。
- **frame_position 本線化の扱い**: PR #567 は
  `synvelis-recording` 既存の `frame_position` 優先経路に実機
  値を供給する upstream additive で、SV 側コード変更は発生しない。
  効果の実測は `real-device` feature を有効化する opt-in lane
  （`test-recording-real-input`/`lint-recording-real-input`）の
  管轄で、本 slice では compile 面の補助検証までを行い実機証拠は
  後続に委ねる。
- **新 API の採用境界**: `real-scenario`/`os_input`/
  `OsInputOutcome`/unsupported 計数透過/`inject_unsupported` は
  本 slice では採用しない。採用は consumer 側の要件・設計判断を
  伴う別タスクとし、pin 更新に混ぜない（YAGNI/スコープ比例性）。
- **workaround 禁止**: ruviform 側の疑わしい破壊・非互換を
  発見した場合、SV 側に恒久 workaround や第二 ledger を実装
  せず、ruviform Issue 起票で報告する（AGENTS.md の Ruviform
  運用規約）。

## Non-Functional Constraints

- `Cargo.lock` は再現性のためコミット対象。pin 更新の差分は
  ruviform SHA fragment と正当な推移的依存の変化に限定する。
- RT 契約: `ruviform-rt` は無変更。`wasapi_input` は `real-device`
  opt-in feature 配下の optional dep 内に閉じ、既定 build・
  headless lane の依存木に新規 GUI/OS framework を混入しない
  （`check-headless-deps` で検証）。
- 検証は `mise.toml` のタスクを正規入口とし、cargo 直接実行の
  結果は補助証拠とする。

## Risks and Trade-offs

- リスク: pin 更新で推移依存や feature 組合せでのみ顕在化する
  非互換。対策: 全 feature lane（webview/native）の check/lint
  を個別に実行し、`lint-recording-real-input`（real-device
  feature の compile 検証）を補助 lane として実行する。
- リスク: `wasapi_input`（約 1,700 行の新規 Win32/WASAPI 経路）
  の compile/挙動は Windows 実機経路でのみ顕在化し、既定 lane
  では `#[cfg(windows)]` かつ optional dep 内に閉じる。対策:
  `lint-recording-real-input` で compile を検証し、実機挙動は
  opt-in lane・CI に委ねて NotRun と記録する。
- リスク: 実 GPUI/OS 固有の問題は本環境（Windows worktree）で
  は検出不能な lane がある。対策: NotRun として PR に記録し、
  CI lane の結果に委ねる。
- ロールバック: `Cargo.lock` の pin 差分を revert すれば旧 rev
  へ戻せる（`cargo update` は lockfile のみの変更）。

## Verification Notes

- [x] 差分調査で公開 API の削除・シグネチャ変更が無いことを
  確認（discovery doc の Evidence、`pub` 削除 0 件）。
- [x] `mise run fmt-check` / `check` / `lint` / `test` /
  `test-rt` / `check-headless-deps` の pass。
- [x] `mise run check-app-webview` / `check-app-native` /
  `lint-app-native` / `test-app-webview` の pass。
- [x] `node tools/check-ruviform-rev.mjs --require-vendor` の
  pass（lockfile/vendor/manifest 三者一致）。
- [x] 実 OS/実 GUI/実 plugin/両 Consumer 横断は NotRun /
  Unsupported として記録。
