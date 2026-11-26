---
id: "TSPEC-z4Xc8Vb2Nm6Qw9Er5Ty7Ui"
type: "test-spec"
status: "approved"
title: "SV#28 Ruviform pin 更新 検証仕様（20dc41f7 → 87a7ba8f）"
created: "2026-11-26"
updated: "2026-11-26"
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
    - "docs/specs/sv28-ruviform-pin-87a7ba8f.md"
    - "docs/designs/sv28-ruviform-pin-87a7ba8f.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/sv28-ruviform-pin-87a7ba8f.md"
    - "docs/designs/sv28-ruviform-pin-87a7ba8f.md"
  derived-by: []
  verifies:
    - "docs/specs/sv28-ruviform-pin-87a7ba8f.md"
    - "docs/designs/sv28-ruviform-pin-87a7ba8f.md"
  verified-by: []
  references:
    - "Cargo.lock"
    - "tools/check-ruviform-rev.mjs"
    - "tools/check-headless-deps.mjs"
    - "mise.toml"
    - "evidence/sv28/ruviform-87a7ba8f-diff-review.md"
  defers: []
  deferred-by: []
---

# SV#28 Ruviform pin 更新 検証仕様

## Purpose

pin 更新が lockfile/vendor の単一 SHA 契約を保ち、compile・lint・test・
RT・headless・UI・native/GPUI feature 経路で非回帰であることを canonical
mise lane で検証する。実機/実 GUI/provisioned endpoint/cross-consumer は
本 slice の検証対象外として `NotRun` を明示する。

## Feature

- Feature: Ruviform pin update `20dc41f7` → `87a7ba8f`。
- 検証対象: `Cargo.lock` 36 source、`ui/vendor/ruviform` REV、workspace
  compile/lint/test、RT、headless dependency、L1 runner artifact、
  UI/webview/native/GPUI lane、prek hook。

## Rules

| # | Rule | 検証方法 |
| --- | --- | --- |
| R1 | lockfile 36 source が `87a7ba8f…` の単一 SHA | `mise run check-ruviform-rev` |
| R2 | vendor REV が lockfile と一致 | `mise run sync-ui-deps` 後の R1 |
| R3 | workspace 全体が compile/lint/test 通過 | `mise run verify-full`（fmt-check/check-full/lint-full/test-full） |
| R4 | RT/no-alloc/no-block 経路が非回帰 | `mise run test-rt` |
| R5 | headless dependency 契約が非回帰 | `mise run check-headless-deps` |
| R6 | L1 runner artifact 経路が新 wire/host で動作 | `mise run test-tim03-l1` |
| R7 | UI/webview/native/GPUI feature lane が非回帰 | `check-ui` / `test-ui` / `build-ui` / `check-app-native` / `lint-app-native` / `build-app-native` / `check-app-gpui` / `lint-app-gpui` / `test-app-gpui` |
| R8 | real-device compile surface が非回帰 | `mise run lint-recording-real-input`（実行はしない） |
| R9 | repository hygiene が通過 | `mise exec -- prek run --all-files` |
| R10 | 実行しない gate を正直に記録 | evidence/PR で NotRun/Deferred と理由を記載 |

## Examples

- 正常系: `check-ruviform-rev` が `REV match`、各 lane exit 0。
- 異常系: fixture 適応漏れで `Plugin::buses` signature mismatch が出た場合は、
  該当 fixture を意味確認のうえ修正し lane を再実行する。

## Guarantees

- `verify-full` の内訳（fmt/check/lint/test）を区別して記録する。
- canonical lane と補助コマンドを区別する。
- 実機録音、virtual endpoint provisioning、実 GUI、実 plugin、
  cross-consumer tuple、3OS main CI は Pass と記載しない。

## Non-goals

- `mise run test-recording-real-input`。
- `mise run test-recording-virtual-endpoint`。
- 実 OS Window、実 Plugin Editor/Native Dock、物理 Audio/MIDI 機材、
  市販 plugin。
- ruviform #219/#294・AudioLens #127 への tuple 提出。
- merge 後 main SHA の 3OS CI 統合検証（Coordinator が確認）。

## Risk

- `verify-full` は時間がかかるが、dependency graph と公開契約が横断的に
  変わる pin update なので package 限定 lane に置き換えない。
- 既存 main 破損や他 Worker 起因の失敗が出た場合は証拠で分類し、
  スコープ内失敗と混同しない。
