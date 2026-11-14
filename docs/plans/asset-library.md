---
id: "PLAN-034X9CqLejWbVm8CI8jjiF"
type: "plan"
status: "approved"
title: "Asset Library 文書鎖・受入写像 実施計画 (synvelis#9)"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/9"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/asset-library.md"
  implemented-by: []
  depends-on:
    - "docs/designs/asset-library-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/alib-01-asset-provider-contract-design.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/asset-library.md"
    - "docs/designs/asset-library-design.md"
  derived-by:
    - "docs/tasks/asset-library-task.md"
  verifies: []
  verified-by:
    - "docs/test-specs/asset-library.md"
  references:
    - "evidence/sv9/asset-library-case-ledger.md"
    - "evidence/sv9/asset-library-verification.md"
  defers: []
  deferred-by: []
---

# Asset Library — 文書鎖・受入写像 実施計画 (synvelis#9)

## Goal

Issue #9 の Asset Library 要件（metadata-first remote browse・
pinned intent・verified import・offline/permission UX・失敗
Case）を canonical 文書鎖 7 文書と evidence 台帳として確立し、
既存 ALIB-01〜07 契約・#89 UX・#91 fault matrix・#88/#90/#331
provider gate・#97 VCS snapshot・#114 実 GUI・Ruviform 共通契約
との境界を固定する。本 slice は docs + evidence のみ。

## Scope

### In Scope（本 slice）

- `docs/specs/asset-library.md` — 製品要件・受入カテゴリ・
  AL-R-01〜17・AC-1〜16。
- `docs/designs/asset-library-design.md` — Owner 分離・
  Key Contracts・状態機械・failure taxonomy・UX 行動仕様。
- `docs/plans/asset-library.md` — 本計画。
- `docs/tasks/asset-library-task.md` — 文書鎖タスク。
- `docs/test-specs/asset-library.md` — LIB-* 保証仕様。
- `docs/impl/ir/asset-library.md`・`docs/impl/exp/asset-library.md`
  — 実装記録・試行記録。
- `evidence/sv9/asset-library-case-ledger.md`・
  `evidence/sv9/asset-library-verification.md` — case 台帳・
  検証記録。
- 触れた dir の docs index を `doc-maintenance` で再生成。

### Out of Scope

- Provider Adapter・cache/importer・Project Controller・UI・
  test fixture の実装（#85〜#91 Owner）。
- Ruviform Rooted File/IO/Security 共通契約の変更。
- 実機/実 network/実 OS/実 GUI の受入実行（Deferred gate）。
- Cargo.toml/Cargo.lock/mise.toml/CI/本番 crate。

## Upstream

- `docs/specs/asset-library.md`（approved）— 本計画が実装する
  製品契約。
- `docs/designs/asset-library-design.md`（approved）— Owner
  分離・状態機械・failure taxonomy の設計正本。
- `docs/test-specs/asset-library.md`（approved）— 本計画の
  検証保証。
- `docs/specs/alib-01-asset-provider-contract.md`・
  `docs/specs/alib-02-streaming-verified-import.md`・
  `docs/specs/alib-03-catalog-preview-cache.md`・
  `docs/specs/alib-04-git-remote-provider.md`・
  `docs/specs/alib-06-webdav-provider.md`・
  `docs/specs/alib-07-fault-matrix.md` — 既存 provider/
  import/cache/fault 契約（参照・非重複）。
- `https://github.com/scarecrow173/synvelis/issues/9` —
  正本 Issue。#89・#91・#85〜#90・#97・#114・#331・#84・
  Ruviform #219/#294 は関連 Owner。

## File Map

| Path | Responsibility | Notes |
|------|----------------|-------|
| `docs/specs/asset-library.md` | 製品要件・受入カテゴリ・AL-R/AC | 本 slice の正本 spec |
| `docs/designs/asset-library-design.md` | Owner 分離・契約・状態機械・failure taxonomy・UX | design gate |
| `docs/plans/asset-library.md` | 実施計画・file map・risk・verification | 本書 |
| `docs/tasks/asset-library-task.md` | 文書鎖タスク・done when・verification | task |
| `docs/test-specs/asset-library.md` | LIB-* 保証仕様・case map | test-spec |
| `docs/impl/ir/asset-library.md` | 実装記録 | impl IR |
| `docs/impl/exp/asset-library.md` | 試行記録 | impl exp |
| `evidence/sv9/asset-library-case-ledger.md` | LIB-* case → fixture/oracle/Owner/evidence 台帳 | evidence |
| `evidence/sv9/asset-library-verification.md` | 実行した検証・OS/toolchain・exit code・結果 | evidence |
| `docs/{specs,designs,plans,tasks,test-specs,impl/ir,impl/exp}/README.md` | 管理 index | `doc-maintenance index apply` で再生成・手編集禁止 |

