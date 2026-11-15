---
id: "DESIGN-034XHN8wd1qFbv3bkzg7bn"
type: "design"
status: "approved"
title: "RCD-02 residual closeout — AC 照合と gate 分類・簿記整合の設計"
created: "2026-11-15"
updated: "2026-11-15"
owners: ["scarecrow173"]
relations:
  source:
    - "docs/specs/rcd-02-residual-closeout.md"
  changes:
    added:
      - type: "file"
        path: "docs/specs/rcd-02-residual-closeout.md"
      - type: "file"
        path: "docs/designs/rcd-02-residual-closeout-design.md"
      - type: "file"
        path: "docs/plans/rcd-02-residual-closeout.md"
      - type: "file"
        path: "docs/tasks/rcd-02-residual-closeout.md"
      - type: "file"
        path: "docs/test-specs/rcd-02-residual-closeout.md"
      - type: "file"
        path: "docs/impl/ir/rcd-02-residual-closeout.md"
      - type: "file"
        path: "docs/impl/exp/rcd-02-residual-closeout.md"
      - type: "file"
        path: "evidence/sv116/rcd-02-closeout-verification.md"
    modified:
      - type: "file"
        path: "docs/tasks/rcd-02-task3-stagedfile-publish-migration.md"
      - type: "file"
        path: "evidence/reports/rcd-02-durability-recovery-review-ledger.md"
    deleted: []
    renamed: []
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
  implements:
    - "docs/specs/rcd-02-residual-closeout.md"
  implemented-by: []
  depends-on: []
  blocks:
    - "docs/plans/rcd-02-residual-closeout.md"
    - "docs/tasks/rcd-02-residual-closeout.md"
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/rcd-02-durability-evidence-recovery-review.md"
    - "docs/designs/rcd-02-stagedfile-publish-migration-design.md"
    - "docs/designs/rcd-02-unsynced-loss-real-fs-probe-design.md"
    - "docs/designs/rcd-02-blob-sha256-design.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/rcd-02-residual-closeout.md"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "evidence/reports/rcd-02-durability-recovery-review-ledger.md"
    - "evidence/sv116/rcd-02-blob-sha256-ledger.md"
  defers: []
  deferred-by: []
---

# RCD-02 residual closeout — AC 照合と gate 分類・簿記整合の設計

## Context

RCD-02 のプロダクション実装は 6 系統の PR（#210 Durability
Evidence/Recovery Review/Commit Receipt/IntegrityDigest +
`tests/recovery_review.rs`、#239 verified_sha256/
MediaReferenceProbe/saved-HEAD、#275 torn-append/finalize/
AlreadyApplied e2e、#305 StagedFile/SealedFile publish 移行、
#320 unsynced-write loss fixture + Windows/NTFS real-FS probe +
`test-recording-real-fs` lane、Task4 Blob SHA-256 promotion
`rcd02_blob_digest.rs`）で main に統合済み。
残る作業は「Issue の受入条件 4 行 ↔ 実在 fixture/test/ledger
参照」の照合、canonical lane の再実測、外部 gate の
NotRun/Deferred 分類、doc status・index の整合という**簿記のみ**
である。

照合の入力は 3 系統:

1. Issue #116 本文 — 受入条件 4 行・検証方法。
2. doc chain — spec/design/plan/task/test-spec/IR/exp
   （`rcd-02-durability-evidence-recovery-review`・
   `rcd-02-stagedfile-publish-migration`・
   `rcd-02-unsynced-loss-real-fs-probe`・`rcd-02-blob-sha256`・
   `rcd-02-residual-closeout`）。
3. 実測面 — `crates/synvelis-recording/tests/`・
   `crates/synvelis-player/tests/rcd02_blob_digest.rs`・
   `crates/recording-poc/tests/real_fs_probe.rs` の実在テスト、
   `evidence/reports/rcd-02-durability-recovery-review-ledger.md`・
   `evidence/sv116/rcd-02-blob-sha256-ledger.md` の既存列挙、
   canonical mise lane。

## Scope

### In Scope

- 受入条件行ごとの証拠写像表（spec の Acceptance Criteria 節と
  IR の対応表、`evidence/sv116/rcd-02-closeout-verification.md`
  の 1:1 表）。
- canonical lane 4 本（`mise run test synvelis-recording` /
  `synvelis-player` / `synvelis-project` / `test-rt`）+ 軽量
  チェック（`fmt-check`・`prek run --all-files`）の実走と
  Pass/Fail 記録。opt-in `test-recording-real-fs` は任意再実行。
- doc status/relations/index の整合（編集許可範囲内 + 生成
  index の再生成）。
- 簿記 drift 修正: task3 status `in-progress`→`done`、
  test-specs index の dup row（managed 領域を正本
  `approved` に揃え、generator 管理外 tail の stale
  `proposed` 行は残存 finding として記録）。
- `evidence/reports/rcd-02-durability-recovery-review-ledger.md`
  への additive note（任意・既存記述は不変）。
- 外部 gate の NotRun/Deferred 分類と所有 Issue の記名。

### Out of Scope

- プロダクションコード・依存定義・workflow の変更。
- 外部 gate の実施または Pass 昇格。
- `*-msrv`・`*-full` 系 lane（Daily Full CI 委譲）。
- Issue close・PR merge・他 slug の docs・`evidence/` 他 dir。

## Components and Boundaries

- **照合器（本設計の中核）**: Issue AC 行 → 既存 test 名/ledger
  fixture/実測 lane の対応表。判定語彙は `Pass`（実測）/
  `NotRun`（lane あるが未実測または自動化不能）/
  `Unsupported`（host に能力なし）/`Deferred`（別 Issue 所有の
  残 gate）/`Fail`（実測失敗）の 5 値で、既存 2 ledger の語彙を
  踏襲する。AC 各行の root 証拠は実 test 名・既存 ledger 引用に
  限定し、新規 fixture/test は作らない。
