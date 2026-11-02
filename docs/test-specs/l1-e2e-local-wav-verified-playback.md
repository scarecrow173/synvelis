---
id: "TSPEC-3kHdKFWk6cHMT13mFySlYa"
type: "test-spec"
status: "approved"
title: "L1 検証仕様 — Local WAV Safe Import・Verified Media・Headless Playback"
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
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/l1-local-wav-verified-playback-design.md"
  derived-by: []
  verifies:
    - "docs/specs/l1-local-wav-verified-media-graph-playback.md"
---

# L1 Test Spec

検証本体: `crates/synvelis-player/tests/l1.rs` + `synvelis-project` unit tests。

## Feature: Safe Local Import (L1-01, L1-04)

- 小 mono/stereo WAV → import: size/digest/frame/channel/rate 実測一致、
  `BlobReceipt`/`ProjectReceipt`/`SavedState` 分離、blob が `blobs/<digest>`。
- `RegisterSource` (state=Imported) のみの Source は `UnverifiedSource`。
- retry same import_id → 同一 source_id、重複登録なし、blob reuse。
- 破損入力 → `Corrupt`、非 WAV/非 PCM → `UnsupportedCodec`、blob 喪失 →
  `MissingMedia`、blob 改竄 → `HashMismatch`、revision conflict →
  `StaleRevision`、全ケースで `live()` 不変 (部分 mutation なし)。
  commit 失敗後の blob は Orphaned として明示。

## Feature: Stable ID / references (L1-02)

- 1 source、2 clips (別 lane・別 placement・別 source_offset)。
  `AddClip`/`SetClipMuted` は controller 経由のみ。Undo/Redo で clip 集合
  一致、Save/Reload で Source/Asset/Clip 参照・muted・applied_imports 一致。

## Feature: Deterministic playback (L1-03, L1-05)

- fixture: `sample(ch,f)=(((f*31+ch*7)%4096)-2048)*16` i16 → f32=v/32768。
- 単一 clip [s,e): 区間内は golden 一致・区間外は 0。Clip 開始/末尾、
  source_offset 中間参照、block 境界跨ぎ (block=64・clip 長非倍数)。
- seek(途中) → 次 block から mapped source が一致。mismatch=0。
- stereo master + mono clip は ChannelMap 両 ch 同値、stereo clip 一致。
- rate 差 source は `Skipped::RateMismatch`、implicit resample なし。

## Feature: Failure / observability (L1-04, L1-06)

- pump 0 → underrun>0・written=0・出力 0。EOF → report.eof・以後 0。
- invalid spec → prepare 失敗で last-good 維持 or Stopped。
- capability 記録: reader/graph/audio version・canonical fmt・declared vs
  実測 frames・total_latency・不足 (video/mux/encode/resample) を report。

## Cross-reference (L1-08)

EX-003(半開区間), EX-005(stable handle), EX-008(DAG), EX-009(last-good→L1
は構造変更のみ・即時 swap 非対象と明示), EX-010(有界 buffer), EX-014
(underrun 観測), EX-016(非RT prepare/RT 分離), EX-019(RT 非所有),
EX-020(明示 Stopped);PAP-002(identity 分離), PAP-005(verify gate),
PAP-008(content-addressed blob), PAP-010(receipt 分離), PAP-013(orphan
明示), PAP-017(snapshot 保持);MR-002(typed fault), MR-005(EOF≠成功),
MR-007(prefetch generation), MR-011(source frame 写像), MR-014(実測 frame)。
未対応: resample/quantize (rate 差は明示 skip)、EX-009 即時 edit swap、
video/mux/encode — 合格表記しない。

## Evidence (L1-07)

`cargo test --workspace --locked` の出力・exit code、fixture byte hash
(digest_fold hex16) を impl record に記録。
