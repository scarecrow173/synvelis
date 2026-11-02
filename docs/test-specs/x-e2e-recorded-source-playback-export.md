---
id: "TSPEC-zzdcoJZ1siQdbof0DBfv5l"
type: "test-spec"
status: "approved"
title: "X 検証仕様 — Recorded Source→Generic Playback/Export・Frame/ID/Content integrity"
created: "2026-11-02"
updated: "2026-11-02"
owners: []
relations:
  source: []
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
    - "docs/designs/x-recorded-verified-source-playback-export-design.md"
  derived-by: []
  verifies:
    - "docs/specs/x-recorded-verified-source-playback-export.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# X Test Spec — Recorded Source→Generic Playback/Export

検証本体: `crates/synvelis-render/tests/x_e2e.rs` (+ recording/player unit
tests)。Fixture: VS-B deterministic `MockPcm` (seed 固定・`sample(input_frame,
ch)` が正本)。録音手順は `vs_d.rs` パターン (CreateSession→OpenSession→
RegisterStream→StagingArea→preflight→Scenario<StreamWriter>→run_to_end→
checkpoint→finish_all→CloseSession) を再利用。

## Feature: Canonical Publication (unit + X-04 連携)

- Complete ref → `canonical.json` 生成・`media_root/blobs/<digest>` 公開・
  `digest == content_digest(wav bytes)`。
- publish 冪等: 2 回目は reused、blob 欠損時は blob のみ再公開。
- Partial/missing/identity-mismatch ref → `skipped`・blob/receipt なし。
- 不整合 receipt (raw_digest 差異) → `PublishError::Conflict`。
- `media_byte_len` 超の tail bytes は publish されない (frontier のみ)。
- channels>2 → `PublishError::UnsupportedChannels`。
- canonical.json 存在+整合 → commit 後 `Source.content_digest` =
  canonical digest、`StreamEntry.raw_digest` = fold digest。
- canonical.json 不在 → digest = fold digest (VS-D 互換)。
- canonical.json 不整合 → `CommitError::Manifest` fail-closed。

## Feature: X-01 End-to-end integrity

- Normal gate `[0,1920)` (48000 frames @48k・stereo) 録音 → publish →
  commit → `DirectoryBundleStore` save → 新 controller reload。
- reload 後の `resolver::prepare` + `lower` + `HeadlessDriver::run` で
  出力 PCM が `MockPcm::sample` と bit-exact (全 frame・全 channel)。
- `RenderSnapshot::freeze` + `export` → staged WAV publish → 独立
  symphonia reopen で frames/channels/rate/PCM が同一 evidence と一致。
- `PlaybackStats.underrun == 0`・`ExportOutcome::Completed`。

## Feature: X-02 Composed gate + placement

- `PunchInLoop` (loop 3 passes・punch window 内のみ保存) +
  `transport_start < 0` (pre-roll) で録音 → publish → commit。
- manifest の全 segment について `source_range` ↔ `project_in/out` が
  gate scheduler の規則と一致。
- auto-clip + `AddClip` (他 pass の take/segment、非 0 `source_offset`、
  負/正 project placement) で playback し、engine frame ごとに
  対応する raw SourceFrame の `MockPcm::sample` と一致
  (clip 不在区間は 0.0、loss/dup/mis-filled silence なし)。

## Feature: X-03 Identity / revision / hash consistency

- commit manifest `identity.content_hash` が `applied_commits` registry と
  一致、Source/Asset digest == blob `content_digest`。
- commit が 1 mutation (revision +1)、undo → sources/takes/clips 消失、
  redo → 同一 ID で復元 (silent re-assign なし)。
- save → reload で SourceId/AssetId/TakeId/ClipId・digest・live/saved
  revision・`applied_commits` が一致。
- playback/export 実行前後で live revision・saved snapshot・source
  digest が不変 (export が project を汚染しない)。

## Feature: X-04 Finalized-only boundary

- crash before commit (session finalized・未 commit): source 未登録 →
  `resolver` が MissingSource/NotFound で拒否。
- crash after commit (commit+save → reload): source resolve 可・
  playback 可。
- journal truncation: `recover_finalize` → Partial/短い frontier →
  `commit_session` が recoverable 分離 → source 非登録。
- WIP/orphan media (finalized.json なし・queue overflow fault):
  publish `skipped`・commit recoverable・resolve 不可。
- どの分岐でも partial/orphan/WIP bytes が blob として出ない
  (blobs/ dir 列挙で検査)。

## Feature: X-05 Fault separation

- capture-fault marker → `RecoverableStream.reason` が `capture-fault:*`
  (録音 fault 系)。
- pump 不足 → `PlaybackStats.underrun > 0` (playback 系・PCM 偽装なし)。
- destination exists / verify 失敗 → `ExportOutcome::Failed{stage}` または
  `Cancelled` (export 系)。三者が別型で混じらない。

## Feature: X-06 No importer shortcut

- 全 X test が `import_local`/`ImportRequest`/`applied_imports` を
  使わない。registered source は commit manifest の予約 ID のみ。

## Evidence (X-07)

- recorded raw PCM fold digest・canonical WAV `content_digest`・
  exported WAV hash・frames/channels/rate を impl record へ記録。
- ruviform revision (`Cargo.lock`)、toolchain、`mise run test` exit code、
  未検証項目 (channels>2 等) を記録。
