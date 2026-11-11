---
id: "IMPL-VvBJRLqOSjnq3Pv2b4D1aO"
type: "impl"
status: "completed"
title: "SV#28 Ruviform pin 更新 — 9865cd57 → fd1d4d5f API 追従"
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
      - type: "file"
        path: "docs/designs/README.md"
      - type: "file"
        path: "docs/discovery/README.md"
      - type: "file"
        path: "docs/impl/ir/README.md"
      - type: "file"
        path: "docs/plans/README.md"
      - type: "file"
        path: "docs/specs/README.md"
      - type: "file"
        path: "docs/tasks/README.md"
      - type: "file"
        path: "docs/test-specs/README.md"
    deleted: []
    renamed: []
    moved: []
    generated:
      - type: "file"
        path: "ui/vendor/ruviform"
  implements:
    - "docs/tasks/sv28-ruviform-pin-fd1d4d5f.md"
  implemented-by: []
  depends-on:
    - "docs/specs/sv28-ruviform-pin-fd1d4d5f.md"
    - "docs/designs/sv28-ruviform-pin-fd1d4d5f.md"
    - "docs/plans/sv28-ruviform-pin-fd1d4d5f.md"
    - "docs/tasks/sv28-ruviform-pin-fd1d4d5f.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/sv28-ruviform-pin-fd1d4d5f.md"
    - "docs/discovery/sv28-ruviform-pin-fd1d4d5f.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/sv28-ruviform-pin-fd1d4d5f.md"
  references:
    - "tools/check-ruviform-rev.mjs"
    - "tools/sync-ruviform-ui.mjs"
    - "tools/check-headless-deps.mjs"
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# SV#28 Ruviform pin 更新 — 9865cd57 → fd1d4d5f API 追従

## Summary

SV#28 tracker の pin 更新 1 回分として、`Cargo.lock` の ruviform
git 依存解決 revision を `9865cd57` から ruviform `main` HEAD
`fd1d4d5f4de57092b96ad1cd309b85fe47cbef38` へ更新した。
`9865cd57..fd1d4d5f` の 6 commits（PR #567 `ruviform-audio-device`
WASAPI-native input `frame_position` 供給・PR #568 WASAPI callback
timestamp 修正・PR #566/#569/#570 `ruviform-ui-gpui`
capture/os_input/multiwindow 拡張・PR #571
`ruviform-audio-plugin-bridge` unsupported 計数透過）を消費可能に
なった。公開 API の削除・シグネチャ変更は 0 件（全件 additive）で、
SV 側の追従修正は不要だった。PR #567 により
`synvelis-recording` 既存の `frame_position` 優先経路が Windows
実機で本線化する（synvelis#115 起票の上流修正・コード変更不要）。
新 API（`real-scenario`/`OsInputOutcome`/unsupported 計数透過/
`inject_unsupported`）は本 slice では採用せず、後続 Issue 候補
として記録。

## Context

- `docs/tasks/sv28-ruviform-pin-fd1d4d5f.md`（本タスク）
- `docs/plans/sv28-ruviform-pin-fd1d4d5f.md` /
  `docs/designs/sv28-ruviform-pin-fd1d4d5f.md` /
  `docs/specs/sv28-ruviform-pin-fd1d4d5f.md` /
  `docs/discovery/sv28-ruviform-pin-fd1d4d5f.md` /
  `docs/test-specs/sv28-ruviform-pin-fd1d4d5f.md`（文書チェーン）
- 先行 pin 更新: PR #193（`cc7d5296`）、PR #301（`b02942c2`）。
- 参照: audio-lens worktree の同一 bump 評価
  `docs/specs/ruviform-fd1d4d5f-rev-watch.md`（AL#127）。

## Implementation

### Changes

- `Cargo.lock`: `cargo update -p ruviform-*`（全 27 ruviform
  workspace 依存を `-p` 指定）で全 ruviform source を
  `fd1d4d5f` へ再解決。差分は 27 箇所全てが `ruviform-*` の
  `source = "git+...#<sha>"` fragment の SHA 更新と、
  `ruviform-audio-device` の依存リストへの `windows 0.62.2`/
  `windows-core 0.62.2`（cfg(windows)）、`ruviform-ui-gpui` の
  依存リストへの `x11rb`（linux-gated optional）参照追加のみ。
  いずれも lockfile 既存 version の参照追加で、新規 package
  entry・version drift なし。`ruviform-midi-device` は synvelis
  の依存木に含まれず lockfile に非出現。
- `crates/*/src/`: 変更なし。API 差分調査（discovery doc）の
  結果、公開 API の削除・シグネチャ変更は 0 件で追従修正は
  不要。`mise run check`（workspace・全 target）で実コンパイル
  確認済み。
