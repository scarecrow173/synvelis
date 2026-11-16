---
id: "DISC-0rH6upYc1dViYzZVllA0t1"
type: "discovery"
status: "resolved"
title: "SV#28 Ruviform pin 更新 — abb9f372 → cbcc8343 差分調査と API 互換性評価"
created: "2026-11-16"
updated: "2026-11-16"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/28"
  changes:
    added:
      - type: "file"
        path: "evidence/sv28/ruviform-cbcc8343-diff-review.md"
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by:
    - "docs/specs/sv28-ruviform-pin-cbcc8343.md"
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/tasks/sv28-ruviform-pin-update.md"
    - "docs/tasks/sv28-ruviform-pin-ac10a2f7.md"
    - "docs/tasks/sv28-ruviform-pin-b02942c2.md"
    - "docs/tasks/sv28-ruviform-pin-fd1d4d5f.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by:
    - "docs/specs/sv28-ruviform-pin-cbcc8343.md"
    - "docs/designs/sv28-ruviform-pin-cbcc8343.md"
    - "docs/plans/sv28-ruviform-pin-cbcc8343.md"
  verifies: []
  verified-by: []
  references:
    - "Cargo.lock"
    - "Cargo.toml"
    - "tools/check-ruviform-rev.mjs"
    - "tools/sync-ruviform-ui.mjs"
    - "tools/check-headless-deps.mjs"
    - "evidence/sv28/ruviform-cbcc8343-diff-review.md"
  defers: []
  deferred-by: []
---

# SV#28 Ruviform pin 更新 — abb9f372 → cbcc8343 差分調査

## Summary

現行 pin `abb9f372d3ee95ee824b3c4f7dc68e8083ce2774`（PR #486
`ac5200c` の付随更新で導入。専用 doc chain なし）から ruviform
`main` HEAD `cbcc83437616fdd7418da37183c6be66b6cfe225` までの
**12 commits** を調査した。`crates/` 配下の diff は 10 crate・
57 files・+17,074/−558 と大規模だが、その大半は
dep-tree-external の test plugin / dock test 基盤
（Issue #630/#631/#532 slice）。synvelis 依存木内で差分があるのは
4 crate（`ruviform-audio-device`・`ruviform-audio-test`・
`ruviform-plugin-core`・`ruviform-plugin-host`）のみで、全て
**additive または内部修正** — `pub` item の削除行スキャンは 0 件、
公開 API の破壊的変更は検出されなかった。`ui/` 差分は 0 件。
精査メモは `evidence/sv28/ruviform-cbcc8343-diff-review.md` に
記録した。

## Exploration Goal

pin 更新で consumer（synvelis）が踏む API 差分を特定し、追従修正の
要否と範囲を確定する。今回初めて依存木内に差分が出た dev-dep
`ruviform-audio-test`（RunMetadata JSON・`StreamFault::Cancellation`
・serde derive 追加）が既存の rf628 oracle
（`tests/rf628_oracle.rs`・`rf628_oracle_rt_alloc.rs`）と整合するか、
および `plugin-core` の shm event record codec 追加・
`plugin-host` の `l1/data.rs` 内部修正が消費面に影響するかを評価する。

## Findings

### 差分に含まれる主な変更（abb9f372..cbcc8343、12 commits）

