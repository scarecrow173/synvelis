---
id: "SPEC-034VobGQ7ailU8PviI0F9x"
type: "spec"
status: "approved"
title: "SV#28 Ruviform pin 更新 — 0f5b5737 → b02942c2 と API 追従"
created: "2026-11-11"
updated: "2026-11-11"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/28"
  changes:
    added:
      - type: "file"
        path: "docs/discovery/sv28-ruviform-pin-b02942c2.md"
      - type: "file"
        path: "docs/specs/sv28-ruviform-pin-b02942c2.md"
      - type: "file"
        path: "docs/designs/sv28-ruviform-pin-b02942c2.md"
      - type: "file"
        path: "docs/plans/sv28-ruviform-pin-b02942c2.md"
      - type: "file"
        path: "docs/test-specs/sv28-ruviform-pin-b02942c2.md"
      - type: "file"
        path: "docs/tasks/sv28-ruviform-pin-b02942c2.md"
      - type: "file"
        path: "docs/impl/ir/sv-28-ruviform-pin-b02942c2.md"
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
    - "docs/designs/sv28-ruviform-pin-b02942c2.md"
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/tasks/sv-gui-02-ruviform-rev-rust-syn-ui-op.md"
    - "docs/tasks/sv28-ruviform-pin-update.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/discovery/sv28-ruviform-pin-b02942c2.md"
  derived-by:
    - "docs/designs/sv28-ruviform-pin-b02942c2.md"
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
    - "tools/check-headless-deps.mjs"
  defers: []
  deferred-by: []
---

# SV#28 Ruviform pin 更新 — 0f5b5737 → b02942c2 と API 追従

## What Should Be Built

`Cargo.lock` が解決する ruviform git 依存の revision を、既定ブランチ
追従の規約どおり ruviform `main` 最新 HEAD
`b02942c2f9e6e195d0d752803b077866468fe58e` へ更新する。現行 pin
`0f5b57372e9a43f84415f7964b2f9636774174f0` からの差分には
RF#253 plugin→host report 配線と VST3 MIDI 正規経路、
RF#293/OBS-11 `ProbeSourceAdapter` と observability `integration`
layer、OBS-03/08/09/10 の opt-in feature、RF#314/RF#388/RF#552 の
GPUI real-scenario/real-window/OS input lane、RF#464 scenario
runner 拡張、Issue #410 の `ruviform-ui` 内部修正、Issue #418 の
ui-theme codegen、Issue #389 の plugin editor session hardening、
IO-02/03/04/06・Issue #251 の新規 IO/audio-device crate 等が
含まれる。pin 更新に伴う API 非互換があれば `crates/*/src/` の
最小限の追従修正のみ行い、SV#28 tracker の pin 更新 1 回分を完了する。

## Why Now

SV#28 は Ruviform 継続追従の tracker Issue であり、本 dispatch は
その pin 更新 1 回分の具体作業。現行 pin は main HEAD より約 80
commits 遅れており、RF#253 の plugin report 配線・OBS 系の
observability 基盤・RF#314 の real-scenario 検証 lane 等、consumer
側の後続作業が依存し得る基盤が蓄積している。pin を進めないと
consumer 側の追従・検証が旧 API 面に固定される。

## Users and Value

- Synvelis 開発者: `ruviform-midi-device`・`real-scenario`・
  `ProbeSourceAdapter`・`ReportDrain`・observability opt-in
  feature 等の新 API が利用可能になる（本 slice では採用せず、
  利用判断は後続タスク）。
- Synvelis 開発者: Issue #410 の stale projection 修正・
  Issue #389 の editor session hardening・RF#388 native dock
  slice 3 等の品質改善を含む最新 driver/plugin 基盤を得る。
- CI/検証: `check-ruviform-rev` の単一 SHA 契約を維持したまま
  lockfile-primary の pin 運用を継続できる。

## Scope

### In Scope

- `Cargo.lock` の ruviform 解決 revision を `b02942c2` へ更新
  （`cargo update -p ruviform-*` による既定ブランチ追従。
  `Cargo.toml` の依存定義・git 追従方式は変更しない）。
