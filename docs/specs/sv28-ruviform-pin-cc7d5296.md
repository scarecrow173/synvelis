---
id: "SPEC-0D1aGavIsYu1NgyvnRxeRx"
type: "spec"
status: "approved"
title: "SV#28 Ruviform pin 更新 — 863a7a9d → cc7d5296 と API 追従（SV#190 前提解消）"
created: "2026-11-09"
updated: "2026-11-09"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/28"
  changes:
    added:
      - type: "file"
        path: "docs/discovery/sv28-ruviform-pin-update.md"
      - type: "file"
        path: "docs/specs/sv28-ruviform-pin-cc7d5296.md"
      - type: "file"
        path: "docs/designs/sv28-ruviform-pin-cc7d5296.md"
      - type: "file"
        path: "docs/plans/sv28-ruviform-pin-cc7d5296.md"
      - type: "file"
        path: "docs/test-specs/sv28-ruviform-pin-cc7d5296.md"
      - type: "file"
        path: "docs/tasks/sv28-ruviform-pin-update.md"
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
    - "docs/designs/sv28-ruviform-pin-cc7d5296.md"
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/tasks/sv-gui-02-ruviform-rev-rust-syn-ui-op.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/discovery/sv28-ruviform-pin-update.md"
  derived-by:
    - "docs/designs/sv28-ruviform-pin-cc7d5296.md"
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
    - "tools/check-headless-deps.mjs"
  defers: []
  deferred-by: []
---

# SV#28 Ruviform pin 更新 — 863a7a9d → cc7d5296 と API 追従

## What Should Be Built

`Cargo.lock` が解決する ruviform git 依存の revision を、既定ブランチ
追従の規約どおり ruviform `main` 最新 HEAD
`cc7d5296d3fbc65ce9b925834de4dadf5db666d9` へ更新する。現行 pin
`863a7a9dc132e4558cb18689dd80ac5288855bb0` からの差分には
RF#464 Scenario Contract、RF#314 Golden Utility（slice 1/2）、
RF#451 FU-1b（Tab/Shift-Tab focus traversal）、RF#243/#482 GPUI
実機ライフサイクル probe、RF#429 consumer observation seam 残 DoD、
RF#465 Legacy WebView 廃止 P0/P1 棚卸し、RF#461/#463 GPUI 修正等が
含まれる。pin 更新に伴う API 非互換があれば `crates/*/src/` の
最小限の追従修正のみ行い、SV#190 の前提となる基盤更新を完了する。

## Why Now

SV#28 は Ruviform 継続追従の tracker Issue であり、本 dispatch は
その pin 更新 1 回分の具体作業。最新 main には SV 側の後続作業
（SV#190 等）が依存する Scenario Contract・Golden・seam 検証基盤が
揃っており、pin を進めないと consumer 側の追従・検証が旧 API 面に
固定される。

## Users and Value

- Synvelis 開発者: RF#464 の consumer-neutral Scenario Contract と
  RF#314 の Golden Utility を consumer 側テストから利用可能になる。
- Synvelis 開発者: RF#451 FU-1b の Tab フォーカス traversal や
  RF#461/#463 の GPUI 修正を含む最新 driver 品質を得る。
- CI/検証: `check-ruviform-rev` の単一 SHA 契約を維持したまま
  lockfile-primary の pin 運用を継続できる。

## Scope

### In Scope

- `Cargo.lock` の ruviform 解決 revision を `cc7d5296` へ更新
  （`cargo update -p ruviform-*` による既定ブランチ追従。
  `Cargo.toml` の依存定義・git 追従方式は変更しない）。
- pin 更新に伴う API 非互換への最小限の追従修正
  （`crates/*/src/` のみ。差分調査の結果、公開 API の削除・変更は
  確認されていないため修正不要の見込み）。
- `ui/vendor/ruviform/` の `sync-ui-deps` 再生成（gitignore 対象の
  生成物。lockfile との一致を `check-ruviform-rev` で検証）。
