---
id: "SPEC-5mj8EIwK50gAS6F6xk87Ua"
type: "spec"
status: "approved"
title: "SV#28 Ruviform pin 更新 — abb9f372 → cbcc8343 と API 追従評価"
created: "2026-11-16"
updated: "2026-11-16"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/28"
  changes:
    added:
      - type: "file"
        path: "docs/discovery/sv28-ruviform-pin-cbcc8343.md"
      - type: "file"
        path: "docs/specs/sv28-ruviform-pin-cbcc8343.md"
      - type: "file"
        path: "docs/designs/sv28-ruviform-pin-cbcc8343.md"
      - type: "file"
        path: "docs/plans/sv28-ruviform-pin-cbcc8343.md"
      - type: "file"
        path: "docs/test-specs/sv28-ruviform-pin-cbcc8343.md"
      - type: "file"
        path: "docs/tasks/sv28-ruviform-pin-cbcc8343.md"
      - type: "file"
        path: "docs/impl/ir/sv-28-ruviform-pin-cbcc8343.md"
      - type: "file"
        path: "evidence/sv28/ruviform-cbcc8343-diff-review.md"
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
    - "docs/designs/sv28-ruviform-pin-cbcc8343.md"
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/tasks/sv-gui-02-ruviform-rev-rust-syn-ui-op.md"
    - "docs/tasks/sv28-ruviform-pin-update.md"
    - "docs/tasks/sv28-ruviform-pin-ac10a2f7.md"
    - "docs/tasks/sv28-ruviform-pin-b02942c2.md"
    - "docs/tasks/sv28-ruviform-pin-fd1d4d5f.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/discovery/sv28-ruviform-pin-cbcc8343.md"
  derived-by:
    - "docs/designs/sv28-ruviform-pin-cbcc8343.md"
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
    - "tools/check-headless-deps.mjs"
    - "evidence/sv28/ruviform-cbcc8343-diff-review.md"
  defers: []
  deferred-by: []
---

# SV#28 Ruviform pin 更新 — abb9f372 → cbcc8343 と API 追従評価

## What Should Be Built

synvelis workspace の `Cargo.lock` に解決される 28 の `ruviform-*`
package source を `abb9f372…` から dispatch 時点の ruviform
`main` HEAD `cbcc83437616fdd7418da37183c6be66b6cfe225` へ更新し、
`ui/vendor/ruviform` を同 revision で再生成する。あわせて
`abb9f372..cbcc8343`（12 commits）の upstream 差分を精査し、
公開 API の破壊的変更が synvelis の消費面に影響するかを評価して
記録する。

## Why Now

Issue #28 は Ruviform の継続追従 tracker であり、pin は
`Cargo.lock` のみが正本。前回 pin `abb9f372` 以降に upstream は
12 commits 前進し、Issue #628 close-out（RunMetadata JSON・
Cancellation fault）・Issue #629 4th slice（endpoint probe）・
Issue #630/#631/#532（test plugin / dock test 基盤）を含む。
consumer 影響を評価した上で pin を追従させ、互換性の証拠を残す
必要がある。`scarecrow173/audio-lens#127` が同一 `cbcc8343` で
rev-watch slice を実施中であり、candidate tuple の整合が求められる。

## Users and Value

- synvelis 開発者: 最新の Ruviform 修正（shm event record codec・
  audio-test Cancellation fault・RunMetadata JSON 等）を取り込み、
  upstream との乖離を最小化する。
- Issue #28 tracker: 各 pin slice の差分評価と検証証拠が
  canonical doc chain として記録され、次回追従の基準になる。

## Scope

### In Scope

- `Cargo.lock` の 28 `ruviform-*` package source を
  `cbcc83437616fdd7418da37183c6be66b6cfe225` へ更新
  （`cargo update -p` scoped 操作）。
- `mise run sync-ui-deps` による `ui/vendor/ruviform` 再生成と
  `check-ruviform-rev --require-vendor` による lockfile/vendor
  整合検証。