- **canonical lane 実行者**: `mise run test <package>` /
  `mise run test-rt` を worktree で逐次実行し、exit code・
  test 数を記録する。docs+evidence only の変更のため cargo 系
  lane は doc 差分に影響されないが、「AC 照合時点の main lane
  実測」として証拠化するため省略しない（sv#86 先例どおり）。
- **簿記整合器**: task3 の status を merge 済み実態（`done`）に
  合わせ、`docs/*/README.md` 管理 index を doc-maintenance の
  `index apply` で再生成する。index は手編集しない。
- **証拠固定者**: `evidence/sv116/rcd-02-closeout-verification.md`
  に lane 結果・照合表・gate 棚卸しを記録し、IR/exp に恒久記録を
  置く。

## Data and Control Flow

1. Issue #116 本文の受入条件 4 行を抽出。
2. 各行について既存 ledger（`evidence/reports/
   rcd-02-durability-recovery-review-ledger.md`・
   `evidence/sv116/rcd-02-blob-sha256-ledger.md`）の fixture 参照と
   `crates/*/tests/` の実在テスト名を `rg` で照合（存在しない
   証拠を推測で補わない・再 derive しない）。
3. canonical lane を実走し結果を採取。
4. spec Acceptance Criteria・plan Task Breakdown・task Done When・
   IR Validation に同一の判定を転記。
5. 外部 gate 行は `NotRun`/`Deferred` + 所有 Issue（真の
   power-loss = 非自動化、Linux/macOS real-FS = 環境、実 device =
   synvelis#115、製品 GUI = synvelis#118、consumer =
   `scarecrow173/ruviform#294`、project 側 HEAD CAS = synvelis#84）
   を記名。
6. doc_status lint/audit・audit_impl_record・doc_maintenance
   index plan/apply を対象で実走し、新規 finding が無いこと・
   index が in-sync になることを確認（repo-wide baseline の
   legacy invalid-id 系 finding は既知・migration 対象として
   区別）。

## Non-Functional Constraints

- **誠実性不変条件**: NotRun/Unsupported/Deferred/Mock を Pass
  と書かない。headless fixture の PASS は headless 範囲の証拠に
  限定する。実行していない検証は NotRun と記録する。
- **占有境界**: worker spec の編集許可リスト外は一切変更しない
  （`crates/` 全体・`ui/`・`Cargo.toml`・`mise.toml`・`.github/`・
  `POLICIES.md`・`AGENTS.md`・他 slug docs・`evidence/` 他 dir・
  `evidence/sv116/` 既存 file を含む）。
- **再現性**: lane 結果には実行環境（OS・toolchain・対象
  commit）と再現コマンドを添える。
- **index 非手編集**: `docs/*/README.md` は doc-maintenance の
  `index apply` でのみ更新する。

## Risks and Trade-offs

- **照合の誤判定**: test 名の列挙だけでは受入条件の意味的充足を
  誤る可能度がある → 各行に fixture/test 名と「何を証明するか」
  を併記し、外部 gate 由来の部分は行内で明示分離する。
- **lane の環境依存 flake**: 過去に webdav flake・LNK1102 の
  観測例あり → Fail 時は単独再実行で切り分け、最終結果を正直に
  記録する。
- **AC-3 の横断性**: 「Legacy v5/Asset/Project Schema
  migration との相互参照、旧 Snapshot/HEAD 保護」は RCD-02 単体
  ではなく ALIB-02（synvelis#86）・project 側 HEAD CAS
  （synvelis#84）にまたがる → 該当部分を分離して gate 記名し、
  RCD-02 側の責務分（CRC32/SHA-256 分離・verified_sha256 記録・
  dual-scheme blob namespace）だけを Pass とする。
- **過剰な doc 量**: 簿記タスクに 7 file chain を起こすのは重い
  → ただし doc-driven-dev-graph の traceability（plan の
  verified-by→test-spec 要件等）を満たすため、各 doc は最小構成
  に留める。

## Deferred Design Concerns

- 真の power-loss・他 OS real-FS・実 device・製品 GUI・
  consumer・project 側 HEAD CAS の gate 実施 | 本環境
  （Windows headless）では原理的に不可または別 Issue 所有 |
  synvelis#115・#118・#84・`scarecrow173/ruviform#294` |
  mock PASS の実機昇格・NotRun の Pass 誤記。
- Issue #116 の close 判定 | 外部 gate が残存 | gate 所有 Issue
  の完了後 | 未証明行を閉じたままにする誤判定。
- `docs/*/README.md` 以外の repo-wide doc drift（legacy
  invalid-id 群等） | migration 対象の baseline | doc-maintenance
  `migrate_ids` 等の別 slice | 本 slice での無関係な修正。

## Verification Notes

- [x] canonical lane（`mise run test synvelis-recording` /
  `synvelis-player` / `synvelis-project` / `test-rt`）+ 軽量
  チェック（`fmt-check`・`prek run --all-files`）を `mise run`
  で実走し exit code を記録（結果は
  `docs/impl/ir/rcd-02-residual-closeout.md` Validation と
  `evidence/sv116/rcd-02-closeout-verification.md`）。
- [x] doc_status lint/audit・audit_impl_record・doc_maintenance
  index plan/apply を変更対象で実走し、新規 finding なし・index
  in-sync を確認。
- [x] spec の Acceptance Criteria 各行が実在テスト/ledger/lane に
  対応することを `rg` で照合済み。
