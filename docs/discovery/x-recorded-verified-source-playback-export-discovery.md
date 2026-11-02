---
id: "DISC-PXOZ9PhzQFVHOLgu0pAGLo"
type: "discovery"
status: "resolved"
title: "X Discovery — Recorded Source→Generic Playback/Export integration with Frame/ID/Content integrity"
created: "2026-11-02"
updated: "2026-11-02"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/26"
    - "https://github.com/scarecrow173/synvelis/issues/22"
    - "https://github.com/scarecrow173/synvelis/issues/23"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  depends-on:
    - "docs/specs/vs-d-recording-commit-e2e-idempotency-undo-reload.md"
    - "docs/specs/l1-local-wav-verified-media-graph-playback.md"
    - "docs/specs/l2-master-sequential-offline-wav-export.md"
  related:
    - "docs/designs/media-import-export-offline-render-ui-diagnostics-architecture.md"
    - "docs/designs/playback-mixer-generic-execution-graph-architecture.md"
    - "docs/designs/time-transport-midi-automation-plugin-integration-architecture.md"
---

# X Discovery — Recorded Source→Generic Playback/Export (E2E-X)

## Goal

Issue #26: VS-D で Project へ commit された録音 Source が、L1 import と同じ
Verified Media Store (Asset Manifest + Content Digest + blob media)・
同一 Source Resolver・同一 Generic Graph・同一 Offline Render/Export 経路で
再生・出力できることを、VS-B deterministic Mock PCM 証拠で実証する。
録音固有の playback path・file-format 特例・第二 Project Store を作らない。

## Hard-start 状態 (origin/main 調査済み)

- VS-A〜D (#16): `synvelis-recording` — deterministic `MockPcm`/`MockInput`、
  `StreamWriter` (raw `f32le-interleaved` PCM media)・journal/checkpoint・
  `recovery` (Healthy/Partial/Corrupted/Unjournaled、durable frontier)・
  `commit_session` (manifest 予約 ID → `RecordingOp::CommitRecording` 単一
  mutation → Asset/Source/Take/Segment/Clip 登録)。
- L1 (#24): `synvelis-player` — `import_local` (digest-addressed blob +
  `CommitImport`)、`resolver::prepare` (`SourceState >= Verified` + asset
  digest + `blobs/<digest>` の symphonia-decodable PCM 検証)、generic
  lowering、`HeadlessDriver`。
- L2 (#25): `synvelis-render` — `RenderSnapshot::freeze`、
  `prepare::open_source` (同一 verified-source 境界)、offline render、
  `ruviform-io-wav` 実 PCM WAV export、独立 reopen verify。

## 発見した Gap (根本原因)

`apply_commit` が登録する `Source.content_digest` は **raw PCM の fold
digest** (`FinalizedSourceRef.digest`) であり、resolver が探す
`media_root/blobs/<content_digest>` の **symphonia-decodable blob は存在
しない**。つまり録音 Source は `Committed` state を満たしても media が
resolve できず、L1/L2 の verified-source 境界を通過できない。

録音 media は container-less な raw f32le interleaved であり、import が
要求する「decodable container + content_digest」という media contract を
満たさない。これは playback 側の問題ではなく、**finalized raw media を
canonical verified-media 形式へ publish する publication 境界が未実装**
であることが根本原因。

## 設計方針 (候補)

1. **Canonical publish 境界 (採用候補)**: finalize 後・commit 前の非 RT
   工程として、recovery で確定した durable frontier の raw PCM を
   `ruviform-io-wav` F32 WAV へ canonicalize し、`import` と同一
   `content_digest` scheme で `blobs/<digest>` へ原子公開。
   session dir に `canonical.json` receipt を残し、`build_payload` が
   整合した receipt を検出した場合に限り `Source.content_digest` に
   canonical digest を採用する。resolver/lowerer/render は一切変更しない。
2. Resolver に raw-PCM fallback を追加 — 却下: recording 固有の file-format
   特例を playback path に持ち込み、#26 の禁止事項に抵触。
3. Recording 側で独自に Source 登録 — 却下: 第二 Project Store / 別経路
   commit になり ID/provenance 一貫性を壊す。

## 検証の軸 (X-01..X-07)

- X-01: Normal gate 録音 → publish → commit → save/reload → L1 playback +
  L2 export + 独立 reopen、`MockPcm::sample` と frame/channel/sample 完全一致。
- X-02: Loop 3-pass + Punch + Pre-roll (負 `transport_start`) + clip
  source offset + project placement の複合で、pass/take/segment ごとの
  raw SourceFrame ↔ output frame 対応を証明。
- X-03: live/saved revision・content hash・commit identity・undo/redo・
  save/reload で Source/Asset/Take/Clip ID と digest が不変。
- X-04: crash before/after commit・journal truncation・partial recovery で
  Finalized 以外 (partial/orphan/WIP) が playback source にならない。
- X-05: 録音 RT fault (capture-fault marker/FaultLatch)・playback
  prefetch underrun・export 失敗の 3 fault 系を型上分離。
- X-06: 別 importer 経由の見かけ上の E2E をしない (import_local 不使用、
  commit 予約 ID のみ使用)。
- X-07: headless test 証拠 (fixture hash・ruviform revision・結果) を
  impl record へ記録。

## Open Questions → 解決済み

- `SourceState::Committed` は resolver の `>= Verified` ゲートを通過する
  (enum 順序 Imported < Verified < Committed < Saved を確認)。
- canonical WAV の wire format は **F32**: 録音 f32 を bit-exact に保ち
  X-01/X-02 の `MockPcm` 完全一致を成立させる (S16 は量子化で不可)。
- channels>2 の WAV speaker-mask mapping は semantic 決定を要するため
  E2E-X は 1–2ch に限定し、他は typed error + capability gap として記録。
- `build_payload` は canonical receipt 不整合を fail-closed で拒否し、
  receipt 不在時は従来どおり raw fold digest を登録 (VS-D 互換維持)。

## Sources and Relations

- Issue #26 本文・#22 Target・#23 Execution Plan
- `docs/specs/vs-d-recording-commit-e2e-idempotency-undo-reload.md`
- `docs/specs/l1-local-wav-verified-media-graph-playback.md`
- `docs/specs/l2-master-sequential-offline-wav-export.md`
