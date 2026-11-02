---
id: "DESIGN-yiV0z7HH1jyKUmweFh9hCT"
type: "design"
status: "approved"
title: "L1 Design — Safe Import・Verified Media・Lowering・Headless Driver"
created: "2026-11-03"
updated: "2026-11-03"
owners: ["scarecrow173"]
relations:
  source: []
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/l1-local-wav-verified-media-graph-playback.md"
  implemented-by: []
  depends-on:
    - "docs/designs/vs-d-recording-commit-e2e-idempotency-undo-reload.md"
    - "docs/designs/project-asset-persistence-architecture.md"
    - "docs/designs/playback-mixer-generic-execution-graph-architecture.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/l1-local-wav-verified-media-graph-playback.md"
    - "docs/adr/l1-player-slice-boundaries.md"
  derived-by: []
---

# L1 Design — Safe Import / Verified Media / Lowering / Headless Driver

## 構成

`crates/synvelis-player` (lib, 新規):

| module | 責務 |
| --- | --- |
| `fixture` | 決定的 PCM WAV 生成 (header + PCM16、値は (ch,frame) から再現) |
| `import` | Local→staging→verify→耐久性測定→blob 実体化→CommitImport→receipt |
| `resolver` | SourceId→applied blob→`SourceReader`→`Prefetch` prepare、型付き失敗 |
| `lowering` | Project→`GraphSpec`+`ClipBinding` (stable node/port 写像) |
| `processor` | `ClipPlayer: GraphProcessor` — `PrefetchConsumer` 内包・window 切り出し・統計 |
| `driver` | `HeadlessDriver` — publish/seek/stop/pump/run・出力収集・状態 |

`synvelis-project` 追加: `ImportId`、`Clip.muted`+`SetClipMuted`、
`Project::applied_imports`、`AppliedImport`。

## Import pipeline

```
local path ─copy(fsync)→ <staging>/import-<id>/source.bin
  → verify: size == staged len; digest_fold → digest; open_demuxer →
    container==Wav else Corrupt; 最初の audio stream の codec==Pcm &&
    decodable==Known else UnsupportedCodec; SourceReader::read_planar
    で実測 frame 数・channels・rate (declared と照合)
  → manifest: {import_id, asset_id, source_id, digest, byte_len,
    frames, channels, rate, durability} 書込 (blob 公開前)
  → blob: blobs/<digest> … publish_atomic + sync_parent_dir
    (既存は再 hash 照合→reuse、不一致→Corrupt)
  → controller.apply(Command{expected_revision, CommitImport})
    → CommitReceipt{revision, source_id, asset_id, import_id}
  → controller.save(store) → SavedState (別 receipt)
```

Receipt 分離: `BlobReceipt` (digest/path/byte_len/durability/materialized)
と `ProjectReceipt` (commit revision + save snapshot name)。
失敗時 orphan: blob は残るが manifest に `blob-published` が残り、
`ImportError::Orphaned { blob }` で明示、再試行は manifest から resume。

## Resolver

`resolve(project, source_id, media_root, PrefetchConfig)`:
- source 不在 → `UnresolvedSource`、state < Verified → `UnverifiedSource`
- asset 不在 → `MissingMedia`、blob 不在 → `MissingMedia`、
  blob 実 hash ≠ digest → `HashMismatch`、demux 失敗 → `Corrupt`、
  codec ≠ Pcm → `UnsupportedCodec`。
- 成功: `PreparedClip { handle, producer, consumer, canonical: AudioFormat,
  metadata: SourceMetadata }`。

## Lowering

- 各非 muted Clip → `Processor` node、out port = `Audio(canonical)`。
- `master` = `Sink`、in port = `Audio(engine_fmt)`。
- 接続: layout 差 → `ConnDef.channel_map = Some` (mono→stereo は
  `[(0,0),(0,1)]`);rate 差 → `Skipped::RateMismatch` (暗黙 resample 禁止)。
- TimeAnchor::Media → `engine_start = floor(v*n*rate/d)` 厳密整数、
  丸めはこの最終境界のみ。Musical → TempoMap で秒→frames (同じく floor のみ)。
- `ClipBinding { clip_id, node_id, out_port, window {eng_start, eng_len,
  src_start, src_len, map: FrameMap::linear} }` — Stable Node/Port 写像。

## Playback

`ClipPlayer::process`: clock=engine 位置を読み、block を
[pre-window silence][in-window fill][post-window silence] に分割。
in-window は `consumer.fill(sub, expected=map(engine), ctx)` で
`FillReport{written,underrun,faulted,eof,mismatch}` を `PlaybackStats`
(atomics) へ集約。underrun/EOF は zero-fill 済み — 成功音声と混同しない。

`HeadlessDriver`: `GraphProject::prepare` (revision 記録・last-good) →
`Validator` → `Compiler::compile(factory)` → `GraphRuntime::publish`。
prepare/publish 失敗で以前の plan を保持、初回なら `Stopped`。
`run(blocks, PumpPolicy)`: 各 block で producer `pump()` (policy 制御→
underrun 再現) → `rt.process(frames, ctx)` → `output_reader(master_in)`
から収集 + crc32 fold。`seek(e)`: clock 更新 + 各 clip へ
`handle.seek(mapped)`。`stop()`: 明示 `Stopped`。

## Determinism / Evidence

- fixture WAV: `sample(ch,f) = (((f*31 + ch*7) % 4096) - 2048) * 16` (i16 厳密)、
  f32 期待値 = v/32768。mono/stereo、48kHz、小 frame 数。
- 全検証は `synvelis-player/tests/l1.rs` (fixture hash・report・exit code を
  impl record に記録)。
