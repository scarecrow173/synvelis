---
id: "TSPEC-3k4PHbyLAFIsmHw7CNvrYM"
type: "test-spec"
status: "approved"
title: "SV#28 Ruviform pin 更新 検証仕様（fd1d4d5f → ac10a2f7）"
created: "2026-11-15"
updated: "2026-11-15"
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
    - "docs/specs/sv28-ruviform-pin-ac10a2f7.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/sv28-ruviform-pin-ac10a2f7.md"
    - "docs/designs/sv28-ruviform-pin-ac10a2f7.md"
    - "docs/plans/sv28-ruviform-pin-ac10a2f7.md"
  derived-by: []
  verifies:
    - "docs/specs/sv28-ruviform-pin-ac10a2f7.md"
    - "docs/designs/sv28-ruviform-pin-ac10a2f7.md"
  verified-by: []
  references:
    - "Cargo.lock"
    - "tools/check-ruviform-rev.mjs"
    - "tools/check-headless-deps.mjs"
    - "mise.toml"
    - "evidence/sv28/ruviform-ac10a2f7-diff-review.md"
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
- 検証対象: `Cargo.lock` の 27 `ruviform-*` source revision・
  `ui/vendor/ruviform` 整合・workspace 全体の compile/lint/
  test 非回帰・feature 経路（webview/native/gpui/real-device
  lint）・prek hook。

## Rules

| # | Rule | 検証方法 |
| --- | --- | --- |
| R1 | lockfile の 27 `ruviform-*` source が単一 SHA `ac10a2f7761c08db73aebd861c0a5c1f53f269b7` を指す | `node tools/check-ruviform-rev.mjs --require-vendor` が `resolved ac10a2f7… (27 lockfile sources, lockfile-primary, REV match)` を出力 |
| R2 | lockfile 差分は ruviform source fragment のみ | `git diff Cargo.lock` が 54 行（27×-/+）のみ。依存リスト・version・新規 package なし |
| R3 | vendor checkout が lockfile revision と一致 | `mise run sync-ui-deps` 実行後に R1 の REV match で確認 |
| R4 | workspace 全 package が新 revision で compile/lint/test 通過 | `mise run check/lint/test <pkg>` × 14 package が exit 0 |
| R5 | RT/headless/UI feature lane が非回帰 | `mise run test-rt`・`check-headless-deps`・`check-ui`/`test-ui`・`check-app-webview`/`lint-app-webview`/`test-app-webview`・`check-app-native`/`lint-app-native`/`build-app-native`・`check-app-gpui`/`lint-app-gpui`/`test-app-gpui`・`lint-recording-real-input` が exit 0 |
| R6 | format/hook が通過 | `mise run fmt-check`・`mise run prek` が exit 0 |
| R7 | 実施しない lane を正直に記録 | impl/ir doc に `NotRun`/`Unsupported` と理由を記載。実機録音・実 GUI・実 plugin・両 consumer 横断は Pass と記載しない |

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

- 新規 upstream 機能（plugin-dock・test-vst3・audio-test・
  io-e2e・uitest・plugin_dock scenario・fault-injection・
  endpoint probe）の採用検証 — 別 slice。
- 実機録音・実 OS GUI・実 plugin/VST3・両 consumer 横断の
  実行 — opt-in/別環境。
- `*-msrv`/`*-full` lane — ユーザー指定により未実施。
- upstream 自体の正しさ検証 — ruviform repo の責務。

## Risk

- 大規模差分（108 files）の見落とし → crate 別評価 + `pub`
  削除行スキャン + 消費面 grep + `check --all-targets` で
  担保。evidence メモに記録。
- MIDI 振る舞い変化の非回帰 → upstream 修正は note-state
  権威の `PluginNode` ledger 移管で、synvelis 消費面は不変。
  `test` lane が非回帰を検証。
