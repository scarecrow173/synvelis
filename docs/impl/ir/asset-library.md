---
id: "IMPL-034X9CHvMBdaJaTDZHyjpx"
type: "impl"
status: "completed"
title: "Asset Library 設計契約 実装記録 (synvelis#9)"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/9"
  changes:
    added:
      - type: "file"
        path: "docs/specs/asset-library.md"
      - type: "file"
        path: "docs/designs/asset-library-design.md"
      - type: "file"
        path: "docs/plans/asset-library.md"
      - type: "file"
        path: "docs/tasks/asset-library-task.md"
      - type: "file"
        path: "docs/test-specs/asset-library.md"
      - type: "file"
        path: "docs/impl/ir/asset-library.md"
      - type: "file"
        path: "docs/impl/exp/asset-library.md"
      - type: "file"
        path: "evidence/sv9/asset-library-case-ledger.md"
      - type: "file"
        path: "evidence/sv9/asset-library-verification.md"
    modified: []
    deleted: []
    renamed:
      - type: "file"
        from: "docs/impl/ir/asset-library-synvelis-9.md"
        to: "docs/impl/ir/asset-library.md"
    moved: []
    generated:
      - type: "file"
        path: "docs/specs/README.md"
      - type: "file"
        path: "docs/designs/README.md"
      - type: "file"
        path: "docs/plans/README.md"
      - type: "file"
        path: "docs/tasks/README.md"
      - type: "file"
        path: "docs/test-specs/README.md"
      - type: "file"
        path: "docs/impl/ir/README.md"
      - type: "file"
        path: "docs/impl/exp/README.md"
  implements:
    - "docs/tasks/asset-library-task.md"
  implemented-by: []
  depends-on:
    - "docs/tasks/asset-library-task.md"
    - "docs/plans/asset-library.md"
    - "docs/designs/asset-library-design.md"
    - "docs/specs/asset-library.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/alib-01-asset-provider-contract.md"
    - "docs/specs/alib-07-fault-matrix.md"
    - "docs/test-specs/alib-07-fault-matrix.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/tasks/asset-library-task.md"
  derived-by: []
  verifies:
    - "docs/test-specs/asset-library.md"
  verified-by: []
  references:
    - "docs/specs/asset-library.md"
    - "docs/designs/asset-library-design.md"
    - "docs/plans/asset-library.md"
    - "docs/tasks/asset-library-task.md"
    - "docs/test-specs/asset-library.md"
    - "docs/impl/exp/asset-library.md"
    - "evidence/sv9/asset-library-case-ledger.md"
    - "evidence/sv9/asset-library-verification.md"
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted:
      - "docs/impl/exp/asset-library.md"
    rejected: []
---

# Asset Library — Issue #9 設計契約文書化 + LIB evidence 台帳

## Summary

Issue #9「Remote-aware Asset Library — metadata-first remote
browse・pinned intent・verified import・offline/permission UX・
失敗 Case」の canonical 設計契約を docs/ 7 文書鎖
（spec/design/plan/task/test-spec/impl-ir/impl-exp）+
`evidence/sv9/`（case ledger・verification）として作成した。
既存 ALIB-01〜07 の provider/import/cache/fault 契約を Synvelis
製品の Browser UX・ImportIntent・coverage/capability typed
state・LIB-* 受入台帳として束ね、#89（UX）・#91（fault
matrix）・#85〜#90（provider/import/cache）・#97（VCS
snapshot）・#114（実 GUI）・#331（LFS remote fetch）・Ruviform
共通契約との境界を固定した。**docs/ 設計契約 + evidence のみ**
であり、実装コード・GUI・実 device/provider/OS 受入は対象外。

## Context

- 対象 Issue: synvelis #9
- ブランチ: `docs/9-asset-library`
- 上流正本: Issue #9 本文（Parent/Design Owner）・#89
  （Browser/Import UX）・#91（LIB-* fault matrix）
- 既存契約: ALIB-01（provider 境界）・ALIB-02（verified
  import・v5 互換）・ALIB-03（catalog/preview cache）・
  ALIB-04（Git provider・LFS）・ALIB-06（WebDAV）・
  ALIB-07（fault matrix・import cancel）
- 関連 Owner: #85・#86・#87・#88・#89・#90・#97・#114・#331・
  #84・Ruviform #219/#294（参照のみ）

## Implementation

### Changes

- `docs/specs/asset-library.md`: AL-R-01〜17 要件・AC-1〜16
  acceptance criteria・Scope・Deferred Design Concerns・
  Sources。
- `docs/designs/asset-library-design.md`: Owner 分離表・
  Key Contracts（identity/coverage-capability/pin/import
  intent/cache-sec-qos/migration）・data/control flow・
  状態機械（BrowseCoverage/PinState/ImportJob）・failure
  taxonomy（LIB-* 写像）・UX 行動仕様・risks・deferred・
  verification notes。