- `ui/vendor/ruviform/`: `mise run sync-ui-deps` で新 SHA へ
  再生成（gitignore 対象の生成物）。`REV` =
  `fd1d4d5f4de57092b96ad1cd309b85fe47cbef38`。
- `docs/*/README.md`: `doc_maintenance apply --force-index` による
  managed index 再生成（本タスクの 6 文書分）。
  `docs/impl/ir/README.md` は hand-curated のため手動で 1 行追記。

### Decisions During Implementation

- pin 更新は repo 規約どおり `cargo update -p`（既定ブランチ追従、
  `--precise` 不使用）で実施。`Cargo.toml` の依存定義は不変。
- ruviform 差分の互換性評価は ruviform local repo の
  `git log 9865cd57..fd1d4d5f`（6 commits）+
  `git diff -- 'crates/*/src/'` の `pub` 削除スキャン（0 件）+
  新規 `pub` item の個別確認で実施。AL 側の同一 bump 評価
  （`ruviform-fd1d4d5f-rev-watch`）と additive 評価が整合。
- PR #567 の `frame_position` 供給は `synvelis-recording` 既存の
  優先経路（`input.rs` `resolve_first_frame`）に実機値が流れる
  内部挙動改善で、SV 側の採用・配線作業は不要。効果の実測は
  `real-device` opt-in lane の管轄として本 slice では compile
  検証（`lint-recording-real-input`）までに留める。
- 新規 API・新規 crate の採用判断は「今回は採用しない」で統一
  （discovery doc の「新 API の採用判断」節・spec の Deferred
  Design Concerns に記録）。

## Related Experiments

### Adopted

<!-- なし -->

### Rejected

<!-- なし -->

## Validation

Windows 11・mise 管理 rust 1.99.0・cargo 1.99.0 で実施。

| 検証 | 結果 |
| --- | --- |
| `mise run check`（`cargo check --workspace --all-targets`） | Passed（exit 0） |
| `mise run fmt-check` | Passed（exit 0） |
| `mise run check-headless-deps` | Passed（exit 0、10 headless crates に GUI 系依存なし） |
| `mise run lint`（`cargo clippy --workspace --all-targets -- -D warnings`） | Passed（exit 0） |
| `mise run test`（`cargo test --workspace --locked`） | TBD |
| `mise run test-rt` | TBD |
| `mise run test-msrv` | TBD |
| `mise run check-app-webview` | TBD |
| `mise run lint-app-webview` | TBD |
| `mise run test-app-webview` | TBD |
| `mise run check-app-native` | TBD |
| `mise run lint-app-native` | TBD |
| `mise run check-ui` | TBD |
| `mise run test-ui` | TBD |
| `mise run lint-recording-real-input` | TBD（補助 lane・real-device feature compile） |
| `node tools/check-ruviform-rev.mjs --require-vendor` | Passed（`resolved fd1d4d5f…`、27 sources、REV match） |

NotRun: 実 GPUI ウィンドウ・実 OS 入力・実 plugin/VST3・実 audio
device（`test-recording-real-input`）・両 Consumer 横断の検証は
本タスク範囲外（Unsupported/NotRun として記録。実機 lane は
CI/手動検証に委ねる）。Issue #28 は Open のまま。

## Risks

- `wasapi_input`（約 1,700 行の新規 Win32/WASAPI input 経路）は
  `#[cfg(windows)]` かつ `real-device` optional dep 内に閉じて
  おり、既定 build・headless lane には影響しない。実機経路の
  挙動証拠は opt-in lane（`test-recording-real-input`）と ruviform
  側 evidence（PR #567/#568 の実機 closeout）に委ねる。
- `ruviform-ui-gpui` の `x11rb`（linux-gated optional）と
  `ruviform-audio-device` の `windows`/`windows-core`（cfg(windows)
  target-gated）依存リスト追加は、いずれも lockfile 既存 version
  で新規 package entry を増やさない。
- 新規 API（`real-scenario`/`OsInputOutcome`/unsupported 計数
  透過/`inject_unsupported`/`ruviform-midi-device`）の採用判断は
  後続タスクに委ねる。

## Follow-ups

- WASAPI input `frame_position` 本線化の実機証拠:
  `test-recording-real-input` opt-in lane で frame_position 経路が
  実デバイスで観測されることを確認する後続検証（synvelis#115 系・
  環境依存）。
- 新規 ruviform API の consumer 採用判断（`real-scenario`/
  `real_window`/`os_input`/`OsInputOutcome`、unsupported 計数透過、
  `inject_unsupported`）は必要に応じて後続 Issue で扱う。
- SV#28 tracker は Open のまま継続。次回 pin 更新は main HEAD
  追従で実施。
