---
id: "IMPL-034XCKZR0STm7pAQ6dXHcS"
type: "impl"
status: "completed"
title: "ALIB-02 fault-injection followup"
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
    - "docs/tasks/alib-02-fault-injection-followup.md"
    - "docs/plans/alib-02-fault-injection-followup.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/ir/alib-02-residual-closeout.md"
    - "docs/impl/ir/alib-02-streaming-verified-import.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies:
    - "docs/test-specs/alib-02-streaming-verified-import.md"
  verified-by: []
  references:
    - "evidence/sv86/bookkeeping-verification.md"
    - "evidence/sv86/fault-injection-followup-verification.md"
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# ALIB-02 fault-injection followup

## Summary

`crates/synvelis-library/tests/import.rs` に残存 4 経路の独立
fault-injection test を追加し（21→25 tests）、spec AC 2 行を実在
test 名と 1:1 で `[x]` 化した。プロダクションコードの変更はない。
scripted double は fault *分類* の証拠であり実 durability 証拠では
ない — 実 OS/FS gate は synvelis#91 等の外部 gate が所有する。

## Context

`docs/tasks/alib-02-fault-injection-followup.md`（plan:
`docs/plans/alib-02-fault-injection-followup.md`、design:
`docs/designs/alib-02-fault-injection-followup.md`）に従う。
`evidence/sv86/bookkeeping-verification.md` の AC 突合表が確定した
残存 2 行（integrity 拒否 4 経路中 3 経路 + DirSync `Unsupported`）
を閉じる slice。Issue: synvelis#86（headless tracker 範囲。Issue は
外部 gate 残存のため open 継続）。

## Implementation

### Changes

`crates/synvelis-library/tests/import.rs`:

- `size_mismatch_is_integrity_error`（新規）— `ScriptedProvider` の
  bytes を使い intent の pin `expected_size` を +1 改変。
  `ImportError::IntegrityMismatch`、project 未変更・blob 未公開・
  job は `ImportStage::Started` 維持・新規 ImportId の誠実な import
  が収束することを assert。
- `staged_drift_is_corrupt`（新規）— `CancelOnEofProvider` で
  `Staged` まで進めて cancel → `source.bin` を改変 → 同一
  ImportId retry で `ImportError::Corrupt`。未 commit・blob 未公開・
  manifest `Staged` 維持を assert し、bytes 復元後に同一 ImportId で
  収束することを確認。
- `existing_blob_mismatch_is_integrity_error`（新規）— 正しい digest
  の `blobs/sha256/<hex>` に不一致 occupant を事前配置し
  `StagedBlobPublisher` 経由で import → `ImportError::IntegrityMismatch`。
  occupant 非上書き・未 commit・manifest `Verified` 維持を assert し、
  occupant 修復後に `BlobMaterialization::Reused` で収束。
- `DirSyncUnsupportedPublisher`（新規 double）+
  `directory_sync_unsupported_is_recorded_not_hidden`（新規）—
  実 publish を `StagedBlobPublisher` に delegate した上で receipt
  の `directory_sync` を `DirectorySync::Unsupported` に改変して返す。
  outcome `blob.directory_sync == Unsupported`・manifest
  `directory_sync == Some(Unsupported)`・stage `Committed`・commit
  収束を assert。test コメントに「分類証拠であり実 durability 証拠
  ではない」旨を明記。

`docs/specs/alib-02-streaming-verified-import.md`:

- AC「hash mismatch・size mismatch・staged drift・既存 blob 不一致を
  `IntegrityMismatch`/`Corrupt` として拒否」を `[x]` 化し、4 経路の
  実測 test 名を行末コメントに記録。
- AC「DirSync `Unsupported`/`Failed` が receipt/manifest に記録され
  publish 成否と分離される」を `[x]` 化し、`Unsupported` の注入方式が
  scripted 分類証拠である旨と外部 gate 残存をコメントに記録。

`docs/test-specs/alib-02-streaming-verified-import.md`:

- test 数記述を 21→25 に更新し、新規 4 tests の由来を追記。
- Idempotency/integrity Feature に `size_mismatch_is_integrity_error`・
  `staged_drift_is_corrupt`・`existing_blob_mismatch_is_integrity_error`
  を、Publish honesty Feature に
  `directory_sync_unsupported_is_recorded_not_hidden` を Example 追記。

### Decisions During Implementation

- size mismatch は provider 側 override ではなく intent pin の
  `expected_size` 改変で注入 — `check_pin_expectations` の同一経路を
  踏み、既存 `ScriptedProvider` を変更せずに済むため（design の
  Components and Boundaries どおり）。
- staged drift は publisher double での staged 改変ではなく、
  `CancelOnEofProvider` で `Staged` に留めてから `source.bin` を
  直接改変する方式を採用 — `verify_staged` の readback 再計測経路を
  忠実に再現するため。
- DirSync `Unsupported` は実 FS での発生を待たず receipt 改変で
  注入 — 分類の伝播・記録・publish/commit との分離の検証が目的で
  あり、実 durability は外部 gate の領分。