| 領域 | upstream 変更 | consumer 影響 |
| --- | --- | --- |
| `ruviform-audio-test` | `RunMetadata`/`RunParameters`/`JitterSpec`/`VirtualClockConfig`/`CaseRequirement`/`CaseResult`/`VerificationLevel`/`EnvironmentKind` に serde derive、`RunMetadata::{to_json,from_json,save_json,load_json}` 追加（Issue #628 close-out `0b510cf8`）、`StreamFault::Cancellation` variant 追加 + backend pump 実装、`serde`/`serde_json` dep 追加 | **additive のみ**。`StreamFault` は `#[non_exhaustive]` で synvelis は variant 構築のみ。synvelis 消費 API（`DeviceSpec`/`InputSpec`/`ScriptedBackend`/`ScriptedDeviceHandle`/`StreamScript`/`StreamDirection`/`PumpReport`/`clock`/`oracle`/`pcm`）は不変 |
| `ruviform-plugin-core` | `src/wire.rs` に `encode_event_record`/`decode_event_record` 追加（shm slot 固定 stride `EVENT_WIRE_SIZE` の record codec。Issue #630 adapter state bridge `e65b8014` 系） | additive。新規 pub fn 2 件のみ。synvelis は wire 層を直接使わない |
| `ruviform-plugin-host` | `src/l1/data.rs` 1 行: `RtLeg` の shm event 書き込みを `encode_event` → `encode_event_record` へ（固定 stride 前提との不整合修正） | なし。`l1` は内部 module。公開 API（`HostCommand`/`HostReply`/`PluginHandle`/`PluginHost` 等）不変 |
| `ruviform-audio-device` | `examples/endpoint_probe.rs` +283、`tests/virtual_endpoint.rs` +1371（Issue #629 4th slice `6a481425`）。**`src/` 差分 0 件** | なし。lib 非影響。`synvelis-recording` `real-device` opt-in lane の依存 crate だが公開面不変 |
| dep-tree-external | `ruviform-plugin-clap`（host.rs +393）、`ruviform-plugin-export`（lib.rs +7・`decode_event_record` 適応）、`ruviform-plugin-vst3`（host.rs +1114・state verb `kNotImplemented` 正直化）、`ruviform-plugin-dock-test`（real_dock/ospix/plugin-dock-lab・dcomp-pixel regression）、`ruviform-plugin-test-clap`/`test-vst3`（test_editor/test_stateful/test_instrument fixture） | なし。synvelis lockfile 非含有 |
| `ui/`（TS） | 差分 0 件 | なし。vendor 再生成は REV bump のみ |

### 互換性評価

- `git diff abb9f372..cbcc8343 -- crates/` の削除行 615 行に
  `pub` を含む行は **0 件**。公開 item の削除・rename・
  signature 変更なし —— 前回 ac10a2f7 slice と異なり
  **破壊的変更を含まない純粋 additive 範囲**。
- `ruviform-audio-test` の新 API は全て additive:
  serde derive は trait impl 追加、`RunMetadata` JSON メソッドは
  新規 method、`StreamFault::Cancellation` は `#[non_exhaustive]`
  enum への variant 追加。既存 variant・構造体 field・関数
  signature の変更なし。
- `Cargo.lock` 差分は 28 package の source fragment 更新 +
  `ruviform-audio-test` dependencies への `serde`/`serde_json`
  2 行追加のみ（両 crate は lockfile 既存:
  serde 1.0.229・serde_json 1.0.154）。新規 package・version
  drift なし。ruviform package 数 28 → 28。

### 影響表（feature/schema/project/graph/RT/plugin/GUI/recorder/
automation/export）

| 軸 | 評価 |
| --- | --- |
| feature | 新規 feature なし。`ruviform-audio-test` の serde/serde_json は通常 dep（optional でない）だが test-only crate であり synvelis では dev-dep 経由のみ |
| schema | 永続化 format・Project schema の変更なし。`RunMetadata` JSON は Case Schema v1 `kebab-case` 語彙の serialize 追加（test-only・非 RT API） |
| project | `ruviform-graph`/`media-*`/`io-*` 無変更 |
| graph | `ruviform-graph` 無変更 |
| RT | `ruviform-rt` 無変更。`audio-test` の serde derive は RT 経路非呼出と upstream が明記。`plugin-host` `l1/data.rs` は shm 書き込みの record codec 修正で bounded/no-alloc 維持 |
| plugin | `plugin-core` additive（record codec）。`plugin-host` 内部修正のみ。`plugin-export`/`plugin-vst3`/`plugin-clap` は dep-tree-external。synvelis 消費の plugin API 不変 |
| GUI | `ui/` 差分 0。dock-test/test-plugin の新 fixture は dep-tree-external で未採用 |
| recorder | additive。`audio-device` lib 不変。`audio-test` の `Cancellation` fault・RunMetadata JSON は既存 rf628 oracle 消費面と非衝突 |
| automation | 実質 diff なし |
| export | `ruviform-io-wav`/`io-symphonia`/`media-*` 無変更 |

