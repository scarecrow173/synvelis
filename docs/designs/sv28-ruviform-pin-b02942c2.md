---
id: "DESIGN-034VobNphX5isOqTl9Wg5i"
type: "design"
status: "approved"
title: "SV#28 Ruviform pin 更新 設計 — lockfile pin 更新と API 追従の境界"
created: "2026-11-11"
updated: "2026-11-11"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/28"
  changes:
    added:
      - type: "file"
        path: "docs/designs/sv28-ruviform-pin-b02942c2.md"
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated:
      - type: "file"
        path: "ui/vendor/ruviform"
  implements:
    - "docs/specs/sv28-ruviform-pin-b02942c2.md"
  implemented-by: []
  depends-on:
    - "docs/specs/sv28-ruviform-pin-b02942c2.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/tasks/sv-gui-02-ruviform-rev-rust-syn-ui-op.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/sv28-ruviform-pin-b02942c2.md"
  derived-by:
    - "docs/plans/sv28-ruviform-pin-b02942c2.md"
    - "docs/test-specs/sv28-ruviform-pin-b02942c2.md"
    - "docs/tasks/sv28-ruviform-pin-b02942c2.md"
  verifies: []
  verified-by:
    - "docs/test-specs/sv28-ruviform-pin-b02942c2.md"
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
1 回分（`0f5b5737` → `b02942c2`、80 commits）の境界を定める。
consumer が消費する公開面は差分調査（discovery doc）で additive
と評価済みだが、`ruviform-ui` の `view.rs`/`behavior.rs` に
Issue #410 の内部修正が含まれるため、feature lane を含む全検証
lane の実走で非回帰を確認する。

## Scope

### In Scope

- `Cargo.lock` の ruviform 解決 revision 更新と差分検査。
- `ui/vendor/ruviform/` の `sync-ui-deps` 再生成と
  `check-ruviform-rev --require-vendor` による三者一致確認。
- API 非互換検出時の `crates/*/src/` 最小追従修正（原則不要の
  見込み）。
- headless lane・webview/native feature lane・UI lane の検証。
- 文書チェーンと impl record。

### Out of Scope

- `Cargo.toml`・`.github/`・`.devin/` の変更。
- 新規 ruviform crate・新 API の consumer 採用。
- 実 OS/実 GUI/実 plugin の検証（NotRun 記録）。

## Components and Boundaries

- `Cargo.lock`: ruviform 解決 revision の唯一の正本。pin 更新の
  唯一の永続化対象。
- `tools/check-ruviform-rev.mjs`: lockfile/vendor/manifest の
  三者一致を fail-closed で検証する既存 gate。
- `tools/sync-ruviform-ui.mjs`: cargo checkout から `ui/vendor/
  ruviform` を lock rev で再生成する既存 tool。
- `crates/*`: API 非互換が検出された場合のみ最小追従修正の
  対象。今回は変更しない見込み。
- `docs/*`: 本タスクの文書チェーン。

## Data and Control Flow

- pin 更新: `cargo update -p ruviform-<pkg>`（全 ruviform
  workspace 依存）→ `Cargo.lock` の `git+...#<sha>` fragment が
  `b02942c2` に再解決 → `git diff Cargo.lock` で差分検査。
- vendor 同期: `mise run sync-ui-deps` → cargo checkout
  `~/.cargo/git/checkouts/ruviform-*/b02942c/` から `ui/*` を
  `ui/vendor/ruviform` へ複製し `REV` を記録 →
  `check-ruviform-rev --require-vendor` で一致確認。
- 検証: `mise run` の各 lane が lockfile-primary の依存木で
  workspace を compile/test する。

## 差分調査結果（0f5b5737..b02942c2）

