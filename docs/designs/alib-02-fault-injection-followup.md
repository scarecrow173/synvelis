---
id: "DESIGN-034XCHS23zpAY0YcXEkGAP"
type: "design"
status: "approved"
title: "ALIB-02 fault-injection followup — 残存 integrity/dirsync 拒否経路の専用 test 設計"
created: "2026-11-14"
updated: "2026-11-15"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/86"
    - "docs/specs/alib-02-streaming-verified-import.md"
    - "docs/test-specs/alib-02-streaming-verified-import.md"
    - "evidence/sv86/bookkeeping-verification.md"
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
  implemented-by: []
  depends-on: []
  blocks:
    - "docs/plans/alib-02-fault-injection-followup.md"
    - "docs/tasks/alib-02-fault-injection-followup.md"
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/alib-02-streaming-verified-import-design.md"
    - "docs/designs/alib-02-residual-closeout-design.md"
    - "docs/designs/alib-07-fault-matrix-design.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/alib-02-streaming-verified-import.md"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "evidence/sv86/bookkeeping-verification.md"
  defers: []
  deferred-by: []
---

# ALIB-02 fault-injection followup — 残存 integrity/dirsync 拒否経路の専用 test 設計

## Context

ALIB-02 の headless slice は PR #220/#253/#326/#328/#339 で main に
統合済みで、`src/import.rs` には integrity 拒否経路
（`check_pin_expectations` の digest/size 照合、`verify_staged` の
readback 再計測、`verify_existing_blob` の occupant 照合）と
DirSync 分類（`PublishPhase::DirectorySync` → `Failed`、
`PublishFailureKind::Unsupported` → `Unsupported`）が実装済みである。

一方 `evidence/sv86/bookkeeping-verification.md` の AC 突合表は、
spec の acceptance 行 2 件を「実装済みだが専用 test が未整備」として
未チェック維持と判定した:

1. hash mismatch・size mismatch・staged drift・既存 blob 不一致を
   `IntegrityMismatch`/`Corrupt` として拒否 — hash mismatch のみ
   `digest_mismatch_is_integrity_error` で実測済み。size mismatch・
   staged drift・既存 blob 不一致の 3 経路は実装済みだが専用 test が
   無い。
2. DirSync `Unsupported`/`Failed` が receipt/manifest に記録され
   publish 成否と分離される — `Failed` は
   `directory_sync_failure_is_recorded_not_hidden` で実測済み。
   `Unsupported` は scripted 注入経路が未整備。

本設計はこの残存 4 経路を `crates/synvelis-library/tests/import.rs`
の scripted double で独立 test として実測し、spec AC 行を実在 test
名と 1:1 で照合するための最小設計である。プロダクションコードの
変更は含まない。

## Scope

### In Scope

- `tests/import.rs` への 4 つの独立 fault-injection test 追加:
  1. `size_mismatch_is_integrity_error` — pin `expected_size` と
     実測 `byte_len` の不一致 → `ImportError::IntegrityMismatch`、
     job は `Started` に留まり resume 可能、誠実な再 import が収束。
  2. `staged_drift_is_corrupt` — `CancelOnEofProvider` で stage 後に
     中断し `source.bin` を改変 → 同一 ImportId retry で
     `ImportError::Corrupt`、manifest は `Staged` のまま、bytes 復元後に
     同一 ImportId で収束。
  3. `existing_blob_mismatch_is_integrity_error` —
     `media/blobs/sha256/<hex>` に不一致 occupant を事前配置 →
     `ImportError::IntegrityMismatch`、occupant 非上書き、manifest は
     `Verified` のまま、occupant 修復後に `Reused` で収束。
  4. `directory_sync_unsupported_is_recorded_not_hidden` —
     `DirSyncUnsupportedPublisher`（実 publish を
     `StagedBlobPublisher` に delegate し receipt の
     `directory_sync` を `Unsupported` に改変）が outcome・
     manifest の両方に `Unsupported` を記録し、publish/commit が
     独立に収束することを assert。
- 各 test が「未 commit・blob 未上書き・誤 reuse なし」を assert する
  こと。
- spec AC 2 行の `[x]` 化（実在 test 名を行末コメントに記録）と
  test-spec の test 数・Example 更新。
- `evidence/sv86/` への検証 ledger 追記と doc chain
  （design/plan/task/IR）の作成。

### Out of Scope

- `src/import.rs` 等プロダクションコードの変更（分類は実装済み。
  変更が必要になった場合のみ最小限で Coordinator に scope 拡張を
  相談する）。
- 実 OS/FS の DirSync 可否の観測 — scripted double は fault
  *分類* の証拠であり実 durability 証拠ではない。実 FS gate は
  synvelis#91 が所有する。
- ALIB-07 fault matrix（synvelis#91 所有）や他 crate の fault
  網羅。
- Issue #86 の close・PR merge・docs index の手編集。

## Components and Boundaries

