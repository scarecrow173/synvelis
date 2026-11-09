---
id: "TASK-034UMa7Lwxo6NvKrTf601Q"
type: "task"
status: "done"
title: "RCD-02 Task2 Project/Library Adapter (verified SHA-256 + MediaReferenceProbe + saved-HEAD)"
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
  implements:
    - "docs/specs/rcd-02-durability-evidence-recovery-review.md"
  implemented-by: []
  depends-on:
    - "docs/plans/rcd-02-durability-recovery-review.md"
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
---

# RCD-02 Task2 Project/Library Adapter (verified SHA-256 + MediaReferenceProbe + saved-HEAD)

## Work

spec の Deferred Design Concerns にあった project/library 統合
adapter slice を実装する（Task1 plan の file scope 外・別 dispatch
として Issue #116 の残件を消化）。

1. `writer.rs`: `FinalizedSourceRef.verified_sha256`
   （`sha256:<64hex>`）を追加。`verified_prefix_sha256` が durable
   frontier（`media_byte_len`）までの media byte 列のみを SHA-256
   で hash し、unverified/journaled tail を含めない。finalize で
   fail-closed に記録（hash 失敗 = finalize 失敗）。
2. `recovery.rs`: `recover_stream_finalize_faultable`（Adopt 経路）
   でも同一 field を記録。
3. `commit.rs`: `StreamEntry.verified_sha256`（serde default +
   skip_serializing_if で旧 manifest との payload hash 互換維持）
   を追加し、`fingerprint` にも含める。`build_payload` が finalized
   ref から転記する。CRC32 fold（`digest`/`digest_scheme`）とは
   別 namespace の verified-blob identity で #86 `ContentDigest`
   と同一 `"<alg>:<hex>"` 形式。
4. `media_probe.rs`（新規）: `ProjectMediaProbe` —
   `MediaReferenceProbe` の project 実装。live project・undo/redo
   履歴（`ProjectController::history_states`）・saved HEAD
   snapshot（`ProjectStore::head`/`read_snapshot`）を read-only で
   調べ、`applied_commits`・当該 stream の take/segment・
   Open/Created session 状態を参照と判定する。HEAD の schema
   不一致・decode 失敗・store error・project_id 不一致は全て
   `Unknown`（fail-closed）。`CompositeProbe` で複数 probe を合成
   （Referenced 優先・Unknown を握り潰さない）。
5. `synvelis-project`: `ProjectController::history_states`
   （undo/redo snapshot の read-only 走査）と `store.rs` の
   `snapshot_revision`/`head_revision`（canonical `r<rev>.json`
   名の revision 復元・HEAD→revision adapter）を追加。
6. `tests/project_adapter.rs`: verified-prefix SHA-256 の
   finalize/recover/commit-manifest 転記、probe の
   Clear/Referenced/Unknown 全経路（live・履歴・saved HEAD・
   Open session・orphan staging・破損/旧 schema/別 project HEAD）、
   `CompositeProbe` semantics、`head_revision`→`commit_receipt`
   の `project_saved` 接続を検証。

## Classification

- `normal-plan-task`

## Upstream Documents

- `docs/specs/rcd-02-durability-evidence-recovery-review.md`
  （Deferred Design Concerns の adapter slice）
- `docs/plans/rcd-02-durability-recovery-review.md`
- `docs/tasks/rcd-02-task1-durability-evidence-recovery-review.md`

## Dependencies

- Task1（durability evidence / recovery review / commit receipt）
  の main 取り込み済み実装に依存。
- #86 verified blob identity とは `sha256:<64hex>` 形式で整合
  （実接続は #86 gate・本 slice は provenance 記録まで）。
- #84 StagedFile/CAS 実接続は別 gate（本 slice は HEAD snapshot の
  read-only 参照判定のみ）。

## Affected Files

- `crates/synvelis-recording/src/{writer,recovery,commit,media_probe,lib}.rs`
- `crates/synvelis-recording/Cargo.toml`（`sha2` 追加）
- `crates/synvelis-project/src/{controller,store,lib}.rs`
- `crates/synvelis-recording/tests/project_adapter.rs`

## Done When

- [x] `FinalizedSourceRef`/`StreamEntry` が verified prefix の
      SHA-256 content digest を保持し、unverified tail を含まない
      ことを試験で固定。
- [x] `MediaReferenceProbe` の project adapter が live・undo/redo
      履歴・saved HEAD の参照を fail-closed に判定し、
      `Discard` の Clear/Referenced/Unknown を試験で固定。
- [x] saved HEAD revision → `commit_receipt.project_saved` の
      接続が試験で固定。
- [x] 旧 manifest（`verified_sha256` 欠落）の decode/payload hash
      互換が維持される。

## Verification

- [x] `mise run fmt-check` / `check` / `lint` / `test` / `test-rt` /
      `build` / `test-msrv` — 全 lane exit 0。
- [x] `cargo test -p synvelis-recording --test project_adapter`
      12 tests pass。
- [ ] 実 OS/FS durability 実測・実 device fault・#118 製品 GUI —
      NotRun（実機/別 gate）。