80 commits。synvelis 消費の 26 crate 中 10 crate に変更あり
（詳細は discovery doc）。公開 API の削除は
`ruviform-ui-test` の root re-export `scenario::visual` 3 item
のみで、item は module path で存続し synvelis は未使用。
`ruviform-ui-gpui` の `OsCapturedFrame` re-export は
`OsWindowGeometry` 追加への拡張。`ruviform-ui` の大きな内部 diff
（Issue #410）は `pub` 宣言の削除・シグネチャ変更を含まない。
新規 crate は `ruviform-midi-device` のみで依存木外。

## 設計判断

- **pin 更新方法**: `cargo update -p ruviform-<pkg>` を全
  ruviform workspace 依存に対して実行する。git 既定ブランチ
  追従のため `--precise` は使わず、解決先は ruviform `main`
  HEAD `b02942c2` となる。他の crates.io 依存は `-p` 指定に
  より更新されない（lockfile 差分で確認）。
- **API 追従の境界**: `mise run check`（workspace・全 target）
  と `check-app-webview`/`check-app-native` でコンパイル契約を
  検証し、壊れた箇所のみ最小修正する。公開 API 削除が無いため
  修正は原則不要の見込みだが、feature-gated lane（webview/
  native）は既定 `check` の対象外なので個別に検証する。
- **新 API の採用境界**: `ruviform-midi-device`・
  `real-scenario`/`real_window`/`os_input`・`ProbeSourceAdapter`・
  `ReportDrain`/`drain_output_reports`・observability opt-in
  feature は本 slice では採用しない。採用は consumer 側の
  要件・設計判断を伴う別タスクとし、pin 更新に混ぜない
  （YAGNI/スコープ比例性）。
- **workaround 禁止**: ruviform 側の疑わしい破壊・非互換を
  発見した場合、SV 側に恒久 workaround や第二 ledger を実装
  せず、ruviform Issue 起票で報告する（AGENTS.md の Ruviform
  運用規約）。
- **legacy API の扱い**: synvelis `synvelis` bin は driver-
  neutral API 面（`SessionConfig`/`run_multi`）を使用済み。
  削除済み API への依存が見つかった場合は修正せず発見として
  報告する。

## Non-Functional Constraints

- `Cargo.lock` は再現性のためコミット対象。pin 更新の差分は
  ruviform SHA fragment と正当な推移的依存の変化に限定する。
- RT 契約: `ruviform-rt` は無変更。新規 `drain_output_reports`
  は ruviform 側で non-RT 契約と明記されており、synvelis の
  RT 経路へは混入しない。
- 検証は `mise.toml` のタスクを正規入口とし、cargo 直接実行の
  結果は補助証拠とする。

## Risks and Trade-offs

- リスク: pin 更新で推移依存や feature 組合せでのみ顕在化する
  非互換。対策: 全 feature lane（webview/native）の check/lint
  を個別に実行。
- リスク: Issue #410 の `ruviform-ui` 内部修正が consumer の
  観測挙動（stale projection 適用タイミング）を変える可能性。
  対策: `test`/`test-app-webview` の既存 contract test で
  非回帰を確認。挙動差が観測された場合は pin 起因か切り分けて
  記録する。
- リスク: 実 GPUI/OS 固有の問題は本環境（Windows worktree）で
  は検出不能な lane がある。対策: NotRun として PR に記録し、
  CI lane の結果に委ねる。
- ロールバック: `Cargo.lock` の pin 差分を revert すれば旧 rev
  へ戻せる（`cargo update` は lockfile のみの変更）。

## Verification Notes

- [x] 差分調査で公開 API の削除・シグネチャ変更が無いことを
  確認（discovery doc の Evidence）。
- [x] `mise run fmt-check` / `check` / `lint` / `test` /
  `test-rt` / `check-headless-deps` の pass。
- [x] `mise run check-app-webview` / `check-app-native` /
  `lint-app-native` / `test-app-webview` の pass。
- [x] `node tools/check-ruviform-rev.mjs --require-vendor` の
  pass（lockfile/vendor/manifest 三者一致）。
- [x] 実 OS/実 GUI/実 plugin/両 Consumer 横断は NotRun /
  Unsupported として記録。
