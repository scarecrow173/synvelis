---
id: "PLAN-KF3NnioeKW8ERFMd7dyN15"
type: "plan"
status: "approved"
title: "X 実装計画 — Recorded Source→Generic Playback/Export integration"
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
  implements:
    - "docs/specs/x-recorded-verified-source-playback-export.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/x-recorded-verified-source-playback-export.md"
    - "docs/designs/x-recorded-verified-source-playback-export-design.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/x-e2e-recorded-source-playback-export.md"
  references: []
  defers: []
  deferred-by: []
---

# X 実装計画 — Recorded Source→Generic Playback/Export

## Goal

ADR Option 1 (canonical publication 境界) を実装し、X-01..X-07 acceptance
を実測 PASS させる。playback/render 側には録音固有コードを入れない。

## Scope

単一実装ストリーム。境界: canonical encode/publish は `synvelis-player`、
receipt schema + commit digest 接続は `synvelis-recording`、
受入 test は `synvelis-render` (player/render 両方に依存できる唯一の crate)。
Ruviform は変更しない。

## Upstream Documents

- docs/specs/x-recorded-verified-source-playback-export.md
- docs/adr/x-recorded-media-canonicalization-boundary.md
- docs/designs/x-recorded-verified-source-playback-export-design.md
- docs/discovery/x-recorded-verified-source-playback-export-discovery.md

## Design Inputs

- CanonicalSourceRef schema・整合 rule・`StreamEntry.raw_digest`・
  `COMMIT_SCHEMA_VERSION` bump (design doc 参照)。
- `publish_recorded` の ordering: blob → canonical.json (fail-closed)。
- fingerprint が `s.digest` を含むため canonical 有無で manifest は
  自動再構築される (prepare_manifest 既存機構)。
- frame correspondence 式: `expected[i][ch] = MockPcm::sample(
  source_start + i - engine_start, ch)`、clip 不在区間は 0。

## Tasks

| Task | 内容 | 依存 |
| --- | --- | --- |
| x-task1 | `staging`: canonical_rel/path/loader + `CanonicalSourceRef`;
  `commit`: `StreamEntry.raw_digest` + `load_canonical` 接続 +
  schema bump + unit test | — |
| x-task2 | `synvelis-player`: `ruviform-io-wav` dep + `publish.rs`
  (`publish_recorded` + `PublishReport`/`PublishError`) + unit test | x-task1 |
| x-task3 | `synvelis-render/tests/x_e2e.rs`: X-01..X-06 受入 test
  (録音→publish→commit→save/reload→playback→export→reopen) | x-task2 |
| x-task4 | `mise` 検証 + impl record + gap 記録 | x-task3 |

## File Map

- modify: `crates/synvelis-recording/src/staging.rs` (canonical schema/path/loader)
- modify: `crates/synvelis-recording/src/commit.rs` (raw_digest + 接続 + bump)
- modify: `crates/synvelis-player/Cargo.toml` (ruviform-io-wav dep)
- add: `crates/synvelis-player/src/publish.rs` (+ `lib.rs` export)
- modify: `crates/synvelis-render/Cargo.toml` (dev-dep synvelis-recording)
- add: `crates/synvelis-render/tests/x_e2e.rs`

## Risk Register

- canonical receipt と commit manifest の ordering ずれ →
  prepare_manifest の fingerprint 再構築 + fail-closed 整合で担保。
- f32 WAV が symphonia で `CodecId::Pcm` に map されるか → map.rs の
  PCM_IDS に `CODEC_ID_PCM_F32LE` 確認済み。
- MockInput の loop pass で input frame が source frame と 1:1 か →
  vs_d.rs の既存パターンで segment `source_range` から導出。

## Verification Matrix

- X-01: `x_normal_end_to_end` — publish→commit→reload→playback+export
  bit-exact。
- X-02: `x_loop_punch_preroll_per_segment` — pass/take/segment 対応。
- X-03: `x_identity_revision_undo_save` — ID/digest/revision/undo。
- X-04: `x_crash_and_partial_recovery_finalized_only` — 4 分岐。
- X-05: `x_fault_classes_separate` — 3 fault 系。
- X-06: 全 test が import_local 非使用で成立 (grep で検査可能)。
- mise run fmt-check/check/lint/test 全 PASS。

## Rollback

全変更は additive (新 module + optional field)。revert で VS-D/L1/L2
既存経路は不変。
