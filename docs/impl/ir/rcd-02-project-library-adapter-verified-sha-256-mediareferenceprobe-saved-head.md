---
id: "IMPL-034UMbAviqhaRhIqExdBM3"
type: "impl"
status: "completed"
title: "RCD-02 Project/Library Adapter (verified SHA-256 + MediaReferenceProbe + saved-HEAD)"
created: "2026-11-09"
updated: "2026-11-09"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/116"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-recording/src/media_probe.rs"
      - type: "file"
        path: "crates/synvelis-recording/tests/project_adapter.rs"
      - type: "file"
        path: "docs/tasks/rcd-02-task2-project-library-adapter.md"
      - type: "file"
        path: "docs/impl/ir/rcd-02-project-library-adapter-verified-sha-256-mediareferenceprobe-saved-head.md"
    modified:
      - type: "file"
        path: "crates/synvelis-project/src/controller.rs"
      - type: "file"
        path: "crates/synvelis-project/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-project/src/store.rs"
      - type: "file"
        path: "crates/synvelis-recording/Cargo.toml"
      - type: "file"
        path: "crates/synvelis-recording/src/writer.rs"
      - type: "file"
        path: "crates/synvelis-recording/src/recovery.rs"
      - type: "file"
        path: "crates/synvelis-recording/src/commit.rs"
      - type: "file"
        path: "crates/synvelis-recording/src/lib.rs"
      - type: "file"
        path: "Cargo.lock"
      - type: "file"
        path: "docs/tasks/README.md"
  implements:
    - "docs/tasks/rcd-02-task2-project-library-adapter.md"
    - "docs/specs/rcd-02-durability-evidence-recovery-review.md"
  implemented-by: []
  depends-on:
    - "docs/tasks/rcd-02-task1-durability-evidence-recovery-review.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/rcd-02-durability-evidence-recovery-review.md"
    - "docs/test-specs/rcd-02-durability-recovery-review-fault-matrix.md"
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

# RCD-02 Project/Library Adapter (verified SHA-256 + MediaReferenceProbe + saved-HEAD)

## Summary

Issue #116 RCD-02 の project/library 統合 adapter slice を実装した。
spec の Deferred Design Concerns（Task1 plan の file scope 外・別
dispatch 担当と明記されていた部分）を消化する。

- verified prefix の SHA-256 content digest（`verified_sha256`、
  `sha256:<64hex>`）を `FinalizedSourceRef`・`StreamEntry` に追加し、
  finalize/recovery-Adopt/commit manifest の全経路で記録する。
- `MediaReferenceProbe` の project 実装 `ProjectMediaProbe` と
  `CompositeProbe` を新設し、staging media の Discard 判定を
  live project・undo/redo 履歴・saved HEAD snapshot へ接続した。
- `ProjectController::history_states` と `snapshot_revision`/
  `head_revision` により、未保存履歴と saved HEAD の revision を
  read-only で観測できるようにした。

## Context

- Task: `docs/tasks/rcd-02-task2-project-library-adapter.md`
- Spec: `docs/specs/rcd-02-durability-evidence-recovery-review.md`
  （Deferred Design Concerns の adapter slice）
- Plan/Task1: `docs/plans/rcd-02-durability-recovery-review.md`、
  `docs/tasks/rcd-02-task1-durability-evidence-recovery-review.md`
  （durability evidence / recovery review / commit receipt、
  main 取り込み済み）
- Issue: <https://github.com/scarecrow173/synvelis/issues/116>

## Implementation

### Changes

- `crates/synvelis-recording/src/writer.rs`: `FinalizedSourceRef.
  verified_sha256`（serde default + skip_serializing_if）と
  `verified_prefix_sha256`（durable frontier の `media_byte_len`
  までを SHA-256、tail を含めない、失敗は finalize 失敗）を追加。
- `crates/synvelis-recording/src/recovery.rs`:
  `recover_stream_finalize_faultable` で同 field を記録。