- `abb9f372..cbcc8343` の upstream 差分精査（crate 別・
  破壊的 API・consumer 影響・新規機能の採用判断）。
- canonical 7-doc chain + evidence メモ。
- POLICIES.md の canonical 検証 lane の実行と結果記録
  （実機/実 GUI/実 plugin lane は opt-in・NotRun の正直な記録）。
- docs index（`docs/**/README.md`）は本 slice では再生成しない
  （Coordinator が統合時に実施）。

### Out of Scope

- 新規 upstream 機能の採用: `RunMetadata` JSON API・
  `StreamFault::Cancellation`・`encode/decode_event_record`・
  `ruviform-plugin-test-{vst3,clap}` 新 fixture・
  `ruviform-plugin-dock-test` 拡張・endpoint_probe/
  virtual_endpoint — 全て別 slice の判断事項。
- `Cargo.toml` の依存・feature 宣言変更（不要と評価）。
- synvelis 側の API 追従修正（消費面の破壊的変更 0 件と評価。
  検証で判明した場合は最小修正を別途実施）。
- `crates/synvelis-workspace` を含むリース外 crate の修正
  （`scarecrow173/synvelis#488` Worker が占有中）。
- 実機録音・実 OS GUI・実 plugin/VST3・両 consumer 横断検証
  （opt-in lane・NotRun）。
- Issue #28 の close（tracker は継続。`Refs #28` のみ）。
- `*-msrv`/`*-full` lane（ユーザー指定により未実施）。
- docs index 再生成（`doc-maintenance` `index apply` /
  `sync_impl_index`）— Coordinator が統合時に実施。

## Requirements

- R1: `Cargo.lock` の全 28 `ruviform-*` source が単一 SHA
  `cbcc83437616fdd7418da37183c6be66b6cfe225` を指し、
  `check-ruviform-rev --require-vendor` が REV match を返す。
- R2: lockfile 差分は ruviform source fragment +
  `ruviform-audio-test` の `serde`/`serde_json` 依存追加のみで、
  無関係な crates.io version drift・新規 package を含まない。
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
  `resolved cbcc83437616fdd7418da37183c6be66b6cfe225 (28
  lockfile sources, lockfile-primary, REV match)` を出力。
- AC2: `git diff Cargo.lock` が source fragment 更新 +
  `ruviform-audio-test` 依存追加のみで、他の差分を含まない。
- AC3: `cargo check --workspace --all-targets` 相当の全
  package check/lint/test lane が exit 0。
- AC4: 7-doc chain が front-matter relation 規約に従い作成
  される（index 再生成は Coordinator 委譲）。
- AC5: PR が `Refs #28` を含み `Closes #28` を含まず、
  検証結果・NotRun・review-protocol 結果を正直に記載する。

## Deferred Design Concerns

- `RunMetadata` JSON API・`StreamFault::Cancellation` は
  additive だが未採用。rf628 oracle の metadata 永続化・
  open-fault 経路への組込みは別 slice の判断事項。
- `wire::encode_event_record`/`decode_event_record` は
  plugin wire 固定 stride codec で synvelis 未使用面。
- `ruviform-plugin-test-{vst3,clap}` 新 fixture・
  `ruviform-plugin-dock-test` 拡張・endpoint_probe は
  GUI-L2/EXT-L3・実 ABI Test Plugin・OS 仮想 Endpoint 受入の
  別 slice 判断事項。
- `scarecrow173/synvelis#488`（`LibraryAction` CI 修正）由来の
  既存 fail が出た場合は pin 非起因として記録し、当該 crate は
  修正しない。

## Sources and Relations

- Issue #28（Ruviform 継続追従 tracker）。
- discovery: `docs/discovery/sv28-ruviform-pin-cbcc8343.md`。
- 差分精査メモ:
  `evidence/sv28/ruviform-cbcc8343-diff-review.md`。
- pin 更新規約: `docs/tasks/sv-gui-02-ruviform-rev-rust-syn-ui-op.md`。