- experiment log（`docs/impl/exp/`）は作成しない — 実装は plan どおり
  一発で収束し、採用/却下を要する試行錯誤が無かったため。

## Related Experiments

### Adopted

- なし（experiment log 未作成。上記 Decisions 参照）

### Rejected

- なし

## Validation

実行環境: Windows（worktree `sv86-alib02fault`）、mise 管理
ツールチェーン。canonical = `mise run`、cargo 直接実行は補助証拠。

| Lane | Command | Result |
| --- | --- | --- |
| fmt | `mise run fmt-check` | Pass（exit 0） |
| check | `mise run check synvelis-library` | Pass（exit 0） |
| lint | `mise run lint synvelis-library` | Pass（exit 0、finding なし） |
| test | `mise run test synvelis-library` | Pass — unit 27・import 25・import_recovery 6・他 package suite すべて Pass、doc-test 失敗なし |
| build | `mise run build synvelis-library` | Pass（exit 0） |
| test-rt | `mise run test-rt` | Pass（exit 0、workspace `rt_` filter 全 test Pass） |
| test-msrv | `mise run test-msrv` | NotRun（POLICIES §5 により Daily Full CI 委譲） |

補助証拠: `cargo test -p synvelis-library --test import` →
`running 25 tests` / `25 passed; 0 failed`。

新規 4 tests の個別結果（`mise run test synvelis-library` 内で実測）:

- `size_mismatch_is_integrity_error` … ok
- `staged_drift_is_corrupt` … ok
- `existing_blob_mismatch_is_integrity_error` … ok
- `directory_sync_unsupported_is_recorded_not_hidden` … ok

AC 照合（spec `alib-02-streaming-verified-import.md`）:

| Spec AC 行 | 証拠 | 判定 |
| --- | --- | --- |
| integrity 拒否 4 経路 | `digest_mismatch_is_integrity_error`（既存）+ 新規 3 tests | Pass（headless/scripted 分類） |
| DirSync `Unsupported`/`Failed` 記録・分離 | `directory_sync_failure_is_recorded_not_hidden`（既存）+ `directory_sync_unsupported_is_recorded_not_hidden`（新規） | Pass（scripted 分類。実 durability は synvelis#91 等の外部 gate） |

### Doc 監査

- `doc_maintenance.js index apply --dir docs/designs|plans|tasks`
  および `sync_impl_index.js --kind ir --apply` で index を生成経路
  再生成。`docs/specs`・`docs/test-specs` は status/metadata 変更が
  無く `index plan` verdict=`in-sync`。全対象 dir で rescan
  `in-sync` を確認。
- `doc_status.js lint --type design|plan|task|impl` — 変更ファイル
  （`alib-02-fault-injection-followup.md`）の finding は 0 件
  （repo-wide baseline の legacy `invalid-id-*`・他ファイルの
  reciprocal warning は既存であり増やしていない）。
  `docs/specs/alib-02-streaming-verified-import.md` の
  `traceability-missing-upstream` と `docs/test-specs/
  alib-02-streaming-verified-import.md` の `invalid-id-prefix`
  （`TESTSPEC-alib02`）は bookkeeping-verification.md が記録した
  既存 baseline で、本 slice では変更していない。
- `audit_impl_record.js --dir docs/impl/ir` — 本ファイルの finding
  0 件（他ファイルの既存 finding は baseline）。
- review-protocol: cooperative/adversarial の 2 reviewer pass +
  validation を実施し、CONFIRMED blocker 0 件（P0/P1 なし）で
  ACCEPT。指摘は P3 のみ（commit 前の checkbox 更新で解消済み）。

## Risks

- scripted double は fault 分類の証拠に限定される。実 OS/FS の
  DirSync 可否・実 durability は未検証で、synvelis#91（実 OS/FS）・
  #97（統合試験）・#114（実 GPUI/GUI）・#331（real provider）・
  ruviform#294（cross-consumer/AudioLens）が所有する。Issue #86 は
  open 継続。
- `DirSyncUnsupportedPublisher` は receipt 改変であり、実 publisher が
  `Unsupported` を返す条件（`PublishFailureKind::Unsupported` や
  強い durability 要求の fallback）の網羅ではない。分類伝播の証拠と
  しては十分だが、発生条件の実測は外部 gate に残る。
- doc-driven-dev-graph の route 実行スクリプト
  （`.apm/skills/doc-driven-dev-graph/scripts/route_graph.js`）が
  worktree に存在せず route 呼び出しに失敗した。skill 本体
  （`.agents/skills/doc-driven-dev-graph/SKILL.md`）のフローに従って
  doc chain を作成した。ツール不在は環境差異として記録し、
  Coordinator の merge 時判断に委ねる。

## Follow-ups

- 実 OS/FS DirSync 実測・実 durability gate → synvelis#91。
- 統合試験・実 GUI・real provider・cross-consumer gate →
  synvelis#97/#114/#331・ruviform#294。
- Issue #86 close 判定 → 外部 gate 完了後に Coordinator が実施。
- docs index の merge 時再生成確認 → Coordinator。
