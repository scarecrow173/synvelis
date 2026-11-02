---
id: "IMPL-XRecordedVerifiedSourcePlaybackExport"
type: "impl"
status: "completed"
title: "E2E-X Recorded Source → Generic Playback/Export with Frame/ID/Content Integrity"
created: "2026-11-02"
updated: "2026-11-02"
owners: []
relations:
  source: []
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-player/src/publish.rs"
      - type: "file"
        path: "crates/synvelis-render/tests/x_e2e.rs"
      - type: "file"
        path: "docs/discovery/x-recorded-verified-source-playback-export-discovery.md"
      - type: "file"
        path: "docs/specs/x-recorded-verified-source-playback-export.md"
      - type: "file"
        path: "docs/adr/x-recorded-media-canonicalization-boundary.md"
      - type: "file"
        path: "docs/designs/x-recorded-verified-source-playback-export-design.md"
      - type: "file"
        path: "docs/plans/x-recorded-verified-source-playback-export.md"
      - type: "file"
        path: "docs/test-specs/x-e2e-recorded-source-playback-export.md"
      - type: "file"
        path: "docs/tasks/x-task1-canonical-ref-manifest.md"
      - type: "file"
        path: "docs/tasks/x-task2-recorded-media-publish.md"
      - type: "file"
        path: "docs/tasks/x-task3-acceptance-tests.md"
      - type: "file"
        path: "docs/tasks/x-task4-verification-impl-record.md"
      - type: "file"
        path: "docs/impl/ir/x-recorded-verified-source-playback-export.md"
    modified:
      - type: "file"
        path: "crates/synvelis-player/Cargo.toml"
      - type: "file"
        path: "crates/synvelis-player/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-recording/src/commit.rs"
      - type: "file"
        path: "crates/synvelis-recording/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-recording/src/staging.rs"
      - type: "file"
        path: "crates/synvelis-render/Cargo.toml"
      - type: "file"
        path: "Cargo.lock"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/x-recorded-verified-source-playback-export.md"
    - "docs/designs/x-recorded-verified-source-playback-export-design.md"
    - "docs/specs/x-recorded-verified-source-playback-export.md"
    - "docs/adr/x-recorded-media-canonicalization-boundary.md"
    - "docs/tasks/x-task1-canonical-ref-manifest.md"
    - "docs/tasks/x-task2-recorded-media-publish.md"
    - "docs/tasks/x-task3-acceptance-tests.md"
    - "docs/tasks/x-task4-verification-impl-record.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# E2E-X Recorded Source → Generic Playback/Export

## Summary

録音済み media（`f32le-interleaved` raw PCM + fold digest）を
canonical verified media（PCM f32 WAV blob・import と同一
content-digest scheme）へ publish する橋を
`synvelis-player::publish` に追加し、commit manifest が
`Source.content_digest` に canonical digest を採用するよう配線した。
これで録音 source は external WAV import（L1）と完全に同一の
`resolver::prepare → lowering::lower → HeadlessDriver` および
`RenderSnapshot → export`（L2）経路を通る — 録音固有の
playback path・format 特例・第二の Project Store は存在しない。

## Context

Issue #26。前提: VS-A..D（#16, recording foundation・冪等
commit）と L2 gate（#25）が origin/main マージ済み。根本原因:
`resolver::prepare` は `media_root/blobs/<content_digest>` に
symphonia-decodable な container を要求するが、recording commit は
raw `f32le-interleaved` bytes + fold digest を `content_digest` に
載せていた — state gate（`Committed ≥ Verified`）は通るが blob が
解決不能だった。ADR X-1: verified-media 境界で canonical 化する
方針を選択（resolver/graph/render に format 特例を入れない）。

## Implementation

### Changes

- `synvelis-recording/src/staging.rs`: `CanonicalSourceRef`
  （`streams/stream-NNNN.canonical.json` receipt: session/stream/
  lane identity + `raw_digest` provenance + canonical `digest` +
  `media_format`/`byte_len`/`source_frames`/`sample_rate`/`channels`）、
  `CANONICAL_SCHEMA_VERSION`/`CANONICAL_MEDIA_FORMAT`（"wav-f32"）、
  `canonical_rel`/`StagingArea::canonical_path`。
