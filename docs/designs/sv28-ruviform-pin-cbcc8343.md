---
id: "DESIGN-2z5KtdGhRdUvPQ0fJobGHe"
type: "design"
status: "approved"
title: "SV#28 Ruviform pin 更新 設計 — lockfile pin 更新と API 追従の境界（abb9f372 → cbcc8343）"
created: "2026-11-16"
updated: "2026-11-16"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/28"
  changes:
    added:
      - type: "file"
        path: "docs/designs/sv28-ruviform-pin-cbcc8343.md"
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated:
      - type: "file"
        path: "ui/vendor/ruviform"
  implements:
    - "docs/specs/sv28-ruviform-pin-cbcc8343.md"
  implemented-by: []
  depends-on:
    - "docs/specs/sv28-ruviform-pin-cbcc8343.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/tasks/sv-gui-02-ruviform-rev-rust-syn-ui-op.md"
    - "docs/tasks/sv28-ruviform-pin-ac10a2f7.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/sv28-ruviform-pin-cbcc8343.md"
    - "docs/discovery/sv28-ruviform-pin-cbcc8343.md"
  derived-by:
    - "docs/plans/sv28-ruviform-pin-cbcc8343.md"
    - "docs/test-specs/sv28-ruviform-pin-cbcc8343.md"
    - "docs/tasks/sv28-ruviform-pin-cbcc8343.md"
  verifies: []
  verified-by:
    - "docs/test-specs/sv28-ruviform-pin-cbcc8343.md"
  references:
    - "Cargo.toml"
    - "Cargo.lock"
    - "tools/check-ruviform-rev.mjs"
    - "tools/sync-ruviform-ui.mjs"
    - "evidence/sv28/ruviform-cbcc8343-diff-review.md"
  defers: []
  deferred-by: []
---

# SV#28 Ruviform pin 更新 設計

## 概要

`Cargo.lock` の ruviform 解決 revision を `cbcc8343` へ更新し、
upstream 差分（`abb9f372..cbcc8343`、12 commits）の consumer
影響を評価する。synvelis の消費面に破壊的変更がないため、
変更は lockfile 更新 + vendor 再生成 + doc chain のみで、
製品コードの変更は行わない設計とする。

## Context

- pin の正本は `Cargo.lock` のみ（`Cargo.toml` の git 依存は
  `rev` 未指定で `main` を追従）。`check-ruviform-rev.mjs` が
  lockfile/vendor の単一 SHA 契約を検証する。
- 前回 pin `abb9f372` 以降の upstream 差分は 57 files・
  +17,074/−558 と大規模だが、大半は dep-tree-external の
  test plugin / dock test 基盤（Issue #630/#631/#532）。
  依存木内の差分は 4 crate のみで全て additive/内部修正
  （`pub` 削除行スキャン 0 件）。
- `ruviform-audio-test` が dev-dep として依存木入りして以降
  初めて同 crate に差分が出た。新 API（RunMetadata JSON・
  `StreamFault::Cancellation`）は additive で既存 rf628
  oracle の消費面と非衝突。
- 新規 upstream 機能は全て dep-tree-external・opt-in・
  test 基盤であり、本 slice のスコープ外。

## Scope

### In Scope

- `cargo update -p ruviform-*`（28 package 全指定）による
  lockfile 更新と差分検査。
- `mise run sync-ui-deps` による vendor 再生成と
  `check-ruviform-rev --require-vendor` 検証。
- canonical 検証 lane の実行と正直な結果記録。
- 7-doc chain + evidence メモ（index 再生成は Coordinator
  委譲）。

### Out of Scope

- 新規 upstream 機能の採用（下記「新 API 採用判断」参照）。
- `Cargo.toml` 変更、製品コード変更、リース外 crate の修正。
- 実機/実 GUI/実 plugin opt-in lane の実行。
- docs index 再生成。

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
  が lockfile 28 source の単一 SHA と vendor REV の一致を
  機械検証する。

## Data and Control Flow

```
origin/main (ruviform) @ cbcc8343
  → cargo update -p ruviform-* (28 packages)
  → Cargo.lock source fragments → cbcc8343…
     (+ ruviform-audio-test deps: serde, serde_json)
  → mise run sync-ui-deps → ui/vendor/ruviform @ cbcc8343
  → check-ruviform-rev --require-vendor → REV match
  → mise run <lane> × N → 結果を impl/ir doc に記録
```

## 差分調査結果（abb9f372..cbcc8343）

12 commits・`crates/` 57 files・+17,074/−558。crate 別の
consumer 影響評価は `evidence/sv28/ruviform-cbcc8343-diff-review.md`
と discovery doc を正本とする。要点:

- **公開 API の破壊的変更なし（純粋 additive 範囲）**:
  削除行 615 行中 `pub` 含有 0 件。
- **依存木内 4 crate の差分は全て additive/内部修正**:
  - `ruviform-audio-test`: serde derive + `RunMetadata` JSON
    メソッド + `StreamFault::Cancellation` variant 追加
    （`#[non_exhaustive]` enum）。既存消費 API 不変。
  - `ruviform-plugin-core`: `wire.rs` に
    `encode_event_record`/`decode_event_record` 追加。
  - `ruviform-plugin-host`: `l1/data.rs` の RtLeg が shm event
    固定 stride record codec を使う内部修正（公開 API 不変）。
  - `ruviform-audio-device`: `examples/`・`tests/` のみ、
    `src/` 差分 0 件。