### 新 API / 新 crate の採用判断

全て **今回は採用しない**（詳細は design doc の判断表）:

- `RunMetadata` JSON メソッド・serde derive・`StreamFault::
  Cancellation`: additive だが synvelis の rf628 oracle は
  `RunMetadata` 未消費・`Cancellation` 未使用。採用は test
  runtime 運用・open-fault oracle 拡張の別 slice 判断事項。
- `wire::encode_event_record`/`decode_event_record`: plugin wire
  固定 stride codec。synvelis は wire 層を直接使わない。
- `ruviform-plugin-test-{vst3,clap}` 新 fixture
  （test_editor/test_stateful/test_instrument）・
  `ruviform-plugin-dock-test`（real_dock/ospix/plugin-dock-lab）・
  `ruviform-audio-device` endpoint_probe 拡張/
  `virtual_endpoint` test: dep-tree-external または
  example/test のみ。GUI-L2/EXT-L3・実 ABI Test Plugin・OS 仮想
  Endpoint 受入の別 slice 判断事項。

## Open Questions

- なし。pin 更新は `cargo update -p ruviform-*`（全 28 package を
  `-p` 指定）で `main` HEAD を解決し、`check-ruviform-rev` の
  単一 SHA 契約で検証する。

## Sources and Relations

- Issue #28（Ruviform 継続追従 tracker。managed コメントは
  編集しない）。
- 先行 pin 更新: PR #193（`cc7d5296`）、PR #301（`b02942c2`）、
  PR #329（`fd1d4d5f`）、PR #475（`ac10a2f7`）、および
  `abb9f372` pin（PR #486 `ac5200c` 付随更新・専用 chain なし）。
- pin 更新規約: `docs/tasks/sv-gui-02-ruviform-rev-rust-syn-ui-op.md`、
  `tools/check-ruviform-rev.mjs`（Cargo.lock が唯一の正本）。
- 差分精査メモ:
  `evidence/sv28/ruviform-cbcc8343-diff-review.md`。
- ruviform 差分範囲: `abb9f372..cbcc8343`（12 commits、
  ruviform repo の `git log`/`git diff` で独立確認）。
- candidate tuple: `scarecrow173/ruviform#294`（候補 SHA tuple
  検証）・`scarecrow173/audio-lens#127`（AL 側 rev-watch slice、
  同一 `cbcc8343`）。

## Evidence

- ruviform local repo: `git rev-parse origin/main` →
  `cbcc83437616fdd7418da37183c6be66b6cfe225`（2026-11-16 確認。
  spec 起稿時点 candidate と一致・以後の前進なし）。
- `git merge-base --is-ancestor abb9f372 cbcc8343` → true。
- `git rev-list --count abb9f372..cbcc8343` → 12。
- `git diff --stat abb9f372..cbcc8343 -- crates/` →
  57 files・+17,074/−558（変更 crate dir 10 + README）。
- `pub` 削除行スキャン: 削除行 615 行中 `pub` 含有 0 件。
- `ui/` 差分: 0 件。
- `Cargo.lock` 差分（本 worktree、`cargo update -p` 実施済み）:
  30 insertions / 28 deletions — 28 package の source fragment
  更新 + `ruviform-audio-test` への `serde`/`serde_json` 依存
  2 行。他の依存リスト・version・新規 package の変動なし。
- `node tools/check-ruviform-rev.mjs --require-vendor`:
  `resolved cbcc83437616fdd7418da37183c6be66b6cfe225 (28 lockfile
  sources, lockfile-primary, REV match)`。
- `mise run sync-ui-deps`: `synced ruviform ui/* @
  cbcc834376 -> ui/vendor/ruviform`。