- `synvelis-recording/src/commit.rs`: `load_canonical`（stream ごと
  に receipt を読み `CanonicalReceipt{canonical,error}` で分類）、
  `adopt_canonical_digest`（receipt が ref と schema/format/
  identity/raw_digest/frames/rate/channels で整合すれば canonical
  digest を採用し `raw_digest=Some(raw)` を manifest へ記録、
  無ければ raw digest に fallback（VS-D の直接 commit 挙動を保持）、
  不整合 receipt は `CommitError::Manifest` で fail-closed）、
  `StreamEntry.raw_digest`（`Option<String>`、serde default で
  後方互換）、payload 構築の中間表現 `StreamSpec` へ構造化。
- `synvelis-player/src/publish.rs`（新規）: `publish_recorded(area,
  media_root)` — `load_finalized` で `Complete` ref のみ対象
  （`partial-finalize`/`missing-finalized`/`finalized-unreadable`/
  `capture-fault` は `skipped` 分類）、staging manifest と ref の
  identity 照合（build_payload と同一 rule・不一致は
  `PublishError::Conflict`）、整合 receipt があれば `reused`、
  別 media を指す receipt は `Conflict`、なければ raw media を
  frontier（`media_byte_len`）まで読み `WavMuxer`/`WavEncoder`
  （ruviform-io-wav・L2 と同一）で PCM f32 WAV encode → import と
  同一 `content_digest` で `blobs/<digest>` へ `publish_atomic` →
  receipt を atomic 記録 + dir sync。`ensure_blob` は既存 blob を
  `content_digest_stream` で再検算し、digest 名と異なる内容は
  上書きせず fail-closed。`PublishError` は `Io`/`CorruptMedia`/
  `Conflict`/`Encode`/`UnsupportedChannels` の typed 分類。
- `synvelis-player/Cargo.toml`: `ruviform-io-wav` dep 追加。
- `synvelis-player/src/lib.rs`: `pub mod publish` + re-export。
- `synvelis-render/Cargo.toml`: dev-dep `synvelis-recording` 追加。
- `synvelis-render/tests/x_e2e.rs`（新規・約 900 行）: X-01..X-05
  acceptance suite（X-06 は経路構造で立証 — 後述）。

### Decisions During Implementation

- canonical 化の境界は player 側（`synvelis-player::publish`）に
  置いた。recording 層は verified raw media の durable 責務を持ち、
  player 層は「resolver が読める canonical blob」の責務を持つ —
  media 形式変換は後者の関心事。recording → player の依存方向
  （render→player→project、recording→project）を崩さない。
- commit は publish 済み receipt を **採用** するだけで publish
  自体は行わない（command 内で media I/O しない VS-D 契約を保持）。
  publish を `commit_session` 前に呼ぶのは呼び出し側の契約だが、
  manifest を先に書いた後に publish しても `prepare_manifest` の
  fingerprint 再検算が stale manifest を破棄・再構築するため
  canonical digest に自己修復する（fail-open ではなく型付き回復）。
- receipt 不整合は `Conflict`/`Manifest` で全体失敗 — 部分 publish
  は atomic 単位でのみ発生し、中途半端な receipt を残さない。
- X-06（no recording-specific playback path）は静的立証:
  `publish_recorded` 以降の経路は `prepare`/`lower`/`HeadlessDriver`/
  `export` の generic API のみを呼び、resolver/graph/render には
  1 行も録音分岐を追加していない（diff で確認可能）。

## Acceptance Matrix Traceability

| AC | evidence | test |
| --- | --- | --- |
| X-01 | 48k stereo 録音 → publish → commit → save/reload → headless playback が 48,000 frame 全 sample bit-exact（MockPcm 再導出）、export reopened planes が playback planes と完全同一、underrun は zero-fill+count で content 化しない | `x_01_recorded_playback_export_bit_exact` |
| X-02 | `PunchInLoop` 3-pass + punch window + `transport_start=-384` pre-roll: 3 segments（session 384,000 frame 間隔・source 191,950 frame 連続タイル）+ 3 takes + auto-clip；pass-1 を [20000,27678)、pass-2 を負 tick [-1920,5758) に配置（clamp+source offset）し、691,950 frame 全域で `clip → segment.source_range → session_range → MockPcm input_frame` の bit-exact 対応 | `x_02_punch_in_loop_per_segment_correspondence` |
| X-03 | `Source.content_digest == Asset.digest == manifest.digest == canonical receipt digest`・`manifest.raw_digest == FinalizedSourceRef.digest`・undo→entity 消失→redo→同一 stable IDs・save/reload 後 `applied_commits.content_hash` 不変・export/playback が revision を変更しない | `x_01` + `x_03_identity_revision_undo_save_reload` |
| X-04 | crash-before-commit（publish 済みでも未 commit なら source 未登録・resolve 不可）、WIP（`missing-finalized` skip・recoverable・未 commit）、journal 末尾 truncation → `Partial` → publish skip・commit recoverable・未 commit、crash-after-commit+reload → `AlreadyApplied` + playback 継続、`blobs/` は canonical digest のみ | `x_04_finalized_only_visibility` |
| X-05 | capture-fault marker → publish `capture-fault:` skip + commit recoverable（typed 分離）、engine underrun → zero-fill + `stats.underrun`、export 既存 destination → `ExportOutcome::Failed` + file 無改変 | `x_01` underrun 部 + `x_05_fault_classes_separate` |
| X-06 | resolver/lowering/driver/export に録音分岐なし（上記 static 立証 + publish idempotent reuse/re-materialization を `x_01` で実測） | `x_01` publish 冪等部 |

