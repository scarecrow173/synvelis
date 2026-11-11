---
id: "SPEC-YmpDjzBvQVDopM2MpgcdhL"
type: "spec"
status: "approved"
title: "SV#28 Ruviform pin 更新 — 9865cd57 → fd1d4d5f と API 追従"
created: "2026-11-11"
updated: "2026-11-11"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/28"
  changes:
    added:
      - type: "file"
        path: "docs/discovery/sv28-ruviform-pin-fd1d4d5f.md"
      - type: "file"
        path: "docs/specs/sv28-ruviform-pin-fd1d4d5f.md"
      - type: "file"
        path: "docs/designs/sv28-ruviform-pin-fd1d4d5f.md"
      - type: "file"
        path: "docs/plans/sv28-ruviform-pin-fd1d4d5f.md"
      - type: "file"
        path: "docs/test-specs/sv28-ruviform-pin-fd1d4d5f.md"
      - type: "file"
        path: "docs/tasks/sv28-ruviform-pin-fd1d4d5f.md"
      - type: "file"
        path: "docs/impl/ir/sv-28-ruviform-pin-fd1d4d5f.md"
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
  implemented-by:
    - "docs/designs/sv28-ruviform-pin-fd1d4d5f.md"
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/tasks/sv-gui-02-ruviform-rev-rust-syn-ui-op.md"
    - "docs/tasks/sv28-ruviform-pin-update.md"
    - "docs/tasks/sv28-ruviform-pin-b02942c2.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/discovery/sv28-ruviform-pin-fd1d4d5f.md"
  derived-by:
    - "docs/designs/sv28-ruviform-pin-fd1d4d5f.md"
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
    - "tools/check-headless-deps.mjs"
  defers: []
  deferred-by: []
---

# SV#28 Ruviform pin 更新 — 9865cd57 → fd1d4d5f と API 追従

## What Should Be Built

`Cargo.lock` が解決する ruviform git 依存の revision を、既定ブランチ
追従の規約どおり ruviform `main` HEAD
`fd1d4d5f4de57092b96ad1cd309b85fe47cbef38` へ更新する。現行 pin
`9865cd57491f3f39061212a85f81f9ebce731bde` からの 6 commit 差分に
は、PR #567 の `ruviform-audio-device` WASAPI-native input
`frame_position` 供給（synvelis#115 が起票した上流修正で、
`synvelis-recording` の `frame_position` 優先経路が Windows 実機で
本線化）、PR #568 の WASAPI callback timestamp 修正、PR #566/#569/
#570 の `ruviform-ui-gpui` capture/os_input/multiwindow 拡張、
PR #571 の `ruviform-audio-plugin-bridge` unsupported 計数透過が
含まれる。pin 更新に伴う API 非互換があれば `crates/*/src/` の
最小限の追従修正のみ行い、SV#28 tracker の pin 更新 1 回分を完了する。

## Why Now

SV#28 は Ruviform 継続追従の tracker Issue であり、本 dispatch は
その pin 更新 1 回分の具体作業。現行 pin `9865cd57` は main HEAD
より 6 commits 遅れており、特に PR #567 は synvelis 側が起票した
Issue #115 の上流修正 — `synvelis-recording` が既に実装済みの
`frame_position` 優先経路へ実デバイスの frame position を供給する —
を含むため、遅れを置く理由がない。AL 側でも同一 bump
（`ruviform-fd1d4d5f-rev-watch`）が評価・採用済みで、両 Consumer
の pin 整合を保つ。

## Users and Value

- Synvelis 開発者: WASAPI 実機録音で `frame_position`（per-packet
  device position）が供給され、`resolve_first_frame` の stamp
  fallback に頼らない本線経路が有効になる（synvelis#115 の効果）。
- Synvelis 開発者: `real-scenario`/`OsInputOutcome`/unsupported
  計数透過等の新 API が利用可能になる（本 slice では採用せず、
  利用判断は後続タスク）。
- CI/検証: `check-ruviform-rev` の単一 SHA 契約を維持したまま
  lockfile-primary の pin 運用を継続できる。

## Scope

### In Scope

- `Cargo.lock` の ruviform 解決 revision を `fd1d4d5f` へ更新
  （`cargo update -p ruviform-*` による既定ブランチ追従。
  `Cargo.toml` の依存定義・git 追従方式は変更しない）。
- pin 更新に伴う API 非互換への最小限の追従修正
  （`crates/*/src/` のみ。差分調査の結果、公開 API の削除・
  変更は確認されていないため修正不要の見込み）。
- `ui/vendor/ruviform/` の `sync-ui-deps` 再生成（gitignore 対象の
  生成物。lockfile との一致を `check-ruviform-rev` で検証）。
- 差分の影響評価を影響表として文書に記録し、新規 API の採用
  判断（今回は採用しない）を明示する。