## Implementation Streams（Owner 別）

### Stream A: 文書鎖生成（本 slice・単一 Owner）

spec → design → plan → test-spec → task → impl-ir/exp の順に
作成し、relations（implements/derives-from/verifies/
verified-by/depends-on/related/references/defers）を双方向に
固定する。

### Stream B: 受入写像・evidence 台帳

Issue #9 の 12 受入条件を LIB-ID/BROWSE/PIN/IMPORT/MIG/
CACHE/SEC/QOS case class へ写像し、`evidence/sv9/` に
fixture/oracle/Owner/mode/layer/expected-observed/status/
diagnostic/link の台帳を作成する。実装 case は `NotRun`、
docs 検証 case は実行後に `Pass`/`Fail` を記録する。

### Stream C: index 再生成・audit・検証

`doc-maintenance index apply` で触れた dir の index を再生成
し、`doc_status lint`/`audit_docs`/`audit_impl_record`/
`audit_experiment_log`/`sync_impl_index` で整合を確認。
`mise run fmt-check` を実行する。

## Dependency Graph

| Step | Depends on | Blocks |
|------|------------|--------|
| A1 spec | — | A2 design, B ledger |
| A2 design | A1 | A3 plan, A5 test-spec |
| A3 plan | A2 | A4 task |
| A4 task | A3 | A6 ir/exp |
| A5 test-spec | A1+A2 | A4 verified-by, B ledger |
| A6 ir/exp | A4+A5 | C audit |
| B ledger | A1+A5 | C audit |
| C index/audit/verify | A6+B | commit/PR |

Critical path: A1→A2→A3→A4→A6→C。B は A1/A5 後に並行可能。

## Risk Register

| # | Risk | Likelihood | Impact | Mitigation |
|---|------|-----------|--------|------------|
| 1 | coverage/capability state の複合性が UI を複雑化 | med | med | typed state を単一 model に集約、icon/label 写像を一箇所に |
| 2 | preview と verified import の混同 | med | high | 別 affordance・別 state・test-spec で保証・evidence 分離 |
| 3 | headless/mock Pass の real 受入への誤昇格 | med | high | ledger で mode/layer/required 分離、#91 gate のみ real 判定 |
| 4 | ImportId replay の digest 設計ミス | low | high | intent 全 field の canonical digest を idempotency key に |
| 5 | 既存 ALIB 契約との矛盾・重複 | med | med | spec/design で参照・非重複・昇格禁止を明記、relations で双方向 link |
| 6 | docs index 競合（他 worker） | med | low | `doc-maintenance index apply` で再生成、merge 時は index のみ再生成で解消 |
| 7 | data-loss / breaking-change / 外部依存 | low | low | docs-only・不可逆操作なし・外部依存は参照のみ |

## Verification Plan

| Step | Verification command or criteria | Pass condition |
|------|----------------------------------|----------------|
| 文書鎖 | `node .agents/skills/doc-status/scripts/doc_status.js lint --type spec --json` 他 type | exit 0・error なし |
| index | `node .agents/skills/doc-maintenance/scripts/doc_maintenance.js index apply` | 触れた dir が in-sync・blocked なし |
| impl audit | `node .agents/skills/impl-doc/scripts/audit_impl_record.js --json` / `audit_experiment_log.js --json` | error なし |
| impl index | `node .agents/skills/impl-doc/scripts/sync_impl_index.js --kind ir --apply` / `--kind exp --apply` | in-sync |
| 全体 audit | `node .agents/skills/doc-status/scripts/audit_docs.js --type all --json` | 新規 error なし（既存 warning は対象外） |
| fmt | `mise run fmt-check` | exit 0 |
| 内容 | spec AC-1〜16 が LIB-* case・Owner・evidence に写像 | ledger で全 case が fixture/oracle/Owner/status を持つ |

## Rollback

- 本 slice は docs + evidence のみで不可逆操作なし。問題が
  あれば該当 commit を revert し、index を再生成する。
- Trigger: review/CI で blocker が残る、または relations/
  index が修復不能に破損。
- Procedure: `git revert <commit>` → `doc-maintenance index
  apply` → `doc_status lint` で確認。
- Verification: `doc_status lint --type all` が新規 error
  なし、`git status` が clean。

## Task Breakdown

1. spec/design/plan/test-spec/task/ir/exp の 7 文書を作成し
   relations を双方向に固定。
2. `evidence/sv9/` に case ledger・verification を作成。
3. index を `doc-maintenance index apply` で再生成。
4. audit/lint/fmt を実行し blocker を解消。
5. review-protocol で self-review、blocker なしを確認。
6. Conventional Commit で論理単位に分けて commit・push・PR。
