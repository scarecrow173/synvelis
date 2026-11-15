---
id: "SPEC-xsFxwBkyqhXpEtoPXGAuah"
type: "spec"
status: "approved"
title: "SV#28 Ruviform pin 更新 — fd1d4d5f → ac10a2f7 と API 追従評価"
created: "2026-11-15"
updated: "2026-11-15"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/28"
  changes:
    added:
      - type: "file"
        path: "docs/discovery/sv28-ruviform-pin-ac10a2f7.md"
      - type: "file"
        path: "docs/specs/sv28-ruviform-pin-ac10a2f7.md"
      - type: "file"
        path: "docs/designs/sv28-ruviform-pin-ac10a2f7.md"
      - type: "file"
        path: "docs/plans/sv28-ruviform-pin-ac10a2f7.md"
      - type: "file"
        path: "docs/test-specs/sv28-ruviform-pin-ac10a2f7.md"
      - type: "file"
        path: "docs/tasks/sv28-ruviform-pin-ac10a2f7.md"
      - type: "file"
        path: "docs/impl/ir/sv-28-ruviform-pin-ac10a2f7.md"
      - type: "file"
        path: "evidence/sv28/ruviform-ac10a2f7-diff-review.md"
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
    - "docs/designs/sv28-ruviform-pin-ac10a2f7.md"
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/tasks/sv-gui-02-ruviform-rev-rust-syn-ui-op.md"
    - "docs/tasks/sv28-ruviform-pin-update.md"
    - "docs/tasks/sv28-ruviform-pin-b02942c2.md"
    - "docs/tasks/sv28-ruviform-pin-fd1d4d5f.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/discovery/sv28-ruviform-pin-ac10a2f7.md"
  derived-by:
    - "docs/designs/sv28-ruviform-pin-ac10a2f7.md"
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
    - "tools/check-headless-deps.mjs"
    - "evidence/sv28/ruviform-ac10a2f7-diff-review.md"
  defers: []
  deferred-by: []
---

# SV#28 Ruviform pin 更新 — fd1d4d5f → ac10a2f7 と API 追従評価

## What Should Be Built

synvelis workspace の `Cargo.lock` に解決される 27 の `ruviform-*`
package source を `fd1d4d5f…` から dispatch 時点の ruviform
`main` HEAD `ac10a2f7761c08db73aebd861c0a5c1f53f269b7` へ更新し、
`ui/vendor/ruviform` を同 revision で再生成する。あわせて
`fd1d4d5f..ac10a2f7`（128 commits）の upstream 差分を精査し、
公開 API の破壊的変更が synvelis の消費面に影響するかを評価して
記録する。

## Why Now

Issue #28 は Ruviform の継続追従 tracker であり、pin は
`Cargo.lock` のみが正本。前回 pin `fd1d4d5f` 以降に upstream は
128 commits 前進し、ParentLease 語彙移行・wire v5・MIDI
event contract 修正など破壊的変更を含む。consumer 影響を
評価した上で pin を追従させ、互換性の証拠を残す必要がある。

## Users and Value

- synvelis 開発者: 最新の Ruviform 修正（MIDI delivery contract、
  WASAPI permission 分類、bridge note-state ledger 等）を
  取り込み、upstream との乖離を最小化する。
- Issue #28 tracker: 各 pin slice の差分評価と検証証拠が
  canonical doc chain として記録され、次回追従の基準になる。

## Scope

### In Scope

- `Cargo.lock` の 27 `ruviform-*` package source を
  `ac10a2f7761c08db73aebd861c0a5c1f53f269b7` へ更新
  （`cargo update -p` scoped 操作）。
- `mise run sync-ui-deps` による `ui/vendor/ruviform` 再生成と
  `check-ruviform-rev --require-vendor` による lockfile/vendor
  整合検証。
- `fd1d4d5f..ac10a2f7` の upstream 差分精査（crate 別・
  破壊的 API・consumer 影響・新規機能の採用判断）。
