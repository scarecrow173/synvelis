---
id: "TSPEC-31xKoCzy7Vtv6MX1J7safw"
type: "test-spec"
status: "approved"
title: "SV#28 Ruviform pin 更新 検証仕様（9865cd57 → fd1d4d5f）"
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
    - "docs/specs/sv28-ruviform-pin-fd1d4d5f.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/sv28-ruviform-pin-fd1d4d5f.md"
    - "docs/designs/sv28-ruviform-pin-fd1d4d5f.md"
    - "docs/plans/sv28-ruviform-pin-fd1d4d5f.md"
  derived-by: []
  verifies:
    - "docs/specs/sv28-ruviform-pin-fd1d4d5f.md"
    - "docs/designs/sv28-ruviform-pin-fd1d4d5f.md"
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
  drift・手編集を含んではならない。`ruviform-audio-device` の
  `windows 0.62.2`/`windows-core 0.62.2`（cfg(windows)）と
  `ruviform-ui-gpui` の `x11rb`（linux-gated optional）への
  依存リスト参照追加は、lockfile 既存 version であり正当な
  推移的変化として許容する。
- Rule: headless crate の依存木に GUI/GPU/WebView framework
  crate（`ruviform-ui-gpui`/`ruviform-ui-webview`/`gpui`/`wry`/
  `tao`/`ruviform-midi-device` 等）が混入しない
  （`check-headless-deps`）。
- Rule: ruviform 側の新規 opt-in feature（`real-scenario`）は
  synvelis の feature 宣言で有効化されない（`Cargo.toml` 不変）。
- Rule: `wasapi_input` を含む `ruviform-audio-device` の変更は
  `real-device` optional dep 内に閉じ、既定 build の headless
  依存木を変えない。

## Examples

- Given `Cargo.lock` が更新済み
  When `node tools/check-ruviform-rev.mjs --require-vendor` を
  実行する
  Then `resolved fd1d4d5f...` で exit 0。
- Given pin 更新後の lockfile
  When `git diff Cargo.lock` を検査する
  Then ruviform source fragment の SHA 変更と依存リスト参照追加
  のみが確認でき、他の無関係な version 変更がない。
- Given 更新後の workspace
  When `mise run check` / `lint` / `test` / `test-rt` /
  `check-headless-deps` / `test-msrv` を実行する
  Then 全て exit 0。
- Given `webview` / `native` feature
  When `mise run check-app-webview` / `check-app-native` /
  `lint-app-native` を実行する
  Then exit 0（feature lane のコンパイル契約が pin 後も成立）。
- Given `real-device` feature（wasapi_input 経路を含む）
  When `mise run lint-recording-real-input` を実行する
  Then exit 0（PR #567 の新規コードが synvelis 側の feature
  組合せで compile できる）。
- Given `fmt-check` が fail する
  When 失敗内容を origin/main の同一ファイルと照合する
  Then pin 更新起因でない main 既存破損であることを切り分けて
  記録する（pin 起因なら保守的に pin を戻して報告）。

## Guarantees

- `ui/vendor/ruviform/` は `Cargo.lock` と同一 SHA の `ui/*`
  パッケージを保持する（`sync-ui-deps` が cargo checkout から
  再生成）。
- `synvelis-recording` の `resolve_first_frame` は引き続き
  `info.frame_position` を優先参照し、pin 更新で WASAPI 実機
  経路の値が流れてもコード変更は不要（既存優先経路の本線化）。
- `Cargo.toml` の依存定義・feature 構成は pin 更新で変更されない。
- 新規 ruviform crate（`ruviform-midi-device`、bridge dev-dep
  経由のみ）と新 API（`real-scenario`/`OsInputOutcome`/
  unsupported 計数透過/`inject_unsupported`）は本 slice では
  採用されない。

## Non-goals

- `real-scenario`/`OsInputOutcome`/unsupported 計数・
  `inject_unsupported` を使う新規テスト・consumer 配線の実装
  （利用は後続タスク）。
- 実 GPUI ウィンドウ・実 OS 入力・実 plugin/VST3・実 audio
  device（`test-recording-real-input`）の検証（NotRun、CI/実機
  lane の管轄）。
- ruviform 側 API の互換性修正（発見時は ruviform Issue 起票）。
- 両 Consumer（synvelis / audio-lens）横断の検証
  （Unsupported として記録）。

## Risk

feature-gated lane（webview/native/real-device）でのみ顕在化する
非互換は既定 `mise run check` では検出できない。
`check-app-webview`/`check-app-native`/`lint-recording-real-input`
の個別実行を省略すると、pin 更新起因の破壊を見逃す。逆に
headless lane の回帰は `check-headless-deps` と `test`/`test-rt`
が検出する。`wasapi_input` の新規コードは optional dep かつ
`#[cfg(windows)]` に閉じているため、既定 lane への影響はないが
実機経路の証拠は opt-in lane に委ねられる。
