---
id: "SPEC-wVS4pCX5veOtjhEh013pJ0"
type: "spec"
status: "approved"
title: "X — Recorded Source→Generic Playback/Export integration with Frame/ID/Content integrity"
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
    - "docs/discovery/x-recorded-verified-source-playback-export-discovery.md"
    - "docs/specs/vs-d-recording-commit-e2e-idempotency-undo-reload.md"
    - "docs/specs/l1-local-wav-verified-media-graph-playback.md"
    - "docs/specs/l2-master-sequential-offline-wav-export.md"
  derives-from:
    - "docs/discovery/x-recorded-verified-source-playback-export-discovery.md"
---

# X — Recorded Source→Generic Playback/Export (E2E-X)

## What Should Be Built

Issue #26: VS-B deterministic Mock PCM を non-RT Writer/Journal/Recovery
(VS-C) → Finalized Verified Source → Project Commit/Save/Reload (VS-D) へ通し、
その Recorded Source を **L1 (#24) と同一の Source Resolver / Generic Graph /
Offline pipeline** で再生し、**L2 (#25) と同一の Master Export** で出力する
E2E 統合スライス。

欠落していた境界は 1 つ: finalized raw `f32le-interleaved` media を canonical
verified-media 形式 (decodable PCM container + `content_digest` blob) へ
publish する **canonical publication 工程**。これを finalize 後・commit 前の
非 RT 工程として実装し、`commit_session` が登録する `Source.content_digest`
を canonical blob digest とする。Resolver/Lowerer/Render/Export には一切
録音固有分岐を入れない。

## Why Now

E2E-L1 (#24)・E2E-L2 (#25)・VS-D (#16) が全て main へマージされ、録音側と
再生/出力側の両契約が固定された。残るは両者を同一 media contract で接続
する工程のみであり、これが録音→再生/Export の最初の実装単位。

## Users and Value

- DAW ユーザー: 録った素材が import 済み外部ファイルと同じ信頼境界で
  そのまま再生・書き出しできる。
- 開発者: 「Finalized 以外は絶対に再生 source にならない」境界と
  per-pass/take/segment の frame 対応が E2E で固定される。

## Scope

### In Scope

1. **Canonical publication (Synvelis・非 RT)**: recovery 確定 durable
   frontier の raw PCM を `ruviform-io-wav` F32 WAV へ encode、
   `import` と同一 `content_digest` (fwd+bwd crc32 hex16) で
   `media_root/blobs/<digest>` へ原子公開、session dir に
   `canonical.json` receipt を原子公開。Complete ref のみ対象。
   Partial/orphan/WIP/missing は publish しない (report へ分離)。
2. **Commit digest 接続 (synvelis-recording)**: `build_payload` が
   canonical receipt を参照し、整合する場合に限り
   `StreamEntry.digest` = canonical digest + `raw_digest` provenance を
   登録。receipt 不在時は従来の raw fold digest (VS-D 互換)。
   receipt 不整合は fail-closed で commit 拒否。
3. **同一経路再生/出力**: `resolver::prepare` → `lower` → `HeadlessDriver`
   (L1 経路) と `RenderSnapshot::freeze` → `export` → 独立 reopen
   verify (L2 経路) を録音 Source へそのまま適用。コード変更なしで
   通ることを X-01/02 で実証。
4. **Frame/ID/Content 証拠**: `MockPcm::sample` deterministic 生成器と
   segment の `source_range`/`session_range`/`project_in/out` から
   pass/take/segment 単位の raw SourceFrame ↔ output frame 対応を検証。
   ID (SourceId/AssetId/TakeId/ClipId)・digest・revision の
   save/reload/undo/redo 不変性を検証。
5. **Finalized-only 境界**: crash before/after commit・journal
   truncation・partial recovery で Finalized 以外が publish/commit/
   resolve されないことを検証。3 fault 系 (RT capture fault・playback
   underrun・export failure) の型分離を確認。

### Out of Scope

実 device capture、RT 再生 GUI、Stem/Bounce、resample、WAV 以外の
canonical container、channels>2 の speaker-mask mapping (typed error +
capability gap 記録)、Ruviform 側の変更 (不足があれば upstream issue 化)。

## Requirements

- Canonicalize は **非 RT**・finalize 後・commit 前。入力は durable
  frontier (`media_byte_len`) のみ — 未検証 tail を publish しない。
- Canonical receipt は blob 公開後に原子公開 (receipt→missing-blob の
  window を作らない)。再実行は冪等 (同一 finalized ref → 同一 digest)。
- `Source.content_digest`/`Asset.digest` = canonical blob digest;
  raw fold digest は `StreamEntry.raw_digest` + finalized.json/journal で
  provenance を保持。
- Recorded SourceId/AssetId/TakeId/ClipId は manifest 予約 ID のみ。
  `import_local`・`applied_imports`・`ImportId` を使わない。
- Export/Playback は project を mutation しない。失敗は typed で
  partial を成功に見せない。
- `Tick`/`TickDuration` の unchecked 演算子は Synvelis 側で使わない。

## Acceptance Criteria

- [ ] **X-01**: Normal gate Mock 録音 (48kHz・1–2ch・deterministic
  `MockPcm`) → finalize → canonical publish → commit → save/reload →
  L1 headless playback + L2 master export → 独立 reopen で、出力の
  frame/channel/rate/sample が `MockPcm::sample` と bit-exact 一致
  (損失・重複・誤充填 silence なし)。
- [ ] **X-02**: Loop 3-pass + Punch + Pre-roll (負 `transport_start`) +
  clip source offset + project placement の複合シナリオで、
  pass/take/segment の `source_range` ↔ engine/output frame が
  mock 生成器との対応どおり (per-segment で確認)。
- [ ] **X-03**: commit 前後・save/reload・undo/redo で
  Source/Asset/Take/Clip ID・`content_digest` (= blob digest)・
  live/saved revision・`applied_commits` identity が一致を保ち、
  export/playback 前後でも不変。
- [ ] **X-04**: (a) crash before commit → source 未登録・resolve
  MissingSource。(b) commit+save 後の reload → source resolve 可。
  (c) journal truncation → `recover_finalize` が Partial を返し commit
  が recoverable 分離 → resolve 不可。(d) WIP/orphan media は publish
  対象にならない。
- [ ] **X-05**: 録音 capture fault・playback underrun・export failure が
  それぞれ別型で報告され相互に偽装されない。
- [ ] **X-06**: テストは `import_local`/`ImportRequest` を一切使わず、
  commit が発行した ID で playback/export が通る (importer-copy
  shortcut ではないこと)。
- [ ] **X-07**: `mise run test` headless で全件 PASS; fixture/recorded
  media hash・ruviform revision・toolchain・exit code・未検証項目を
  impl record に記録。

## Deferred Design Concerns

- channels>2 canonical WAV の speaker-mask 決定 | E2E-X は 1–2ch |
  多ch 録音対応 Issue で speaker mapping 方針を決定 | 現状 typed error。
- 長尺 media の decode-to-memory 上限 | L2 同様 fixture は小規模 |
  実尺需要時に windowed decode | L2 deferred と同一理由。
- canonical receipt と blob の orphan GC | staging 消去方針と一体 |
  staging lifecycle Issue | 現状 blob は content-addressed で冪等。

## Sources and Relations

- Issue #26 本文 (X-01..X-07 受入・禁止事項)
- `docs/discovery/x-recorded-verified-source-playback-export-discovery.md`
- `docs/specs/vs-d-recording-commit-e2e-idempotency-undo-reload.md`
- `docs/specs/l1-local-wav-verified-media-graph-playback.md`
- `docs/specs/l2-master-sequential-offline-wav-export.md`