- **dep-tree-external の大規模差分**: `plugin-test-vst3`/
  `test-clap`（test_editor/test_stateful/test_instrument
  fixture）、`plugin-dock-test`（real_dock/ospix/plugin-dock-lab
  ・dcomp-pixel regression）、`plugin-vst3`/`plugin-clap` host
  拡張、`plugin-export` の record codec 適応 — synvelis
  lockfile 非含有で非影響。
- **`ui/` 差分 0 件**: vendor 再生成は REV bump のみ。

## 設計判断

| 判断 | 内容 | 根拠 |
| --- | --- | --- |
| lockfile のみ更新 | `Cargo.toml` の git 依存は `rev` 未指定のまま維持し、`cargo update -p` で lockfile を更新 | 既存 pin 規約（`check-ruviform-rev` が lockfile-primary を要求）。`rev` 固定は行わない |
| scoped `cargo update -p` | 28 package 全てを `-p` 指定して ruviform のみ更新 | 無関係な crates.io drift を排除。lockfile 差分は source fragment + audio-test 依存追加のみ |
| vendor は生成物 | `mise run sync-ui-deps` で再生成し手編集しない | 既存規約。`--require-vendor` で lockfile との一致を検証 |
| 製品コード変更なし | 消費面の破壊的変更 0 件のため `crates/` 配下は不変 | `pub` 削除スキャン 0 件 + `check --all-targets` で実証 |
| 新 API 不採用 | RunMetadata JSON・Cancellation fault・event record codec・test plugin fixture・dock-test 拡張・endpoint probe を全て不採用 | additive/dep-tree-external/opt-in/test 基盤。採用は GUI-L2/EXT-L3・Test Runtime・IO 試験要件の別 slice の判断事項（YAGNI） |
| リース境界の遵守 | `crates/synvelis-workspace`（`scarecrow173/synvelis#488` 占有中）を含むリース外 crate は pin 起因の破壊が出ても修正しない | Worker 共通契約の HARD BOUNDARY。破壊時は記録・起票・報告で対応 |

## 差分影響評価表（軸別）

| 軸 | 評価 |
| --- | --- |
| feature | 新規 feature なし。`ruviform-audio-test` の serde/serde_json は通常 dep 追加（test-only crate・synvelis では dev-dep 経由のみ） |
| schema | 永続化 format・Project schema の変更なし。`RunMetadata` JSON は Case Schema v1 語彙の serialize 追加（test-only・非 RT API） |
| project | `ruviform-graph`/`media-*`/`io-*` 無変更 |
| graph | `ruviform-graph` 無変更 |
| RT | `ruviform-rt` 無変更。audio-test serde derive は RT 経路非呼出。plugin-host `l1/data.rs` は bounded/no-alloc 維持の record codec 修正 |
| plugin | `plugin-core` additive（record codec）。`plugin-host` 内部修正のみ。dep-tree-external の host/fixture 拡張は非影響 |
| GUI | `ui/` 差分 0。dock-test/test-plugin fixture は dep-tree-external で未採用 |
| recorder | additive。`audio-device` lib 不変。`audio-test` 新 API は既存 rf628 oracle 消費面と非衝突 |
| automation | 実質 diff なし |
| export | `ruviform-io-wav`/`io-symphonia`/`media-*` 無変更 |

## Non-Functional Constraints

- `mise.toml` の task が唯一の検証入口。cargo コマンドは
  `mise run` 経由。
- lockfile 差分は ruviform source fragment + `ruviform-audio-test`
  の serde/serde_json 依存追加のみ。無関係な version drift・
  新規 package を混入させない。
- `ui/vendor/ruviform` は生成物として扱い、gitignore 済み・
  直接編集しない。
- 実機/実 GUI/実 plugin lane は opt-in であり、未実施は
  `NotRun`/`Unsupported` と正直に記録する（POLICIES.md）。

## Risks and Trade-offs

| リスク | 評価 | 対応 |
| --- | --- | --- |
| 消費面の見落としによる compile 破壊 | 低 | `pub` 削除スキャン 0 件 + `check --all-targets` 全 package で実証 |
| `ruviform-audio-test` 新 API との非互換 | 低 | additive のみ（serde derive・新規 method・`#[non_exhaustive]` variant）。rf628 oracle の `test` lane が非回帰を検証 |
| `plugin-host` `l1/data.rs` 内部修正の影響 | 低 | 公開 API 不変。shm event record の固定 stride 化は upstream の一貫性修正で、synvelis は `l1` data plane を直接消費しない |
| 大規模差分の見落とし | 中 | 57 files の精査は crate 単位 + `pub` 削除行スキャン + 依存木交差判定で実施。evidence メモに記録 |
| vendor/lockfile 不整合 | 低 | `--require-vendor` が機械検証 |
| #488 由来の既存 fail との混同 | 中 | pin 起因か既存破損かを lane 結果と upstream diff で切り分けて記録。当該 crate は修正しない |

## Verification Notes

- 機械検証: `check-ruviform-rev --require-vendor`、
  `sync-ui-deps`、`fmt-check`、全 package `check`/`lint`/`test`、
  `test-rt`、`check-headless-deps`、ui/app-webview/app-native/
  app-gpui 各 lane、`lint-recording-real-input`、`prek`。
- 実機 lane（`test-recording-real-input`・実 OS GUI・実
  plugin/VST3・両 consumer 横断）は opt-in/別環境のため
  `NotRun`/`Unsupported` として記録。
- 結果の正本は `docs/impl/ir/sv-28-ruviform-pin-cbcc8343.md`
  の Validation 節。
