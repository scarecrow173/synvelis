---
id: "TASK-034XCIEQUzAmm8Nhw5iVPN"
type: "task"
status: "completed"
title: "ALIB-02 fault-injection followup — integrity/dirsync 拒否経路の専用 test 実装と AC 照合"
created: "2026-11-14"
updated: "2026-11-15"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/86"
  changes:
    added:
      - type: "file"
        path: "docs/designs/alib-02-fault-injection-followup.md"
      - type: "file"
        path: "docs/plans/alib-02-fault-injection-followup.md"
      - type: "file"
        path: "docs/tasks/alib-02-fault-injection-followup.md"
      - type: "file"
        path: "docs/impl/ir/alib-02-fault-injection-followup.md"
      - type: "file"
        path: "evidence/sv86/fault-injection-followup-verification.md"
    modified:
      - type: "file"
        path: "crates/synvelis-library/tests/import.rs"
      - type: "file"
        path: "docs/specs/alib-02-streaming-verified-import.md"
      - type: "file"
        path: "docs/test-specs/alib-02-streaming-verified-import.md"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/alib-02-fault-injection-followup.md"
  implemented-by: []
  depends-on:
    - "docs/plans/alib-02-fault-injection-followup.md"
    - "docs/designs/alib-02-fault-injection-followup.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/tasks/alib-02-residual-closeout.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "evidence/sv86/bookkeeping-verification.md"
  defers: []
  deferred-by: []
---

# ALIB-02 fault-injection followup — integrity/dirsync 拒否経路の専用 test 実装と AC 照合

## Work

`crates/synvelis-library/tests/import.rs` に残存 4 経路の独立
fault-injection test を追加し、spec AC 2 行を実在 test 名と 1:1 で
`[x]` 化する。プロダクションコードは変更しない。

## Classification

- `normal-plan-task`

## Upstream Documents

- `docs/plans/alib-02-fault-injection-followup.md`
- `docs/designs/alib-02-fault-injection-followup.md`
- `docs/specs/alib-02-streaming-verified-import.md`
- `docs/test-specs/alib-02-streaming-verified-import.md`
- `evidence/sv86/bookkeeping-verification.md`

## Dependencies

- `docs/plans/alib-02-fault-injection-followup.md`

## Affected Files

- `crates/synvelis-library/tests/import.rs` — 新規 4 tests +
  `DirSyncUnsupportedPublisher` double
- `docs/specs/alib-02-streaming-verified-import.md` — AC 2 行 `[x]` 化
- `docs/test-specs/alib-02-streaming-verified-import.md` — test 数・
  Example 更新
- `docs/designs|plans|tasks|impl/ir/alib-02-fault-injection-followup.md`
  — doc chain（新規）
- `evidence/sv86/fault-injection-followup-verification.md` — 検証
  ledger（新規）
- `docs/*/README.md` — 触れた dir の index（生成経路のみ）

## Done When

- [x] `size_mismatch_is_integrity_error` が pin `expected_size`
      不一致で `ImportError::IntegrityMismatch` を返し、未 commit・
      blob 未公開・job `Started` 維持・fresh import 収束を assert
      する。
- [x] `staged_drift_is_corrupt` が `CancelOnEofProvider` + staged
      bytes 改変で `ImportError::Corrupt` を返し、未 commit・blob
      未公開・manifest `Staged` 維持・bytes 復元後の同一 ImportId
      収束を assert する。
- [x] `existing_blob_mismatch_is_integrity_error` が occupant 不一致で
      `ImportError::IntegrityMismatch` を返し、occupant 非上書き・
      未 commit・manifest `Verified` 維持・修復後 `Reused` 収束を
      assert する。
- [x] `directory_sync_unsupported_is_recorded_not_hidden` が
      `DirSyncUnsupportedPublisher` で receipt `directory_sync` を
      `Unsupported` に注入し、outcome・manifest の双方に記録され
      publish/commit が独立に収束することを assert する（分類証拠
      であり実 durability 証拠ではない旨を明記）。
- [x] `mise run test synvelis-library` で新規含む全 test が非回帰
      Pass（import.rs 25/25・import_recovery.rs 6/6・他 suite 含む）。
- [x] spec AC 2 行が実在 test 名と 1:1 で `[x]` 化され、test-spec の
      test 数（25）と Example が実測と一致する。
- [x] canonical lane（fmt-check/check/lint/test/build/test-rt）・
      doc 監査・review-protocol・commit/push/PR が完了する。

## Verification

- [x] `mise run fmt-check` — exit 0
- [x] `mise run check synvelis-library` — exit 0
- [x] `mise run lint synvelis-library` — exit 0
- [x] `mise run test synvelis-library` — 全 test Pass
- [x] `mise run build synvelis-library` — exit 0
- [x] `mise run test-rt` — Pass（exit 0）
- [x] `doc_status.js lint`/`audit`・`audit_impl_record.js` — 変更 doc
      の整合確認（新規 finding なし）
- [x] `git diff --stat` で占有リスト外の変更なしを確認