- `crates/synvelis-recording/src/commit.rs`: `StreamEntry.
  verified_sha256`（旧 manifest 互換の serde 属性）を追加し
  `fingerprint`/`build_payload` に組み込み。
- `crates/synvelis-recording/src/media_probe.rs`（新規）:
  `ProjectMediaProbe`（live・`history_states`・saved HEAD の
  read-only 参照判定。`applied_commits`/stream take/segment/
  Open・Created session を参照とし、HEAD の schema 不一致・
  decode 失敗・store error・project_id 不一致は `Unknown`）と
  `CompositeProbe`（Referenced 優先・Unknown 保持の合成）。
- `crates/synvelis-project/src/controller.rs`:
  `history_states`（undo/redo `&Project` 走査）。
- `crates/synvelis-project/src/store.rs`: `snapshot_revision`
  （canonical `r<rev>.json` の逆変換）と `head_revision`
  （HEAD→`Option<ProjectRevision>`、非 canonical 名は Store error）。
- `crates/synvelis-recording/Cargo.toml`: `sha2` 依存追加。
- `crates/synvelis-recording/tests/project_adapter.rs`（新規）:
  12 test。

### Decisions During Implementation

- `verified_sha256` は `digest`/`digest_scheme`（CRC32 integrity
  fold・暫定 canonical digest）とは別 namespace とし、#86
  `ContentDigest` と同一の `"<alg>:<hex>"` 形式に揃えた。
  unverified/journaled tail は hash 対象外（`media_byte_len` まで）。
- probe の参照正本は recording registry の stream 結合 entity
  （`applied_commits`・`takes.stream_id`・`segments.stream_id`）と
  session 状態。clip/comp/take group はこれら経由でのみ media
  source に到達し得るため推移的に覆われる。
- Open/Created session は「録音継続中の可能性がある media」として
  live・履歴の双方で Referenced とした（undo で Open 状態へ戻り
  得るため履歴側も対象）。orphan staging（project 未登録
  session）は Clear になり得る。
- saved HEAD は schema_version 一致・decode 可能・project_id
  整合の場合のみ評価し、それ以外は `Unknown`（旧 snapshot を
  推測で読まない fail-closed）。
- `finalize()` は journaled tail を暗黙 checkpoint で durable に
  昇格するため、Partial を作る試験では unverified media tail を
  使用した（仕様通りの動作）。

## Related Experiments

### Adopted

<!-- なし -->

### Rejected

<!-- なし -->

## Validation

- `mise run fmt-check` — Pass
- `mise run check` — Pass
- `mise run lint` — Pass
- `mise run test` — Pass（workspace 全 test、`project_adapter`
  12 tests 含む）
- `mise run test-rt` — Pass
- `mise run build` — Pass
- `mise run test-msrv`（rust 1.89 `cargo test --workspace`）— Pass

NotRun: 実 OS/FS durability 実測・実 device/LowDisk/disconnect/
crash fault・#118 製品 GUI E2E（実機/別 gate）。

## Risks

- `verified_sha256` は provenance 記録であり、#86 の verified blob
  import/Asset 登録・#84 CAS への実接続は未実施（各 gate）。
- `ProjectMediaProbe` は `synvelis-project` snapshot の read-only
  参照のみ。store への書き込み・migration は行わない。
- Open/Created session を履歴側でも Referenced とする保守的判定は
  undo 履歴が retain する間 Discard を拒否し続ける（fail-closed
  優先の意図的な保守性）。

## Follow-ups

- #86 verified blob identity との実接続（`verified_sha256` を
  `ContentDigest` 検証・Asset 登録へ接続）。
- #84 StagedFile/HEAD CAS との接続（saved HEAD の snapshot bytes
  ではなく CAS object を参照する形への発展）。
- #118 製品 Comp/Take/Recovery GUI E2E。