## Validation

全て exit code 0 で実測:

- `mise run fmt-check`: clean。
- `mise run check`（`cargo check --workspace --all-targets`）: clean。
- `mise run lint`（`cargo clippy --workspace --all-targets -- -D warnings`）: clean。
- `mise run test`（`cargo test --workspace --locked`）: **全 test binary green / 0 fail**
  — `x_e2e` **5/5**、`vs_d` 22/22（regression なし）、`l1_e2e` 10/10、
  `l2_e2e` 15/15、unit/doc tests 含む全 23 suite `ok`。
- `cargo test -p synvelis-render --test x_e2e`: 5/5 pass（約 1.8s）。
- Toolchain: `rustc 1.99.0`（workspace `rust-version = "1.89"` 準拠）。
- Ruviform lock: `git+…ruviform.git#322a661a`（変更なし）。
- Headless E2E: mock input → writer/journal → finalize → canonical
  publish → commit → generic graph → export まで実 symphonia/io-wav
  経路・GUI/device 不要。

Deterministic evidence（同一入力で再現 — MockPcm は
`(input_frame, channel, seed)` の純関数）:

| scenario | frames×ch | 証明 |
| --- | --- | --- |
| x_01 normal | 48,000×2 | playback/export 全 sample == `MockPcm{seed:0}`、blob digest == source.content_digest |
| x_02 punch-in-loop + pre-roll | 691,950×2 | 3 segment の source↔session↔input 対応が clip placement（正・負 tick・offset）全域で bit-exact |
| x_04 crash/retry | 24,000×2 | `AlreadyApplied` + reload 後の playback bit-exact |

未計測・未実施: mono/multi-channel(>2) 録音の publish（
`UnsupportedChannels` は型付きで refuse 済み）、media anchor clip
（録音 commit は Musical anchor のみ生成 — L1 側で網羅済み）、
export `Estimated` tail・inserts 付き構成（L2 側で網羅済み・
本件スコープ外）。

## Related Experiments

なし（experiment log 対象の試行は発生しなかった）。

## Risks

- publish の呼び出し順は「`commit_session`（実質
  `prepare_manifest` の最終評価）より前」が契約。順序違反は
  fingerprint 再構築 or `IntegrityConflict` の型付き結果になり
  silent な raw-digest commit にはならないが、呼び出し側が receipt
  採用を期待する場合は publish → commit の順で呼ぶこと。
- raw staging media（staging area 配下）と canonical blob（media
  root 配下）は別々の durability 境界 — canonical blob は raw
  media からいつでも再 materialize 可能（digest で一致保証）。

## Known Limitations / Report

- `canonical.json` receipt は session-dir に persist されるため
  staging area を消すと publish は再 encode が必要（blob が残れば
  digest 一致で reused 相当、receipt は再生成）。
- Ruviform 側の新規 issue は不要 — 既存 `#191`（WAV encoder/muxer）
  と `#198`（prepared offline render）をそのまま使用。

## Follow-ups

- `publish_recorded` を commit 前処理として呼ぶ薄い orchestration
  （CLI/PoC 入口）は本件スコープ外 — 現状は test が直接合成する。

## Rollback

- `StreamEntry.raw_digest` は `Option`+serde default — receipt 無しの
  旧挙動 manifest と両立。`publish.rs`/`x_e2e.rs`/docs は追加物の
  ため revert で完全除去可能。`commit.rs` の receipt 採用分岐は
  receipt 不在時に旧挙動へ自然 fallback。