- 本タスクの文書チェーン（spec/design/plan/test-spec/task/impl）。

### Out of Scope

- `Cargo.toml` の依存定義・feature 構成の変更。
- `.github/`・`.devin/` の変更。
- `crates/synvelis-app` の webview/native lane の大規模変更、
  新規 Scenario/Golden テストの実装（利用は後続タスク）。
- ruviform 側の不具合修正（発見時は ruviform Issue 起票で報告し、
  SV 側の恒久 workaround で隠さない）。
- 実 GPUI・実 OS 検証（`check-app-native`/`run-app-native` 相当の
  実窓確認は本タスク範囲外。NotRun として記録）。

## Requirements

- `Cargo.lock` 内の全 ruviform `git+...#<sha>` source が単一の
  `cc7d5296d3fbc65ce9b925834de4dadf5db666d9` に解決されること
  （`tools/check-ruviform-rev.mjs` の fail-closed 契約）。
- pin 更新以外の依存変更を混入させないこと（crates.io 等他
  パッケージの version drift があれば pin 更新と分離する）。
- API 追従修正は「pin 更新でコンパイル・契約が壊れた箇所の最小修正」
  に限定し、機能追加・リファクタリングを混ぜないこと。
- ruviform 側の破壊的変更が consumer 影響を持つと判断した場合は、
  SV 側で恒久 workaround を実装せず ruviform Issue 起票で報告する
  こと。

## Acceptance Criteria

- [ ] `Cargo.lock` の ruviform 解決 revision が最新 main
  （`cc7d5296`、RF#464・RF#314・RF#451 FU-1b 等を含む）へ更新され、
  差分が意図した pin 更新のみであることを PR 本文で示す。
- [ ] API 非互換があれば最小限の追従修正を行い、修正点と理由を
  列挙する。ruviform 側の疑わしい破壊は Issue 起票で報告する。
- [ ] `mise run check` / `lint` / `test` / `test-rt` /
  `check-headless-deps` / `fmt-check` が pass する。
- [ ] `synvelis` bin（webview feature）が削除済み legacy API
  （`HostConfig`/`run`、RF#423/#465 関連）を使っている場合は
  修正せず発見として報告する（対処方針は別 Issue）。
  現状 `SessionConfig`/`WebViewWindowConfig`/`run_multi` 移行済み
  （fa98f41）であり、RF#465 の legacy API 物理削除は P2 で本 pin に
  は含まれない見込み。
- [ ] 既存の headless/fake 検証で回帰なし。実 GPUI・実 OS 検証は
  NotRun 記録。

## Deferred Design Concerns

<!-- Intentionally deferred future work. Link the deferred draft doc via relations.defers. -->

- RF#465 P2: `ruviform-ui` の legacy WebView adapter 面
  （`event`/`window`/`bridge`/`render`/`detect` module、
  `WindowConfig::url`/`devtools`、`WindowHandle::eval`）の物理削除
  と driver-neutral asset/data delivery 契約。synvelis webview lane
  （`synvelis` bin + `ui/` frontend pipeline）の退役または移行判断は
  Coordinator 側の別 Issue で扱う。
- RF#314 slice 3: Pixel capture（screenshot 取得）の consumer 側
  配線。現行 slice 1/2 は Golden Utility コアと能力契約接続まで。

## Sources and Relations

- Issue #28（Ruviform 継続追従 tracker。managed コメントは編集しない）。
- 前提 Issue: SV#190（本 pin 更新が前提解消対象）。
- 先行 pin 更新: PR #185（`fa98f41`、769d595a → 863a7a9d +
  WebView host 新 API 移行）。
- pin 更新規約: `docs/tasks/sv-gui-02-ruviform-rev-rust-syn-ui-op.md`、
  `tools/check-ruviform-rev.mjs`（Cargo.lock が唯一の正本）。
- ruviform 差分範囲: `863a7a9d..cc7d5296`（15 commits、
  `crates/` の公開 API 変更は追加のみ・`ruviform-ui-webview` 無変更）。
