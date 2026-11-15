---
id: "DESIGN-gAU8JKJSNsO9ZsGEgF0CDj"
type: "design"
status: "approved"
title: "SV#28 Ruviform pin 更新 設計 — lockfile pin 更新と API 追従の境界（fd1d4d5f → ac10a2f7）"
created: "2026-11-15"
updated: "2026-11-15"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/28"
  changes:
    added:
      - type: "file"
        path: "docs/designs/sv28-ruviform-pin-ac10a2f7.md"
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated:
      - type: "file"
        path: "ui/vendor/ruviform"
  implements:
    - "docs/specs/sv28-ruviform-pin-ac10a2f7.md"
  implemented-by: []
  depends-on:
    - "docs/specs/sv28-ruviform-pin-ac10a2f7.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/tasks/sv-gui-02-ruviform-rev-rust-syn-ui-op.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/sv28-ruviform-pin-ac10a2f7.md"
    - "docs/discovery/sv28-ruviform-pin-ac10a2f7.md"
  derived-by:
    - "docs/plans/sv28-ruviform-pin-ac10a2f7.md"
    - "docs/test-specs/sv28-ruviform-pin-ac10a2f7.md"
    - "docs/tasks/sv28-ruviform-pin-ac10a2f7.md"
  verifies: []
  verified-by:
    - "docs/test-specs/sv28-ruviform-pin-ac10a2f7.md"
  references:
    - "Cargo.toml"
    - "Cargo.lock"
    - "tools/check-ruviform-rev.mjs"
    - "tools/sync-ruviform-ui.mjs"
    - "evidence/sv28/ruviform-ac10a2f7-diff-review.md"
  defers: []
  deferred-by: []
---

# SV#28 Ruviform pin 更新 設計

## 概要

`Cargo.lock` の ruviform 解決 revision を `ac10a2f7` へ更新し、
upstream 差分（`fd1d4d5f..ac10a2f7`、128 commits）の consumer
影響を評価する。synvelis の消費面に破壊的変更がないため、
変更は lockfile 更新 + vendor 再生成 + doc chain のみで、
製品コードの変更は行わない設計とする。

## Context

- pin の正本は `Cargo.lock` のみ（`Cargo.toml` の git 依存は
  `rev` 未指定で `main` を追従）。`check-ruviform-rev.mjs` が
  lockfile/vendor の単一 SHA 契約を検証する。
- 前回 pin `fd1d4d5f` 以降の upstream 差分は大規模
  （108 files・+18,207/−656）で、破壊的 API 変更を複数含むが、
  いずれも synvelis 未使用面に限定されることを grep +
  `cargo check --all-targets` で確認済み。
- 新規 upstream 機能は全て opt-in feature・依存木外 crate・
  test 基盤であり、本 slice のスコープ外。

## Scope

### In Scope

- `cargo update -p ruviform-*`（27 package 全指定）による
  lockfile 更新と差分検査。
- `mise run sync-ui-deps` による vendor 再生成と
  `check-ruviform-rev --require-vendor` 検証。
- canonical 検証 lane の実行と正直な結果記録。
- 7-doc chain + evidence メモ + index 再生成。

### Out of Scope

- 新規 upstream 機能の採用（下記「新 API 採用判断」参照）。
- `Cargo.toml` 変更、製品コード変更。
- 実機/実 GUI/実 plugin opt-in lane の実行。

## Components and Boundaries

- **lockfile（正本）**: `Cargo.lock` の `source =
  "git+https://github.com/scarecrow173/ruviform?rev=...#<sha>"`
  fragment が唯一の解決 revision を保持。`Cargo.toml` は
  `rev` を固定しない（`main` 追従 + lockfile pin の既存規約）。
- **vendor checkout（生成物）**: `ui/vendor/ruviform` は
  `tools/sync-ruviform-ui.mjs`（`mise run sync-ui-deps`）が
  lockfile 解決 revision で生成する gitignore 済み checkout。
  手編集禁止。
- **検証境界**: `tools/check-ruviform-rev.mjs --require-vendor`
  が lockfile 27 source の単一 SHA と vendor REV の一致を
  機械検証する。

## Data and Control Flow

```
origin/main (ruviform) @ ac10a2f7
  → cargo update -p ruviform-* (27 packages)
  → Cargo.lock source fragments → ac10a2f7…
  → mise run sync-ui-deps → ui/vendor/ruviform @ ac10a2f7
  → check-ruviform-rev --require-vendor → REV match
  → mise run <lane> × N → 結果を impl/ir doc に記録
```

## 差分調査結果（fd1d4d5f..ac10a2f7）

128 commits・`crates/` 108 files・+18,207/−656。crate 別の
consumer 影響評価は `evidence/sv28/ruviform-ac10a2f7-diff-review.md`
と discovery doc を正本とする。要点:

- **破壊的変更は存在するが全て synvelis 未使用面**:
  - `ruviform-plugin-core`: `GuiParentToken`→`ParentLease`、
    `WIRE_VERSION` 4→5、`merge_call_events` →
    `MergeOutcome{events,truncated}`。
  - `ruviform-plugin-host`: `HostCommand::OpenGui.parent`・
    `LeaseRedeemer` 戻り値型変更。
  - `ruviform-audio-plugin-bridge`: `InputStageCounters`/
    `NodeStatusSnapshot` の field 再編（`released`/
    `ledger_overflows` → `anchor_rejected`/
    `event_merge_truncated`）、`InputStage::release_all`/
    `active_notes` 削除。
  - `ruviform-ui-test`: `TestIo`/`TestApp`/`Journal*`/
    `HeadlessIo` の driver 語彙移行（`FakeHost`→`FakeDriver` 等）。
- **synvelis 消費面の削除・signature 変更は 0 件**。
  `cargo check --workspace --all-targets` が実証。
- **MIDI 振る舞いの改善**: bridge の note-state 権威が
  `PluginNode` ledger へ移管され、V9/V11/V12 event contract
  修正が入る。synvelis の MIDI 消費経路はコード変更不要で
  振る舞いが改善する。
- **`ruviform-audio-device`**: WASAPI input の
  `permission_denied` 分類追加（`real-device` opt-in lane 内）。
- **`ruviform-ui-gpui`**: `uitest` feature 新設（opt-in・
  synvelis 未有効化）。
- **`ruviform-io-file`**: `fault-injection` feature +
  `#[doc(hidden)]` mock facade 新設（opt-in・未有効化）。
- **新規 crate は全て依存木外**: `ruviform-audio-test`、
  `ruviform-plugin-dock-test`、`ruviform-plugin-test-vst3`、
  `ruviform-io-e2e` 等は lockfile 非含有（27 package 不変）。

## 設計判断

| 判断 | 内容 | 根拠 |
| --- | --- | --- |
| lockfile のみ更新 | `Cargo.toml` の git 依存は `rev` 未指定のまま維持し、`cargo update -p` で lockfile を更新 | 既存 pin 規約（`check-ruviform-rev` が lockfile-primary を要求）。`rev` 固定は行わない |
| scoped `cargo update -p` | 27 package 全てを `-p` 指定して ruviform のみ更新 | 無関係な crates.io drift を排除。lockfile 差分は source fragment 54 行のみ |
| vendor は生成物 | `mise run sync-ui-deps` で再生成し手編集しない | 既存規約。`--require-vendor` で lockfile との一致を検証 |
| 製品コード変更なし | 消費面の破壊的変更 0 件のため `crates/` 配下は不変 | `cargo check --all-targets` + grep による消費面スキャンで確認 |
| 新 API 不採用 | plugin-dock/test-vst3/audio-test/io-e2e/uitest/plugin_dock scenario/fault-injection/endpoint probe を全て不採用 | 依存木外・opt-in・test 基盤。採用は GUI-L2/EXT-L3・Test Runtime・IO 試験要件の別 slice の判断事項（YAGNI） |
| `frame_position` 継続 | `synvelis-recording` の既存 `frame_position` 優先経路を維持 | upstream の WASAPI input 供給経路は既に本線化済み（前 slice）。今回の差分は permission 分類の additive のみ |
| ParentLease は記録のみ | 将来 `Plugin` trait 実装・`open_gui`・wire v5 使用時の移行要否を doc に記録 | 現時点で synvelis は当該 API を使用しない。先回りの抽象化は YAGNI 違反 |

## Non-Functional Constraints

- `mise.toml` の task が唯一の検証入口。cargo コマンドは
  `mise run` 経由。
- lockfile 差分は ruviform source fragment のみ。無関係な
  version drift・新規 package を混入させない。
- `ui/vendor/ruviform` は生成物として扱い、gitignore 済み・
  直接編集しない。
- 実機/実 GUI/実 plugin lane は opt-in であり、未実施は
  `NotRun`/`Unsupported` と正直に記録する（POLICIES.md）。

## Risks and Trade-offs

| リスク | 評価 | 対応 |
| --- | --- | --- |
| 消費面の見落としによる compile 破壊 | 低 | `cargo check --workspace --all-targets` 全 14 package で実証済み |
| MIDI 振る舞いの非回帰 | 低 | upstream が note-state 権威を `PluginNode` ledger へ移管する修正。synvelis の bridge 消費面は不変で、`test` lane が非回帰を検証 |
| wire v5 の影響 | なし | synvelis は wire `OpenGui`/`CloseGui` command を発行しない（in-process host の使用 variant に含まれない） |
| 大規模差分の見落とし | 中 | 108 files の精査は crate 単位 + `pub` 削除行スキャン + 消費面 grep で実施。evidence メモに記録 |
| vendor/lockfile 不整合 | 低 | `--require-vendor` が機械検証 |

## Verification Notes

- 機械検証: `check-ruviform-rev --require-vendor`、
  `sync-ui-deps`、`fmt-check`、全 package `check`/`lint`/`test`、
  `test-rt`、`check-headless-deps`、ui/app-webview/app-native/
  app-gpui 各 lane、`lint-recording-real-input`、`prek`。
- 実機 lane（`test-recording-real-input`・実 OS GUI・実
  plugin/VST3・両 consumer 横断）は opt-in/別環境のため
  `NotRun`/`Unsupported` として記録。
- 結果の正本は `docs/impl/ir/sv-28-ruviform-pin-ac10a2f7.md`
  の Validation 節。
