---
id: "SPEC-ubzJtNuQxbSXLe7nevbdUw"
type: "spec"
status: "approved"
title: "L1 — Local WAV Safe Import・Verified Media・Track/Clip・Headless Generic Graph Playback"
created: "2026-11-03"
updated: "2026-11-03"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/24"
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
    - "docs/discovery/l1-local-wav-verified-playback-discovery.md"
    - "docs/specs/vs-d-recording-commit-e2e-idempotency-undo-reload.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/project-asset-persistence-architecture.md"
    - "docs/designs/playback-mixer-generic-execution-graph-architecture.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/discovery/l1-local-wav-verified-playback-discovery.md"
  derived-by: []
---

# L1 Spec — Local WAV Safe Import / Verified Media / Headless Graph Playback

## Purpose

Issue #24: 実ローカル PCM WAV を対象とした最初の Player E2E。
安全な検証・実体化、安定 Project 実体、決定的 headless 再生を
一段の縦断スライスで実装・実測する。

## Contracts

1. **Source/Asset/Clip 分離**: `SourceId`/`AssetId`/`ClipId` は別の安定 ID。
   複数 Clip が同一不変 Source を参照可。Project への write は
   `ProjectController` の検証済 `Command` のみ。Import 前の未検証 Path は
   正式 Source として参照させない。
2. **Safe Local Import**: `Local File → non-RT Staging →
   Size/Hash/Container/Codec/Frame/Channel Verify → OS 耐久性測定 →
   Verified Blob Materialization → ExpectedRevision Project Mutation →
   Save/Reload`。Blob 公開 Receipt と Project Commit/Saved Snapshot
   Receipt は別に保持する。
3. **Verified Resolver**: 安定 Source 参照 → Manifest/Blob →
   `SourceReader`/`Decoder`/`Prefetch` を非RT Prepare。失敗は
   Corrupt / HashMismatch / UnsupportedCodec / UnresolvedSource /
   MissingMedia を型付きで区別する。
4. **Timeline Lowering**: Track/Clip/Source Offset/Placement/Mute の
   P0 意味を安定 Node/Port 写像と Generic Audio Graph へ lower する。
   Graph に Project/Track/Clip 公開型を持ち込まない。
   SourceFrame/EngineFrame/Project beat を混同せず丸めは最終 Frame 境界のみ。
5. **Playback**: 単一→複数 Clip、Mono/Stereo、Clip 開始/末尾、
   ソース途中参照、Seek/Stop、Prefetch underrun/EOF を
   決定的 Headless Driver で検証。
6. **Compat/Failure**: Graph Prepare/Reader 失敗で Last-good または
   明示 Stopped へ。旧 Snapshot/Verified Media を破壊せず、同一
   Import/Commit 再実行で重複・破損を生まない。

## Acceptance Criteria

- **L1-01**: 小 mono/stereo WAV の Frame/Channel/Header/Hash を検証し
  Verified Source として実体化。検証前 Source は Playback 対象にならない。
- **L1-02**: SourceId/AssetId/ClipId 分離。1 Source → 2 Clip (別
  placement/offset)。Undo/Redo/Save/Reload で参照一致。
- **L1-03**: 48kHz 決定的 fixture (PCM 値が channel/frame から再構成可能)
  で Clip start/end・`[start,end)`・Seek・Block 境界跨ぎ Sample 一致。
- **L1-04**: 切断/破損/不正 Hash/Unsupported/Revision Conflict/Cancel/
  Store Error で部分 Mutation なし。Verified Blob/orphan 処理は明示。
- **L1-05**: Source frame/Placement の Off-by-one なし。Native Format/
  Rate/Time 変換条件を記録。暗黙 Resample/Quantize なし。
- **L1-06**: Ruviform Reader/Graph の Version/Capability/Latency/不足機能を
  明示。Underrun/EOF/Plan failure を成功音声と混同しない。
- **L1-07**: `cargo test --workspace --locked` と新規 Headless Fixture
  テストの実行結果・Fixture hash・exit code を記録。lock 再現可能。
- **L1-08**: 該当 EX-001〜020/PAP-001〜018/MR-001〜014 を受入テストと
  相互参照。未実装を合格と表記しない。

## Non-goals

DAW UI、Remote Provider、VCS、Multi-Plugin Mixer、Stem/Freeze/Bounce、
録音 Writer、WAV Output (rf#191)、実デバイス入出力、Sample-rate 変換
(rate 不一致は型付き skip/error で表明)、Musical→Frame の tempo 変換は
既存 TempoMap 経由の P0 範囲のみ。
