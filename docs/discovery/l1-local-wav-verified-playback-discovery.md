---
id: "DISC-99vL2v7VILngzJrF9fbNos"
type: "discovery"
status: "resolved"
title: "L1 Discovery — Local WAV Safe Import・Verified Media・Headless Generic Graph Playback"
created: "2026-11-03"
updated: "2026-11-03"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/24"
    - "https://github.com/scarecrow173/synvelis/issues/17"
    - "https://github.com/scarecrow173/synvelis/issues/18"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on:
    - "docs/specs/vs-d-recording-commit-e2e-idempotency-undo-reload.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
---

# L1 Discovery — Local WAV Safe Import / Verified Media / Headless Graph Playback

## Framing

Issue #24 は Synvelis 初の Player E2E: ローカル WAV ファイルを安全にインポートし、
検証済み Source として不変の Verified Media に実体化し、Track/Clip 配置を Generic
Execution Graph へ lower して決定的 headless 再生を検証する縦断スライス。

非対象: DAW UI・Remote Provider・Mixer/Plugin・録音 Writer・WAV 出力 (rf#191)。

## 既存資産の棚卸し (main マージ済み)

- VS-A (`synvelis-project`): `Project` 集約・`ProjectController` 単一 writer・
  `Command { expected_revision, op }`・`Operation { validate, apply }`・
  `ProjectStore`/`DirectoryBundleStore`・安定 ID (`SourceId`/`AssetId`/`ClipId` 別型)・
  `SourceState Imported→Verified→Committed→Saved`・`AddClip`/`RegisterAsset`/`RegisterSource`/`SetSourceState` op・
  `undo`/`redo`/`save`/`load`。
- VS-B/C/D (`synvelis-recording`): staging (`publish_atomic`/`sync_parent_dir`/`detect_dir_sync`)・
  journal/recovery・`apply_commit` (manifest → 検証 → 単一 atomic apply → applied registry)・
  `crc32`/`digest_fold`・faultinj。
- Ruviform (`Cargo.lock` pin 更新対象 `c2a2f07`): `ruviform-media-reader`
  (`SourceReader`/`Prefetch`/`FrameMap`/`FillReport`/`SourceFault`)・`ruviform-graph`
  (`GraphBuilder`/`Validator`/`StableHandleMap`)・`ruviform-audio`
  (`Compiler`/`PreparedPlan`/`GraphRuntime`/`RtRuntime`/last-good publish)・
  `ruviform-media-io` + `ruviform-io-symphonia` (WAV/PCM demux/decode)・
  `ruviform-rt` (SPSC/slot/Retirer/`RtContext`)。

## ギャップ分析

| 要求 | 既存 | 不足 |
| --- | --- | --- |
| 未検証 Path ≠ Source | SourceState 機械 | Import パイプライン本体 (staging→verify→blob→commit) |
| Import 冪等 | `applied_commits` (録音 session 限定) | `applied_imports` レジストリ |
| Mute | なし | `Clip.muted` + `SetClipMuted` op |
| Verified Resolver | なし | Source→blob→reader/prefetch、型付き失敗 |
| Timeline lowering | `MediaSpan`/`FrameMap` 型 | Track/Clip→node/port 写像・mute・rate 整合判定 |
| Headless 再生 | `RtRuntime`/`PrefetchConsumer` | deterministic driver (pump 制御・seek/stop・EOF/underrun 計測) |
| Fixture | なし | 決定的 PCM WAV 生成 (mono/stereo、値再現可能) |

## 依存 Revision 決定

`AGENTS.md` は Ruviform を既定ブランチ追従 git 依存 + `Cargo.lock` pin と定める。
本 Issue の前提 PR #225/#227/#231 は pin `8ad88f5` 以後の `c2a2f07` で利用可能となるため、
新規依存追加とともに lock を再生成する (浮動 SHA ハードコード禁止のため lock pin が正本)。

## 代替案の検討

- Import を `synvelis-recording` に置く案: 録音 session 前提の型に引きずられ非対称。
  `synvelis-player` 新設 + `synvelis-recording` の staging/sync ヘルパ再利用を選択。
- Boundary-Source ノード方式 (driver が `input_writer` で供給): ClipPlayer
  `GraphProcessor` 方式を選択 — Install/Remove・last-good・per-node fault の
  実モデルに一致し、underrun/EOF をノード単位で観測できる。
