---
id: "TSPEC-rf1029v5Nx8Lp2Qw7Km4Ty6Zs1Jd"
type: "test-spec"
status: "approved"
title: "SV#28 Ruviform pin 更新 検証仕様（87a7ba8f → 0479fa62 / rf#1029）"
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
      - type: "file"
        path: "crates/synvelis-app/frontend/dist"
  implements: []
  implemented-by: []
  depends-on:
    - "docs/specs/sv28-ruviform-pin-rf1029.md"
    - "docs/designs/sv28-ruviform-pin-rf1029.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/sv28-ruviform-pin-rf1029.md"
    - "docs/designs/sv28-ruviform-pin-rf1029.md"
  derived-by: []
  verifies:
    - "docs/specs/sv28-ruviform-pin-rf1029.md"
    - "docs/designs/sv28-ruviform-pin-rf1029.md"
  verified-by: []
  references:
    - "Cargo.lock"
    - "tools/check-ruviform-rev.mjs"
    - "tools/check-headless-deps.mjs"
    - "mise.toml"
    - "evidence/sv28/rf1029/ruviform-0479fa62-diff-review.md"
  defers: []
  deferred-by: []
---

# SV#28 Ruviform pin 更新 検証仕様（rf#1029）

## Purpose

pin 更新が lockfile/vendor の単一 SHA 契約を保ち、workspace compile、recording、
real-device compile surface、app、RT、headless dependency、UI 経路で非回帰である
ことを canonical mise lane で検証する。provisioned endpoint、実機、実 GUI、
cross-consumer、merge 後 CI は本 slice の検証対象外として `NotRun` を明示する。

## Feature

- Feature: Ruviform pin update `87a7ba8f` → `0479fa62`。
- 検証対象: `Cargo.lock` 36 source、`ui/vendor/ruviform` REV、workspace compile、
  synvelis-recording、synvelis-app、RT、headless dependency、UI、prek hook。

## Rules

| # | Rule | 検証方法 |
| --- | --- | --- |
| R1 | lockfile 36 source が `0479fa62…` の単一 SHA | `mise run check-ruviform-rev` |
| R2 | vendor REV が lockfile と一致 | `mise run sync-ui-deps` 後の R1 |
| R3 | workspace 全体が compile 通過 | `mise run check-full` |
| R4 | recording crate が lint/test 通過 | `mise run lint synvelis-recording` / `mise run test synvelis-recording` |
| R5 | real-device compile surface が非回帰 | `mise run lint-recording-real-input`（実行はしない） |
| R6 | synvelis-app crate が lint/test 通過 | `mise run lint synvelis-app` / `mise run test synvelis-app` |
| R7 | RT/no-alloc/no-block 経路が非回帰 | `mise run test-rt` |
| R8 | headless dependency 契約が非回帰 | `mise run check-headless-deps` |
| R9 | UI frontend が typecheck/test/build 通過 | `mise run check-ui` / `mise run test-ui` / `mise run build-ui` |
| R10 | repository hygiene が通過 | `mise exec -- prek run --all-files` |
| R11 | 実行しない gate を正直に記録 | evidence/PR で NotRun/Deferred と理由を記載 |

## Examples

- 正常系: `check-ruviform-rev` が `REV match`、各 lane exit 0。
- 異常系: compile/lint/test の失敗が upstream API/behavior diff に起因する場合は、
  lease 内で最小適応する。lease 外変更が必要なら Coordinator へ確認する。

## Guarantees

- canonical lane と補助コマンドを区別する。
- `test-recording-virtual-endpoint` の exit 0/NotRun を Pass にしない。
- upstream rf#1029 provisioned Pass を synvelis consumer Pass に転用しない。
- 実機録音、virtual endpoint provisioning、実 GUI、実 plugin、cross-consumer tuple、
  3OS main CI は Pass と記載しない。

## Non-goals

- `mise run test-recording-real-input`。
- `mise run test-recording-virtual-endpoint`。
- synvelis#115 canonical EXT-L3 再 run、manual workflow dispatch、provisioned endpoint workflow。
- 実 OS Window、実 Plugin Editor/Native Dock、物理 Audio/MIDI 機材、市販 plugin。
- ruviform #219/#294・AudioLens #127 への tuple 提出。
- merge 後 main SHA の 3OS CI / Daily Full CI 統合検証（Coordinator が確認）。

## Risk

- `check-full` は compile 非回帰の下限であり、consumer-visible runtime contract の
  Pass を示さない。recording/app/UI の package lane と併記する。
- 既存 main 破損や他 Worker 起因の失敗が出た場合は証拠で分類し、スコープ内失敗と
  混同しない。
