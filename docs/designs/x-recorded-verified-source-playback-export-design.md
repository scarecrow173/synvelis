---
id: "DESIGN-HEDK2Gt2m6FAc3GREIhJDl"
type: "design"
status: "approved"
title: "X Design — Canonical publication receipt・commit digest 接続・同一経路 playback/export"
created: "2026-11-02"
updated: "2026-11-02"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/26"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/x-recorded-verified-source-playback-export.md"
  depends-on:
    - "docs/adr/x-recorded-media-canonicalization-boundary.md"
    - "docs/designs/l1-local-wav-verified-playback-design.md"
    - "docs/designs/l2-master-sequential-offline-wav-export-design.md"
    - "docs/designs/vs-d-recording-commit-e2e-idempotency-undo-reload.md"
  derives-from:
    - "docs/specs/x-recorded-verified-source-playback-export.md"
    - "docs/adr/x-recorded-media-canonicalization-boundary.md"
---

# X Design — Recorded Source→Generic Playback/Export

## Overview

```
MockPcm/MockInput → StreamWriter (raw f32le PCM + journal)
  → finish_all (FinalizedSourceRef: durable frontier)
  → [NEW] publish_recorded: frontier bytes → F32 WAV → content_digest
       → blobs/<digest> 原子公開 → canonical.json 原子公開
  → commit_session: build_payload が canonical.json を検出・整合確認
       → StreamEntry.digest = canonical digest (+raw_digest provenance)
       → RecordingOp::CommitRecording 単一 mutation
       → Asset/Source(digest=canonical, state=Committed)/Take/Segment/Clip
  → save → reload
  → resolver::prepare → lower → HeadlessDriver      (L1 経路・無変更)
  → RenderSnapshot::freeze → export → reopen verify (L2 経路・無変更)
```

## CanonicalSourceRef (canonical.json)

session dir の `streams/stream-NNNN.canonical.json` (staging schema,
`deny_unknown_fields`, 原子公開)。`synvelis-recording::staging` が
path helper (`canonical_rel`, `canonical_path`) と loader
(`load_canonical`) を所有する — session-dir layout の責務。

```json
{
  "schema_version": 1,
  "session_id": "rs_...", "stream_index": 0,
  "capture_stream_id": "cs_...", "lane_id": "ln_...",
  "raw_digest": "<FinalizedSourceRef fold digest>",
  "digest": "<canonical blob content_digest>",
  "media_format": "wav-f32",
  "byte_len": <canonical wav bytes>,
  "source_frames": 48000, "sample_rate": 48000, "channels": 2
}
```

整合条件 (build_payload / publish 双方で同一 rule を検査):
session_id・stream_index・capture_stream_id・lane_id・sample_rate・
channels が FinalizedSourceRef / staging manifest と一致し、
`raw_digest == fin.digest`・`source_frames == fin.source_frames`。
不整合は fail-closed (`CommitError::Manifest` / `PublishError::Conflict`)。

## StreamEntry 変更

- `digest`: 「登録される `content_digest`」の意味に統一。canonical
  存在時 = canonical digest、不在時 = raw fold digest (従来)。
- `raw_digest: Option<String>` (`#[serde(default)]`): canonical 時のみ
  `Some(fold digest)` — manifest 内で raw media への provenance を保持。
- `media`・`media_byte_len`・segments の `media_offset/len`: **raw
  staging media のまま** (segment の byte range は raw PCM を指す。
  resolver は `digest` で blob を引くため media path を使わない)。
- `COMMIT_SCHEMA_VERSION` を bump: `digest` 意味の拡張を fail-fast で
  区別する。未適用 stale manifest は `prepare_manifest` が再構築する。

## publish_recorded (synvelis-player::publish)

```rust
pub fn publish_recorded(
    area: &StagingArea, media_root: &Path,
) -> Result<PublishReport, PublishError>
```

1. `load_finalized(area)` — `FinalizedOutcome::Complete` のみ対象。
   Partial/missing/unreadable/identity-mismatch は `skipped[]` に
   理由付きで分離 (publish しない = Finalized-only)。