- 本タスクの文書チェーン（discovery/spec/design/plan/test-spec/
  task/impl）と managed index 再生成。

### Out of Scope

- `Cargo.toml` の依存定義・feature 構成の変更。
- `.github/`・`.devin/`・`mise.toml` の変更。
- `crates/` の変更（別 Worker が占有。pin 起因のコンパイル破壊が
  出た場合はアダプタ修正を試みず、API 差分を PR 本文に記録する）。
- 新 API（`real-scenario`/`OsInputOutcome`/unsupported 計数透過/
  `inject_unsupported`）の consumer 側採用。
- ruviform 側の不具合修正（発見時は ruviform Issue 起票で報告し、
  SV 側の恒久 workaround で隠さない）。
- 実 GPUI・実 OS・実 plugin・実 VST3・両 Consumer 横断の検証
  （NotRun/Unsupported として記録。Issue #28 は Open のまま）。

## Requirements

- `Cargo.lock` 内の全 ruviform `git+...#<sha>` source が単一の
  `fd1d4d5f4de57092b96ad1cd309b85fe47cbef38` に解決されること
  （`tools/check-ruviform-rev.mjs` の fail-closed 契約）。
- pin 更新以外の依存変更を混入させないこと（crates.io 等他
  パッケージの version drift があれば pin 更新と分離する）。
- API 追従修正は「pin 更新でコンパイル・契約が壊れた箇所の最小修正」
  に限定し、機能追加・リファクタリングを混ぜないこと。
- ruviform 側の破壊的変更が consumer 影響を持つと判断した場合は、
  SV 側で恒久 workaround を実装せず ruviform Issue 起票で報告する
  こと。
- 影響評価は feature/schema/project/graph/RT/plugin/GUI/recorder/
  automation/export の各軸で記録し、新規 API の採用判断を明示する
  こと。

## Acceptance Criteria

- [x] `Cargo.lock` の ruviform 解決 revision が main HEAD
  （`fd1d4d5f`）へ更新され、差分が意図した pin 更新＋推移的依存の
  必要更新のみであることを PR 本文で示す。
- [x] API 非互換があれば最小限の追従修正を行い、修正点と理由を
  列挙する。ruviform 側の疑わしい破壊は Issue 起票で報告する。
- [x] `mise run check` / `lint` / `test` / `test-rt` /
  `check-headless-deps` / `fmt-check` が pass する
  （`fmt-check` が main 既存破損で fail する場合は pin 起因かを
  切り分けて記録）。
- [x] `mise run check-app-webview` / `check-app-native` /
  `lint-app-native`（および `test-app-webview` 等の補助 lane）が
  pass する。
- [x] `node tools/check-ruviform-rev.mjs --require-vendor` が
  pass し、`ui/vendor/ruviform` が lock rev と一致する。
- [x] 影響表と新規 API の採用判断（今回は採用しない旨と後続
  Issue 候補）を文書・PR 本文に記録する。
- [x] 実 OS/実 GUI/実 plugin/両 Consumer 横断の検証は NotRun /
  Unsupported と記録し Pass に繰り上げない。Issue #28 は Open の
  まま（Close しない）。

## Deferred Design Concerns

<!-- Intentionally deferred future work. Link the deferred draft doc via relations.defers. -->

- `ruviform-ui-gpui` の `real-scenario`/`real_window`/`os_input`/
  `OsInputOutcome`/OS-window pixel capture: 実 OS 入力・実窓
  capture を伴う GUI 検証 lane の consumer 配線は、GUI 実機検証系
  Issue の別タスク判断事項。
- `ruviform-audio-plugin-bridge` の unsupported 計数透過の
  diagnostics/UI 消費配線: synvelis の status/diagnostics 表示側の
  設計判断が必要な別タスク。
- `ruviform-audio-core` の `midi_input::mock::inject_unsupported`:
  MIDI bridge 試験での mock 拡張採用は試験要件確定時の別タスク。
- WASAPI-native input path の実機効果検証:
  `test-recording-real-input` opt-in lane で `frame_position`
  本線化の実機証拠を採る後続検証（環境依存）。

## Sources and Relations

- Issue #28（Ruviform 継続追従 tracker。managed コメントは編集しない）。
- Issue #115（WASAPI input `frame_position` 上流修正の起票元）。
- 先行 pin 更新: PR #193（`cc7d5296`）、PR #301（`b02942c2`）。
- pin 更新規約: `docs/tasks/sv-gui-02-ruviform-rev-rust-syn-ui-op.md`、
  `tools/check-ruviform-rev.mjs`（Cargo.lock が唯一の正本）。
- ruviform 差分範囲: `9865cd57..fd1d4d5f`（6 commits）。
- 差分調査: `docs/discovery/sv28-ruviform-pin-fd1d4d5f.md`。