- `docs/plans/asset-library.md`: goal/scope/upstream/file
  map/3 implementation streams/dependency graph/risk
  register/verification plan/rollback/task breakdown。
- `docs/tasks/asset-library-task.md`: work/classification/
  upstream/dependencies/affected files/done-when/verification/
  out-of-scope。
- `docs/test-specs/asset-library.md`: AL-T-R-01〜08 rules・
  LIB-* case map・5 examples・guarantees・non-goals・risk。
- `docs/impl/ir/asset-library.md`: 本記録。
- `docs/impl/exp/asset-library.md`: 試行記録。
- `evidence/sv9/asset-library-case-ledger.md`: LIB-* case
  → fixture/oracle/Owner/mode/layer/status/diagnostic/link
  の台帳。
- `evidence/sv9/asset-library-verification.md`: 実行した
  検証・OS/toolchain・exit code・結果。
- `docs/{specs,designs,plans,tasks,test-specs,impl/ir,
  impl/exp}/README.md`: `doc-maintenance index apply`・
  `sync_impl_index` で再生成（手編集なし）。

### Decisions During Implementation

- **D-1 docs-only slice**: Issue #9 は Parent/Design Owner
  であり、provider/import/cache/UI の実装は #85〜#91 が
  Owner。本 slice は正本文書鎖と evidence のみを作成し、
  crate・CI・Cargo・Ruviform 共通契約を変更しない。
- **D-2 AL-R-01〜17 + AC-1〜16**: Issue #9 の 12 受入条件を
  ID/BROWSE/PIN/IMPORT/MIGRATION/CACHE-SEC/FAULT-OS・UX の
  要件番号と acceptance criteria へ整理し、#91 の LIB-*
  分類へ写像。
- **D-3 typed state の固定**: coverage（Complete/Partial/
  Stale/Offline/IndexIncomplete/Unsupported/Unauthorized/
  PermissionDenied/Empty）・capability・pin/import job 状態を
  design で固定し、「No Results」への退行を防ぐ。
- **D-4 ImportIntent/job 状態機械**: import_id・pinned_asset・
  expected_project_revision・target・copy_policy・
  source_principal_ref と Queued→…→CommittedLive・副状態を
  製品契約として固定。preview 成功 ≠ verified import・
  CommittedLive ≠ saved HEAD。
- **D-5 evidence honesty**: Required/Deferred/NotApplicable
  と Pass/Fail/NotRun/Unsupported/NotImplemented/Unknown を
  別軸で管理し、headless/mock/fixture Pass を real 受入へ
  昇格しない。実装 case は `NotRun`、docs 検証 case のみ
  実行結果を記録。
- **D-6 impl/exp canonical 名**: `docs/impl/ir/asset-
  library.md`・`docs/impl/exp/asset-library.md`（command-
  gateway 先例一致）。scaffold の slug 名は canonical 名へ
  rename。

## Related Experiments

### Adopted

- `docs/impl/exp/asset-library.md` — 文書鎖生成・上流契約
  照合・Owner 境界固定・evidence honesty 分類の試行を採用。

### Rejected

- なし。

## Validation

- `node .agents/skills/doc-status/scripts/doc_status.js lint
  --type spec/design/plan/task/test-spec --json` — 各 type
  の lint。
- `node .agents/skills/doc-maintenance/scripts/doc_maintenance.js
  index apply` — 触れた dir の index 再生成。
- `node .agents/skills/impl-doc/scripts/audit_impl_record.js
  --json` / `audit_experiment_log.js --json` — impl audit。
- `node .agents/skills/impl-doc/scripts/sync_impl_index.js
  --kind ir/exp --apply` — impl index 同期。
- `node .agents/skills/doc-status/scripts/audit_docs.js
  --type all --json` — 全体 audit。
- `mise run fmt-check` — docs-only slice の fmt 検証。
- 詳細な実行結果は `evidence/sv9/asset-library-verification.md`
  を正本とする。

## Risks

- coverage/capability state の複合性 → typed state を単一
  model に集約（design で固定）。
- preview/verified import の混同 → 別 affordance・別 state・
  test-spec で保証。
- headless/mock Pass の real 受入への誤昇格 → evidence
  ledger で mode/layer/required を分離。
- 既存 ALIB 契約との矛盾 → relations で双方向 link・
  非重複・昇格禁止を明記。

## Follow-ups

- 実装は #85（provider/metadata-first/handle scope）・#86
  （verified import/Project CAS/v5 migration）・#87（cache/
  principal/preview/index coverage/QoS）・#88（Git/LFS/固定
  Commit/secret）・#89（Browser/Import UX・GPUI）・#90
  （WebDAV/If-Match/実 server capability/security）・#97
  （VCS snapshot 統合）・#114（実 GPUI GUI）が Owner。
- 実 Provider/実 GUI/実 OS の L2/L3 受入は #88/#90/#114/#331
  の別 gate で実施。
- LIB-* case の実行は #91 の fault matrix gate で実施。
