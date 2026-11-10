---
id: "TSPEC-034Voc4dPYUq3RE0eQXwi7"
type: "test-spec"
status: "approved"
title: "SV#28 Ruviform pin 更新 検証仕様"
created: "2026-11-11"
updated: "2026-11-11"
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
    - "docs/specs/sv28-ruviform-pin-b02942c2.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/sv28-ruviform-pin-b02942c2.md"
    - "docs/designs/sv28-ruviform-pin-b02942c2.md"
    - "docs/plans/sv28-ruviform-pin-b02942c2.md"
  derived-by: []
  verifies:
    - "docs/specs/sv28-ruviform-pin-b02942c2.md"
    - "docs/designs/sv28-ruviform-pin-b02942c2.md"
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
Scenario/Golden/real-scenario テスト追加や新 API の採用は範囲外
であり、本 spec は依存更新の回帰防止に限定する。

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
  `tao`/`ruviform-midi-device` 等）が混入しない
  （`check-headless-deps`）。
- Rule: ruviform 側の新規 opt-in feature（`real-scenario`/
  `tracing-export`/`allocation-probe`/`integration`）は
  synvelis の feature 宣言で有効化されない（`Cargo.toml` 不変）。
- Rule: `ruviform-ui-test` の root re-export 削除
  （`scenario::visual` 3 item）は synvelis の使用面に影響しない
  （module path で存続・synvelis 未使用）。

## Examples

- Given `Cargo.lock` が更新済み
  When `node tools/check-ruviform-rev.mjs --require-vendor` を
  実行する
  Then `resolved b02942c2...` で exit 0。
- Given pin 更新後の lockfile
  When `git diff Cargo.lock` を検査する
  Then ruviform source fragment の SHA 変更のみが確認でき、他の
  無関係な version 変更がない。
- Given 更新後の workspace
  When `mise run check` / `lint` / `test` / `test-rt` /
  `check-headless-deps` を実行する
  Then 全て exit 0。
- Given `webview` / `native` feature
  When `mise run check-app-webview` / `check-app-native` /
  `lint-app-native` を実行する
  Then exit 0（feature lane のコンパイル契約が pin 後も成立）。
- Given `fmt-check` が fail する
  When 失敗内容を origin/main の同一ファイルと照合する
  Then pin 更新起因でない main 既存破損であることを切り分けて
  記録する（pin 起因なら保守的に pin を戻して報告）。

## Guarantees

- `ui/vendor/ruviform/` は `Cargo.lock` と同一 SHA の `ui/*`
  パッケージを保持する（`sync-ui-deps` が cargo checkout から
  再生成）。
- webview bin（`synvelis`）は `SessionConfig`/`run_multi` の
  driver-neutral host API を使い続け、削除済み legacy API
  （`HostConfig`/`run`）への依存を新たに持たない。
- `Cargo.toml` の依存定義・feature 構成は pin 更新で変更されない。
- 新規 ruviform crate（`ruviform-midi-device` 等）と新 API
  （`real-scenario`/`ProbeSourceAdapter`/`ReportDrain`/observability
  opt-in feature）は本 slice では採用されない。

## Non-goals

- RF#253/RF#293/RF#314/RF#464 の新 API を使う新規テスト・consumer
  配線の実装（利用は後続タスク）。
- 実 GPUI ウィンドウ・実 OS 入力・実 plugin/VST3 の検証
  （NotRun、CI/実機 lane の管轄）。
- ruviform 側 API の互換性修正（発見時は ruviform Issue 起票）。
- 両 Consumer（synvelis / ruviform 側 consumer）横断の検証
  （Unsupported として記録）。

## Risk

feature-gated lane（webview/native）でのみ顕在化する非互換は
既定 `mise run check` では検出できない。`check-app-webview` /
`check-app-native` の個別実行を省略すると、pin 更新起因の破壊を
見逃す。逆に headless lane の回帰は `check-headless-deps` と
`test`/`test-rt` が検出する。Issue #410 の `ruviform-ui` 内部
修正は API を変えないが観測挙動を変え得るため、既存 contract
test の非回帰が第二の検出面となる。