- canonical 7-doc chain + evidence メモ + index 再生成。
- POLICIES.md の canonical 検証 lane の実行と結果記録
  （実機/実 GUI/実 plugin lane は opt-in・NotRun の正直な記録）。

### Out of Scope

- 新規 upstream 機能の採用: `ruviform-plugin-dock-test`、
  `ruviform-plugin-test-vst3`、`ruviform-audio-test`、
  `ruviform-io-e2e` lane、`uitest` feature、`plugin_dock`
  scenario/`PluginHostProbe`、`fault-injection` feature、
  endpoint probe examples — 全て別 slice の判断事項。
- `Cargo.toml` の依存・feature 宣言変更（不要と評価）。
- synvelis 側の API 追従修正（消費面の破壊的変更 0 件と評価。
  検証で判明した場合は最小修正を別途実施）。
- 実機録音・実 OS GUI・実 plugin/VST3・両 consumer 横断検証
  （opt-in lane・NotRun）。
- Issue #28 の close（tracker は継続。`Refs #28` のみ）。
- `*-msrv`/`*-full` lane（ユーザー指定により未実施）。

## Requirements

- R1: `Cargo.lock` の全 27 `ruviform-*` source が単一 SHA
  `ac10a2f7761c08db73aebd861c0a5c1f53f269b7` を指し、
  `check-ruviform-rev --require-vendor` が REV match を返す。
- R2: lockfile 差分は ruviform source fragment のみで、
  無関係な crates.io version drift・新規 package・依存リスト
  変動を含まない。
- R3: `ui/vendor/ruviform` が `mise run sync-ui-deps` で
  再生成され、REV が lockfile と一致する。
- R4: upstream 差分の crate 別影響評価・破壊的 API の
  consumer 影響評価・新規機能の採用/不採用判断が
  discovery/design doc として記録される。
- R5: POLICIES.md の canonical lane（fmt-check・check・
  lint・test・test-rt・headless-deps・ui・app-webview・
  app-native・app-gpui・recording-real-input lint・prek）が
  実行され、結果が正直に記録される。
- R6: 実施しなかった lane は `NotRun`/`Unsupported` として
  理由付きで記録され、Pass と偽らない。

## Acceptance Criteria

- AC1: `node tools/check-ruviform-rev.mjs --require-vendor` が
  `resolved ac10a2f7761c08db73aebd861c0a5c1f53f269b7 (27
  lockfile sources, lockfile-primary, REV match)` を出力。
- AC2: `git diff Cargo.lock` が 54 行の source fragment 更新
  のみ（27 package × -/+）で、他の差分を含まない。
- AC3: `cargo check --workspace --all-targets` 相当の全
  package check/lint/test lane が exit 0。
- AC4: 7-doc chain が front-matter relation 規約に従い作成
  され、index が再生成される。
- AC5: PR が `Refs #28` を含み `Closes #28` を含まず、
  検証結果・NotRun・review-protocol 結果を正直に記載する。

## Deferred Design Concerns

- ParentLease/wire v5/MIDI contract 修正は synvelis 未使用面。
  将来 `Plugin` trait 実装・`open_gui`・wire command を使う
  場合は `ParentLease` 語彙への移行が必要（記録のみ）。
- `ruviform-ui-gpui` `uitest`・`plugin_dock` scenario 等の
  GUI 試験基盤採用は GUI-L2/EXT-L3 受入 slice の判断事項。
- `ruviform-io-file` `fault-injection`・`ruviform-io-e2e`
  lane の採用は SYN-IO 試験要件確定時。
- `ruviform-{audio,midi}-device` endpoint probe examples は
  library API ではなく example binary。採用判断は別途。

## Sources and Relations

- Issue #28（Ruviform 継続追従 tracker）。
- discovery: `docs/discovery/sv28-ruviform-pin-ac10a2f7.md`。
- 差分精査メモ:
  `evidence/sv28/ruviform-ac10a2f7-diff-review.md`。
- pin 更新規約: `docs/tasks/sv-gui-02-ruviform-rev-rust-syn-ui-op.md`。