- pin 更新に伴う API 非互換への最小限の追従修正
  （`crates/*/src/` のみ。差分調査の結果、公開 API の削除・
  変更は確認されていないため修正不要の見込み）。
- `ui/vendor/ruviform/` の `sync-ui-deps` 再生成（gitignore 対象の
  生成物。lockfile との一致を `check-ruviform-rev` で検証）。
- 差分の影響評価を影響表として文書に記録し、新規 API の採用
  判断（今回は採用しない）を明示する。
- 本タスクの文書チェーン（spec/design/plan/test-spec/task/impl）。

### Out of Scope

- `Cargo.toml` の依存定義・feature 構成の変更。
- `.github/`・`.devin/` の変更。
- 新規 ruviform crate（`ruviform-midi-device` 等）の依存追加、
  新 API（`real-scenario`/`ProbeSourceAdapter`/`ReportDrain`/
  observability opt-in feature）の consumer 側採用。
- ruviform 側の不具合修正（発見時は ruviform Issue 起票で報告し、
  SV 側の恒久 workaround で隠さない）。
- 実 GPUI・実 OS・実 plugin・実 VST3・両 Consumer 横断の検証
  （NotRun/Unsupported として記録。Issue #28 は Open のまま）。

## Requirements

- `Cargo.lock` 内の全 ruviform `git+...#<sha>` source が単一の
  `b02942c2f9e6e195d0d752803b077866468fe58e` に解決されること
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

- [ ] `Cargo.lock` の ruviform 解決 revision が最新 main
  （`b02942c2`）へ更新され、差分が意図した pin 更新のみである
  ことを PR 本文で示す。
- [ ] API 非互換があれば最小限の追従修正を行い、修正点と理由を
  列挙する。ruviform 側の疑わしい破壊は Issue 起票で報告する。
- [ ] `mise run check` / `lint` / `test` / `test-rt` /
  `check-headless-deps` / `fmt-check` が pass する
  （`fmt-check` が main 既存破損で fail する場合は pin 起因かを
  切り分けて記録）。
- [ ] `mise run check-app-webview` / `check-app-native` /
  `lint-app-native`（および `test-app-webview` 等の補助 lane）が
  pass する。
- [ ] `node tools/check-ruviform-rev.mjs --require-vendor` が
  pass し、`ui/vendor/ruviform` が lock rev と一致する。
- [ ] 影響表と新規 API の採用判断（今回は採用しない旨と後続
  Issue 候補）を文書・PR 本文に記録する。
- [ ] 実 OS/実 GUI/実 plugin/両 Consumer 横断の検証は NotRun /
  Unsupported と記録し Pass に繰り上げない。Issue #28 は Open の
  まま（Close しない）。

## Deferred Design Concerns

<!-- Intentionally deferred future work. Link the deferred draft doc via relations.defers. -->

- `ruviform-midi-device`（OS MIDI input backend）の consumer 統合:
  実 MIDI device 入力の要件が確定した時点で依存追加と adapter
  設計を行う後続 Issue 候補。
- `ruviform-ui-gpui` の `real-scenario`/`real_window`/`os_input`:
  実 OS 入力・実窓 capture を伴う GUI 検証 lane の consumer 配線は
  別タスクの判断事項。
- `ruviform-audio` `typed::probe` の `ProbeSourceAdapter`:
  probe source の consumer 配線は別タスク。
- `ruviform-plugin-host` `ReportDrain`/`drain_output_reports`:
  plugin→host report の consumer 配線は synvelis-player の host
  層設計判断が必要な別タスク。
- `ruviform-observability` の `tracing-export`/`allocation-probe`/
  `integration` feature: observability 要件に応じた採用判断は
  別タスク。

## Sources and Relations

- Issue #28（Ruviform 継続追従 tracker。managed コメントは編集しない）。
- 先行 pin 更新: PR #193（`0f5b5737` への更新）。
- pin 更新規約: `docs/tasks/sv-gui-02-ruviform-rev-rust-syn-ui-op.md`、
  `tools/check-ruviform-rev.mjs`（Cargo.lock が唯一の正本）。
- ruviform 差分範囲: `0f5b5737..b02942c2`（80 commits）。
- 差分調査: `docs/discovery/sv28-ruviform-pin-b02942c2.md`。
