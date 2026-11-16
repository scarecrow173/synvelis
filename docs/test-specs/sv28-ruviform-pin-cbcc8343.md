---
id: "TSPEC-6TEktuxBLkWUIBtS8qbO8g"
type: "test-spec"
status: "approved"
title: "SV#28 Ruviform pin 更新 検証仕様（abb9f372 → cbcc8343）"
created: "2026-11-16"
updated: "2026-11-16"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/28"
  changes:
    added: []
    modified:
      - type: "file"
        path: "Cargo.lock"
    deleted: []
    renamed: []
    moved: []
    generated:
      - type: "file"
        path: "ui/vendor/ruviform"
  implements: []
  implemented-by: []
  depends-on:
    - "docs/specs/sv28-ruviform-pin-cbcc8343.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/sv28-ruviform-pin-cbcc8343.md"
    - "docs/designs/sv28-ruviform-pin-cbcc8343.md"
    - "docs/plans/sv28-ruviform-pin-cbcc8343.md"
  derived-by: []
  verifies:
    - "docs/specs/sv28-ruviform-pin-cbcc8343.md"
    - "docs/designs/sv28-ruviform-pin-cbcc8343.md"
  verified-by: []
  references:
    - "Cargo.lock"
    - "tools/check-ruviform-rev.mjs"
    - "tools/check-headless-deps.mjs"
    - "mise.toml"
    - "evidence/sv28/ruviform-cbcc8343-diff-review.md"
  defers: []
  deferred-by: []
---

# SV#28 Ruviform pin 更新 検証仕様

## Purpose

pin 更新が synvelis の全 consumer 経路で非回帰であること、および
lockfile/vendor の単一 SHA 契約が保たれることを、canonical
mise lane で検証する。実機/実 GUI/実 plugin lane は opt-in
であり、本仕様では `NotRun`/`Unsupported` として扱う。

## Feature

- Feature: Ruviform 継続追従 pin 更新（SV#28 slice）。
- 検証対象: `Cargo.lock` の 28 `ruviform-*` source revision・
  `ui/vendor/ruviform` 整合・workspace 全体の compile/lint/
  test 非回帰・feature 経路（webview/native/gpui/real-device
  lint）・prek hook。

## Rules

| # | Rule | 検証方法 |
| --- | --- | --- |
| R1 | lockfile の 28 `ruviform-*` source が単一 SHA `cbcc83437616fdd7418da37183c6be66b6cfe225` を指す | `node tools/check-ruviform-rev.mjs --require-vendor` が `resolved cbcc8343… (28 lockfile sources, lockfile-primary, REV match)` を出力 |
| R2 | lockfile 差分は ruviform source fragment + `ruviform-audio-test` の `serde`/`serde_json` 依存追加のみ | `git diff Cargo.lock` が 30 insertions/28 deletions（28×-/+ fragment + 2 行依存追加）。依存リスト・version・新規 package の他の変動なし |
| R3 | vendor checkout が lockfile revision と一致 | `mise run sync-ui-deps` 実行後に R1 の REV match で確認 |
| R4 | workspace 全 package が新 revision で compile/lint/test 通過 | `mise run check/lint/test <pkg>` × 14 package が exit 0 |
| R5 | RT/headless/UI feature lane が非回帰 | `mise run test-rt`・`check-headless-deps`・`check-ui`/`test-ui`・`check-app-webview`/`lint-app-webview`/`test-app-webview`・`check-app-native`/`lint-app-native`/`build-app-native`・`check-app-gpui`/`lint-app-gpui`/`test-app-gpui`・`lint-recording-real-input` が exit 0 |
| R6 | format/hook が通過 | `mise run fmt-check`・`mise exec -- prek run --all-files` が exit 0 |
| R7 | 実施しない lane を正直に記録 | impl/ir doc に `NotRun`/`Unsupported` と理由を記載。実機録音・実 GUI・実 plugin・両 consumer 横断は Pass と記載しない。`scarecrow173/synvelis#488` 由来の既存 fail は pin 非起因として切り分け記録 |

## Examples

- 正常系: `check-ruviform-rev` が `REV match` を返し、
  全 lane が exit 0 → pin 更新受理。
- 異常系（想定しない）: lockfile に ruviform 以外の差分が
  混入、または消費面 API 破壊で compile 失敗 → 当該差分を
  除去/修正して再検証。現時点で該当なし。

## Guarantees

- `Cargo.lock` が ruviform 解決 revision の唯一の正本として
  単一 SHA を保持する。
- `ui/vendor/ruviform` が lockfile と同一 revision で再生成
  される。
- synvelis の全消費経路（headless・webview・native・gpui・
  real-device lint）が新 revision で非回帰。
- 実機依存の振る舞い（WASAPI 実入力・実 plugin・実 GUI）は
  opt-in lane の管轄であり、本 slice では `NotRun` として
  明示される。

## Non-goals

- 新規 upstream 機能（RunMetadata JSON・Cancellation fault・
  event record codec・test plugin fixture・dock-test 拡張・
  endpoint probe）の採用検証 — 別 slice。
- 実機録音・実 OS GUI・実 plugin/VST3・両 consumer 横断の
  実行 — opt-in/別環境。
- `*-msrv`/`*-full` lane — ユーザー指定により未実施。
- upstream 自体の正しさ検証 — ruviform repo の責務。
- docs index 再生成 — Coordinator が統合時に実施。

## Risk

- 大規模差分（57 files）の見落とし → crate 別評価 + `pub`
  削除行スキャン（0 件）+ 依存木交差判定 + `check
  --all-targets` で担保。evidence メモに記録。
- `ruviform-audio-test` 新 API との非互換 → additive のみで
  既存消費面と非衝突。`test` lane が非回帰を検証。
- #488 由来の既存 fail との混同 → lane 結果と upstream diff
  で切り分け。当該 crate は修正しない。