- **注入点の選択（本設計の中核）**: 各拒否経路が最初に破れる地点に
  fault を置き、typed error の境界で観測する。
  - size mismatch: `ScriptedProvider` の bytes をそのまま使い、
    intent の pin `expected_size` を改変して `check_pin_expectations`
    （staging 中の期待値照合）を踏む。provider 側に size override を
    増やすより pin 改変の方が intent 形状を変えず既存 double を
    再利用できる。
  - staged drift: `CancelOnEofProvider`（既存、EOF で cancel）で
    `Staged` まで進めてから `source.bin` を直接改変し、同一
    ImportId で retry して `verify_staged` の readback 再計測を踏む。
    publisher 側の staged 改変 double は不要 — verification 段階の
    drift を直接再現する方が経路を忠実に写す。
  - 既存 blob 不一致: `LocalBlobDir` の `sha256:<hex>` →
    `blobs/sha256/<hex>` 写像に従い occupant を事前配置。
    `publish_blob` の stat → `verify_existing_blob` 経路を踏む。
    occupant はテスト後に assert で非破壊を確認する。
  - DirSync `Unsupported`: `DirSyncFailPublisher`（:769 系）と同型の
    `DirSyncUnsupportedPublisher` を新設。実 publish を
    `StagedBlobPublisher` に delegate してから receipt の
    `directory_sync` を `Unsupported` に書き換えて返す。
    publish 効果（`Created`）と DirSync 分類を分離して観測できる。
- **正直性の境界**: scripted double は fault 分類の証拠であり、
  実 durability・実 OS/FS 挙動の証拠ではない。この区別を test 名・
  コメント・IR・evidence・PR 本文のすべてに明記する。
- **占有境界**: 編集は worker spec の占有リスト
  （`tests/import.rs`・spec/test-spec 2 件・doc chain 5 件・
  `evidence/sv86/`）に限定する。

## Data and Control Flow

1. `ScriptedProvider`/`ScriptedPublisher`/`CancelOnEofProvider`/
   `StagedBlobPublisher`/`ImportStore`/`ProjectController`/`TempTree`
   の既存 helper を再利用して import を駆動する。
2. 各 test は fault 注入 → `run_import`/`resume` の結果が期待する
   `ImportError` variant であることを assert。
3. 失敗後の不変条件を assert: `ProjectController` の
   applied imports/assets/sources が増えていない、blob path が
   存在しない/occupant が不変、manifest の stage が期待値
   （`Started`/`Staged`/`Verified`）のまま。
4. 復旧可能性を assert: bytes/occupant を修復して同一 ImportId で
   retry し、収束（`Committed`・`Reused`/`Created`）を確認する。
   size mismatch のみ job が `Started` に留まるため「誠実な fresh
   import（新規 ImportId）」での収束を確認する。
5. DirSync test は outcome `blob.directory_sync == Unsupported`・
   manifest `directory_sync == Some(Unsupported)`・commit 収束を
   三点で assert し、publish 成否と DirSync 分類の分離を示す。

## Non-Functional Constraints

- **誠実性不変条件**: NotRun/Unsupported/Mock を Pass と書かない。
  scripted double の結果は分類証拠に限定し、実 durability は
  synvelis#91 の外部 gate に残す。
- **非回帰**: 既存 21 tests + `import_recovery.rs` 6 tests を含む
  `synvelis-library` 全 test が Pass であること。
- **決定性**: test は `TempTree` 配下の一時 root と scripted double
  のみを使い、実 provider・ネットワーク・GUI に依存しない。
- **占有境界**: 並行 Worker が稼働中のため占有リスト外に触れない。
  docs index は `doc-maintenance`/`doc_status.js` の生成経路のみで
  更新し手編集しない。

## Risks and Trade-offs

- **staged drift の再現方法**: `verify_staged` は stage 完了後の
  retry 時に走るため、`CancelOnEofProvider` で `Staged` に留めて
  から bytes を改変する方式を採用。代替案（publisher double が
  staged_source を改変）は publish 段階の別経路を踏むため採用せず、
  より忠実な verification 段階の drift を直接再現した。
- **occupant 配置の path 解決**: `LocalBlobDir::object_rel` の
  namespaced 写像を test 側で複製すると drift リスクがある →
  test は digest から `blobs/sha256/<hex>` を直接組み立て、
  `stat`/`open` が同じ写像を使うことを実測で確認する形に留める。
- **DirSync `Unsupported` の意味論**: 実 publisher は
  `PublishFailureKind::Unsupported` または強い durability 要求の
  fallback で `Unsupported` を返し得る。本 test は receipt 改変で
  分類の伝播・記録・分離のみを検証し、実 FS での発生条件は
  外部 gate に委ねる — 分類経路の証拠としては十分で、過剰な主張を
  避ける。
- **flake**: 全 test は in-process + tempdir で外部依存がなく、
  既存 lane の flake リスクは低い。

## Deferred Design Concerns

- 実 OS/FS での DirSync `Unsupported`/`Failed` の発生条件と
  durability 実測 | 本環境は Windows headless で実 FS gate は
  synvelis#91 が所有 | synvelis#91 完了時 | 分類証拠を実 durability
  証拠と誤読するリスク。
- size mismatch を provider 側で注入する variant | pin 改変で
  同じ `check_pin_expectations` 経路を踏めるため不要 | provider
  契約側に size 報告経路を追加する変更が入った時 | pin と実測の
  どちらが嘘をついたかの切り分け粒度。
- Issue #86 の close | 外部 gate（#91/#97/#114/#331/ruviform#294）
  が残存 | gate 所有 Issue の完了後 | 未証明行を閉じたままにする
  誤判定。

## Verification Notes

- [x] `mise run fmt-check` / `check synvelis-library` /
  `lint synvelis-library` / `test synvelis-library` /
  `build synvelis-library` / `test-rt` を実走し exit code 0 を確認
  （結果は `docs/impl/ir/alib-02-fault-injection-followup.md`
  Validation と `evidence/sv86/fault-injection-followup-verification.md`）。
- [x] `cargo test -p synvelis-library --test import` で 25/25 Pass
  （新規 4 tests を含む）を確認。
- [x] spec AC 2 行を実在 test 名と 1:1 で `[x]` 化し、test-spec の
  test 数（21→25）と Example を更新。