2. identity 整合: staging manifest stream と ref の
   capture_stream_id/lane_id/session_id/stream_index/channels/
   sample_rate/media を照合 (build_payload と同一 rule、fail-closed)。
3. canonical.json 存在時: 整合 receipt + blob 存在 → `reused`。
   整合 receipt + blob 欠損 → blob のみ再公開。不整合 receipt →
   `PublishError::Conflict` (silent re-publish しない)。
4. raw media `media_path` から **durable frontier `media_byte_len` のみ**
   読み、`source_frames*channels*4` と一致を検証 (不一致 = CorruptMedia)。
5. deinterleave → `AudioBuffer` planar → `WavEncoder(F32)` → `WavMuxer`
   → WAV bytes → `content_digest` → `blob_path` → `publish_atomic` →
   canonical.json `publish_atomic` (+dir sync)。layout:
   1ch→Mono, 2ch→Stereo, 他→`PublishError::UnsupportedChannels`。
6. `PublishReport{published, reused, skipped}` を返す。

## build_payload digest 接続

`build_payload` 冒頭で `load_canonical(area)` (load_finalized と同型の
best-effort loader、read/parse 失敗は `None`+error)。Complete ref ごとに:

- canonical 存在 + 整合 → `digest = canonical.digest`,
  `raw_digest = Some(r.digest)`。
- canonical 存在 + 不整合 → `CommitError::Manifest` (fail-closed)。
- canonical 不在 → `digest = r.digest`, `raw_digest = None` (従来)。

canonical 有無は `fingerprint` の `s.digest` 入力を変えるため、
未適用 manifest は自動で再構築され、適用済みは registry で不変。

## Playback / Export (変更なし経路)

- `resolver::prepare`: `Source.state >= Verified` (Committed は通過) +
  `Asset.digest == Source.content_digest` + `blobs/<digest>` 存在 +
  `content_digest_stream` で bytes 検証 + symphonia demux `CodecId::Pcm`
  (F32LE WAV は PCM family) + canonical f32 decode。
- `lower`: clip Musical anchor → tempo map (120bpm PPQN960 →
  25 frames/tick @48k) → engine window; `source_offset` → `source_start`;
  negative project placement → lead clamp + source offset 前進。
- `export`: `RenderSnapshot::freeze` → `open_source` → `decode_window`
  → sequential render → io-wav F32/S16 → reopen verify → staged publish。

## Frame correspondence (X-01/X-02 の検証式)

- `MockPcm::sample(input_frame, ch)` が raw SourceFrame の正本
  (`MockInput::new(epoch, 0)` なら source frame = input frame)。
- segment ごとに `source_range` ↔ `project_in/out` (ticks) ↔
  engine frame range `tick * frames_per_tick`。
- 出力 expected[i][ch] = window に含まれる clip の
  `MockPcm::sample(source_start + (i - engine_start), ch)`、
  含まれなければ 0.0。Loop pass の各 take は `source_range` が
  pass ごとの source 区間を指す (auto-clip は最低 pass のみ;
  他 pass は `AddClip` で同一 SourceId・segment `source_range.start()`
  offset・対応 project tick へ配置して generic clip path で再生)。

## Fault separation (X-05)

- 録音側: capture-fault marker (session 単位 durable) → build_payload が
  全 stream を `capture-fault` recoverable 分離。queue overflow/
  discontinuity も recoverable。いずれも `CommitOutcome`/`RecoverableStream`
  の型で報告。
- Playback: `PlaybackStats.underrun` (>0 で内容供給欠損を報告 — silence
  を成功に見せない)。
- Export: `ExportOutcome::Failed{stage,error}` / `Cancelled{phase}`。
- 三者は全て別型・別観測点で、相互に変換されない。

## Deferred / capability gaps

- channels>2: `PublishError::UnsupportedChannels` — WAV speaker-mask
  mapping の semantic 決定が必要 (upstream/design followup)。
- raw media→canonical の二重保持容量: staging lifecycle 管理に委譲。
