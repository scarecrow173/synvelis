---
id: "PLAN-034XCImELY72NyiUF1VVCf"
type: "plan"
status: "completed"
title: "ALIB-02 fault-injection followup — 残存拒否経路 test + spec AC 照合の実装計画"
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
  implements: []
  implemented-by:
    - "docs/tasks/alib-02-fault-injection-followup.md"
    - "docs/impl/ir/alib-02-fault-injection-followup.md"
  depends-on:
    - "docs/designs/alib-02-fault-injection-followup.md"
  blocks:
    - "docs/tasks/alib-02-fault-injection-followup.md"
  supersedes: []
  superseded-by: []
  related:
    - "docs/plans/alib-02-residual-closeout.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/alib-02-streaming-verified-import.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/alib-02-streaming-verified-import.md"
  references:
    - "evidence/sv86/bookkeeping-verification.md"
  defers: []
  deferred-by: []
---

# ALIB-02 fault-injection followup — 残存拒否経路 test + spec AC 照合の実装計画 Implementation Plan

## Goal

`tests/import.rs` に残存 4 つの integrity/dirsync 拒否経路の独立
fault-injection test を追加して実測し、spec AC 2 行を実在 test 名と
1:1 で `[x]` 化する。プロダクションコードの変更は含まない。

## Scope

単一 stream の test + docs/evidence 変更。分割不要。
編集境界は worker spec の占有リストに限定する。

## Upstream Documents

- `docs/specs/alib-02-streaming-verified-import.md`（受入条件の正本）
- `docs/test-specs/alib-02-streaming-verified-import.md`（test 仕様）
- `docs/designs/alib-02-fault-injection-followup.md`（設計入力）
- `evidence/sv86/bookkeeping-verification.md`（AC 突合の入力）
- Issue: <https://github.com/scarecrow173/synvelis/issues/86>

## Design Inputs

- `docs/designs/alib-02-fault-injection-followup.md`

## File Map

| File | 変更 |
| --- | --- |
| `crates/synvelis-library/tests/import.rs` | 新規 4 tests + `DirSyncUnsupportedPublisher` double |
| `docs/specs/alib-02-streaming-verified-import.md` | AC 2 行の `[x]` 化と実測根拠コメント更新 |
| `docs/test-specs/alib-02-streaming-verified-import.md` | test 数 21→25・新規 test の Example 追記 |
| `docs/designs/alib-02-fault-injection-followup.md` | 設計（新規） |
| `docs/plans/alib-02-fault-injection-followup.md` | 本計画（新規） |
| `docs/tasks/alib-02-fault-injection-followup.md` | task checklist（新規） |
| `docs/impl/ir/alib-02-fault-injection-followup.md` | 実装記録（新規） |
| `evidence/sv86/fault-injection-followup-verification.md` | lane 実測・AC 照合 ledger（新規） |
| `docs/*/README.md`（触れた dir の index） | doc-maintenance の生成経路でのみ再生成 |

## Implementation Sequence

- [x] `tests/import.rs` に 4 tests と `DirSyncUnsupportedPublisher`
      を追加（pin `expected_size` 改変 / `CancelOnEofProvider` +
      staged bytes 改変 / occupant 事前配置 / receipt `directory_sync`
      改変の 4 注入方式）
- [x] `cargo test -p synvelis-library --test import` で 25/25 Pass を
      確認（新規 4 tests 含む）
- [x] canonical lane（fmt-check/check/lint/test/build/test-rt）の
      `mise run` 実走と結果記録
- [x] spec AC 2 行を実在 test 名と 1:1 で `[x]` 化、test-spec の
      test 数・Example を更新
- [x] doc chain（design/plan/task/IR）の作成
- [x] docs index を doc-maintenance の生成経路で再生成し、
      doc_status lint/audit・audit_impl_record で新規 finding なしを
      確認
- [x] review-protocol レビューで blocker なしを確認
- [x] `git merge origin/main` で最新 main を取り込み、生成物を
      再生成してコミット・push・PR 作成

## Dependencies and Risks

- プロダクションコードは変更しない前提。分類経路
  （`check_pin_expectations`/`verify_staged`/`verify_existing_blob`/
  DirSync 分類）が test の想定と異なる場合は test 側の注入点を
  見直す。分類自体の不具合が判明した場合のみ scope 拡張を
  Coordinator に相談する。
- `mise run test-rt` は workspace 全 test を対象とするため最長 lane。
- `mise run test-msrv` は POLICIES §5 どおり Daily Full CI 委譲で
  NotRun 記録可。
- 並行 Worker が同 repo で稼働中のため、占有リスト外の file に
  触れないことを diff で最終確認する。

## Task Breakdown

- `docs/tasks/alib-02-fault-injection-followup.md` — 本 plan の
  実施 task（test 追加・lane 実走・AC 照合・doc/evidence 作成・
  review・commit/push/PR）。

## Verification

- [x] `mise run fmt-check` — exit 0
- [x] `mise run check synvelis-library` — exit 0
- [x] `mise run lint synvelis-library` — exit 0（lint finding なし）
- [x] `mise run test synvelis-library` — 全 package test Pass
      （unit 27・import 25・import_recovery 6・他 suite 含む）
- [x] `mise run build synvelis-library` — exit 0
- [x] `mise run test-rt` — Pass（exit 0、workspace `rt_` filter 全
      test Pass）
- [x] `doc_status.js lint` / `audit`・`audit_impl_record.js` —
      変更 doc の整合確認（新規 finding なし。結果は IR Validation）
- [x] `git diff --stat` で占有リスト外の変更が無いことを確認

## Open Questions / Gaps

- 実 OS/FS の DirSync `Unsupported`/`Failed` 発生条件と durability
  実測は synvelis#91 の外部 gate が所有する。本 slice は分類経路の
  証拠のみを固定する。
- Issue #86 の close 判定は外部 gate 完了後に Coordinator が行う。

## Self-Review

- [x] Every spec requirement maps to a task.
- [x] No placeholders remain.
- [x] File paths, relations, status values, and terminology are consistent.

## Implementation Handoff

- Recommended mode: inline execution（worker spec の指示どおり
  単一 worker が実施。subagent 不使用）
- Independent work streams: none
- User approval required before dispatch: yes — review-protocol の
  blocker なしを承認として扱う
- If approved: execute task-by-task with review checkpoints.

## Review Handoff

- scripted double は fault *分類* の証拠であり実 durability 証拠では
  ない点、spec AC の `[x]` 化が実走 test のみに基づく点、
  `test-msrv` の NotRun 記録の妥当性を reviewer に確認してもらう。
