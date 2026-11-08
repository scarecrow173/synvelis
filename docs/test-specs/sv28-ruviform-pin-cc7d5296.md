---
id: "TSPEC-CrvvXGRCC7NBEy9bfba4LH"
type: "test-spec"
status: "approved"
title: "SV#28 Ruviform pin 更新 検証仕様"
created: "2026-11-09"
updated: "2026-11-09"
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
    - "docs/specs/sv28-ruviform-pin-cc7d5296.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/sv28-ruviform-pin-cc7d5296.md"
    - "docs/designs/sv28-ruviform-pin-cc7d5296.md"
    - "docs/plans/sv28-ruviform-pin-cc7d5296.md"
  derived-by: []
  verifies:
    - "docs/specs/sv28-ruviform-pin-cc7d5296.md"
    - "docs/designs/sv28-ruviform-pin-cc7d5296.md"
  verified-by: []
  references:
    - "Cargo.lock"
    - "tools/check-ruviform-rev.mjs"
    - "tools/check-headless-deps.mjs"
    - "mise.toml"
  defers: []
  deferred-by: []
---

# SV#28 Ruviform pin 更新 検証仕様

## Purpose

pin 更新が「意図した lockfile 変更のみ」を含み、headless lane と
feature lane の既存契約を回帰させないことを pin する。新規の
Scenario/Golden テスト追加は範囲外であり、本 spec は依存更新の
回帰防止に限定する。

## Feature

Ruviform pin 単一性と lane 別検証 — `Cargo.lock` の全
`ruviform-*` source が単一 SHA に解決され、workspace の各検証
lane が pin 更新後も pass する。

## Rules

- Rule: `Cargo.lock` 内の ruviform `git+...#<sha>` source は全て
  同一 SHA でなければならない（`check-ruviform-rev` の fail-closed
  契約。`--require-vendor` で vendor REV も一致必須）。
- Rule: pin 更新差分は ruviform の解決 revision と、それに伴う
  依存木の正当な変化のみを含む。無関係な crates.io version
  drift・手編集を含んではならない。
- Rule: headless crate の依存木に GUI/GPU/WebView framework
  crate（`ruviform-ui-gpui`/`ruviform-ui-webview`/`gpui`/`wry`/
  `tao` 等）が混入しない（`check-headless-deps`）。
- Rule: `ruviform-ui-test` の新規依存（`png`/`base64`）は
  dev-dependency 経路のみであり、headless crate の normal/build
  依存木へ GUI 系を持ち込まない。

## Examples

- Given `Cargo.lock` が更新済み
  When `node tools/check-ruviform-rev.mjs --require-vendor` を
  実行する
  Then `resolved cc7d5296...` で exit 0。
- Given pin 更新後の lockfile
  When `git diff Cargo.lock` を検査する
  Then ruviform source fragment の SHA 変更と、ui-test 経由の
  既存クレート（`png`/`base64`）の解決のみが確認でき、他の
  無関係な version 変更がない。
- Given 更新後の workspace
  When `mise run check` / `lint` / `test` / `test-rt` /
  `check-headless-deps` / `fmt-check` を実行する
  Then 全て exit 0。
- Given `webview` / `native` feature
  When `mise run check-app-webview` / `check-app-native` を実行する
  Then exit 0（feature lane のコンパイル契約が pin 後も成立）。

## Guarantees

- `ui/vendor/ruviform/` は `Cargo.lock` と同一 SHA の `ui/*`
  パッケージを保持する（`sync-ui-deps` が cargo checkout から
  再生成）。
- webview bin（`synvelis`）は `SessionConfig`/`run_multi` の
  driver-neutral host API を使い続け、削除済み legacy API
  （`HostConfig`/`run`）への依存を新たに持たない。
- `Cargo.toml` の依存定義・feature 構成は pin 更新で変更されない。

## Non-goals

- RF#464 Scenario Contract / RF#314 Golden を使う新規テストの
  実装（consumer 側の利用は後続タスク）。
- 実 GPUI ウィンドウ・実 OS 入力の検証（NotRun、CI/実機 lane の
  管轄）。
- ruviform 側 API の互換性修正（発見時は ruviform Issue 起票）。

## Risk

feature-gated lane（webview/native）でのみ顕在化する非互換は
既定 `mise run check` では検出できない。`check-app-webview` /
`check-app-native` の個別実行を省略すると、pin 更新起因の破壊を
見逃す。逆に headless lane の回帰は `check-headless-deps` と
`test`/`test-rt` が検出する。
