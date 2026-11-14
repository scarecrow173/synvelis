---
id: "IMPL-034X9JqvKCwVsxWs2xcNVz"
type: "impl"
status: "completed"
title: "VCS Integration 統合契約文書チェーン + evidence/sv7 Case 被覆台帳 (synvelis#7)"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/7"
  changes:
    added:
      - type: "file"
        path: "docs/specs/vcs-integration.md"
      - type: "file"
        path: "docs/designs/vcs-integration-design.md"
      - type: "file"
        path: "docs/plans/vcs-integration-plan.md"
      - type: "file"
        path: "docs/tasks/vcs-integration-task.md"
      - type: "file"
        path: "docs/test-specs/vcs-integration.md"
      - type: "file"
        path: "docs/impl/ir/vcs-integration.md"
      - type: "file"
        path: "docs/impl/exp/vcs-integration.md"
      - type: "file"
        path: "evidence/sv7/vcs-integration-case-ledger.md"
      - type: "file"
        path: "evidence/sv7/vcs-integration-verification.md"
    modified:
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
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/tasks/vcs-integration-task.md"
    - "docs/plans/vcs-integration-plan.md"
    - "docs/specs/vcs-integration.md"
    - "docs/designs/vcs-integration-design.md"
  implemented-by: []
  depends-on:
    - "docs/designs/vcs-integration-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/vcs-integration.md"
    - "docs/test-specs/vcs-integration.md"
    - "docs/impl/exp/vcs-integration.md"
    - "evidence/sv7/vcs-integration-case-ledger.md"
    - "evidence/sv7/vcs-integration-verification.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/plans/vcs-integration-plan.md"
  derived-by: []
  verifies:
    - "docs/test-specs/vcs-integration.md"
  verified-by: []
  references:
    - "evidence/reports/vcs-06-fixture-matrix-ledger.md"
    - "evidence/reports/vcs-06-compat-evidence.md"
    - "evidence/sv7/vcs-integration-case-ledger.md"
    - "evidence/sv7/vcs-integration-verification.md"
    - "docs/specs/vcs-01-immutable-snapshot-resolver.md"
    - "docs/specs/vcs-02-semantic-diff-changeset.md"
    - "docs/specs/vcs-03-git-read-provider.md"
    - "docs/specs/vcs-04-history-visual-diff-ui.md"
    - "docs/specs/vcs-04-backend-wiring.md"
    - "docs/specs/vcs-04-native-mount.md"
    - "docs/specs/vcs-05-apply-restore-headless.md"
    - "docs/specs/vcs-06-fixture-fault-matrix.md"
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted:
      - "docs/impl/exp/vcs-integration.md"
    rejected: []
---

# VCS Integration 統合契約文書チェーン + evidence/sv7 Case 被覆台帳 (synvelis#7)

## Summary

Issue #7（VCS Integration / Semantic Visual Diff）の統合契約を
docs-only スライスとして canonical 化した。`docs/` に
spec/design/plan/task/test-spec/impl-ir/impl-exp の 7 文書を作成し、
`evidence/sv7/` に Case 被覆台帳と検証記録を作成した。
コード・CI・本番 crate・既存 slice spec・既存 evidence ledger は
一切変更していない。実装 Owner（#92〜96）と統合 Gate（#97）の
責務境界を維持し、既存 vcs-01..06 spec を正本として参照する
統合契約のみを追加した。

## Context

Implements `docs/tasks/vcs-integration-task.md` per
`docs/specs/vcs-integration.md` and
`docs/designs/vcs-integration-design.md`, planned by
`docs/plans/vcs-integration-plan.md`, verified by
`docs/test-specs/vcs-integration.md`（synvelis#7 の統合契約
スライス。実装は #92〜96・#97 Owner）。

前提: VCS-01（#92）〜VCS-05（#96）の slice spec と VCS-06（#97）の
fixture ledger が main に統合済み。sv#95（VCS History native
mount/dock、PR #384）も統合済み。本スライスはそれらを束ねる
親 Issue #7 の契約文書と Case 被覆台帳のみを担当する。

## Implementation

### Changes

- `docs/specs/vcs-integration.md` — 統合契約 spec。What/Why/
  Users/Scope/Requirements/14 受入条件/Deferred/Sources。
- `docs/designs/vcs-integration-design.md` — 層構成図・slice 責務
  対応表・ChangeSet 共有不変条件・Data/Control Flow・Non-Functional
  Constraints・Risks・Deferred・8 view 採否判定・Creative Branch
  分離記録・sv#95 整合確認。
- `docs/plans/vcs-integration-plan.md` — File Map・実装順序・
  Verification Matrix・Review Handoff。
- `docs/tasks/vcs-integration-task.md` — 実行単位（Done When・
  Verification コマンド列）。
- `docs/test-specs/vcs-integration.md` — 文書監査向け Rules/
  Examples/Guarantees/Non-goals/Risk。
- `evidence/sv7/vcs-integration-case-ledger.md` — Issue #7 受入条件
  → Case family 写像、責務 Owner 写像、mode/layer/fixture 凡例、
  全 Case ID（基底 + 拡張行）の被覆表、明示的非 Pass 領域、
  verdict 分布集計。
- `evidence/sv7/vcs-integration-verification.md` — 実行した検証の
  記録（コマンド・OS・toolchain・exit code・結果）。
- `docs/{specs,designs,plans,tasks,test-specs}/README.md` —
  `doc_maintenance.js index apply` による再生成。
- `docs/impl/{ir,exp}/README.md` — `sync_impl_index.js --apply`
  による再生成。

### Decisions During Implementation

- **文書 ID は `new_*` script 採番**: spec `SPEC-034X9CW66GeytQXJHrogRB`、
  design `DESIGN-034X9E4j59hipUI46lNAQC`、plan
  `PLAN-034X9Fn5NTJQhX4cdeEEoq`、task `TASK-034X9GCGPXdCl5ePe8gJ14`、
  test-spec `TSPEC-034X9G0UGtaLhkU5NlUIjh`、impl
  `IMPL-034X9JqvKCwVsxWs2xcNVz`。opaque ID 規約に従う。
- **status**: spec/design/test-spec は `approved`（Issue #7 が
  承認済み契約であり review-protocol が承認ゲートを代替）、
  plan は `in-progress`、task/impl-ir は作業完了後に `done`/
  `completed` へ更新。
- **evidence ファイル名**: sv44/sv45 先例に倣い
  `vcs-integration-case-ledger.md` / `vcs-integration-verification.md`
  とした（`case-ledger.md`/`verification.md` ではなく prefix 付き）。
- **impl-exp は `.md` 形式**: `new_experiment_log.js` が生成する
  `.jsonl` event log ではなく、`vcs-04-native-mount.md` 等の
  markdown experiment log 先例に倣い `docs/impl/exp/
  vcs-integration.md` を採用（task 指定パスが `.md` のため）。
- **verdict は転記のみ**: ledger の `Pass`/`Pass*`/`NotRun`/
  `Unsupported`/`Blocked` を `vcs-06-fixture-matrix-ledger.md` から
  そのまま転記し、昇格・再評価を行わない。
- **8 view 採否は契約レベル記録**: mockup HTML 本体が repo 未収録の
  ため、Issue 本文の画面構成 + 既存 view mode 決定を根拠に採否を
  記録し、pixel レビュー未実施を Deferred として明示。

## Related Experiments

### Adopted

- `docs/impl/exp/vcs-integration.md` — 本スライスの判断ログ
  （命名規約・status 方針・verdict 転記方針・8 view 採否の根拠）。

### Rejected

- なし（設計段階の代替案は design doc §Risks/Deferred に記録）。

## Validation

canonical = mise、OS: Windows、toolchain: mise pin。実測値は
`evidence/sv7/vcs-integration-verification.md` に記録:

- `mise run fmt-check` — exit 0
- `doc_maintenance.js index plan|apply`（7 dir）— 全て適用、
  planHash 一致、index 手編集なし
- `sync_impl_index.js --kind ir|exp` — IN-SYNC
- `doc_status.js lint`（spec/design/plan/task/test-spec）—
  新規文書の finding 0（repo 全体の pre-existing finding は
  範囲外として記録）
- `audit_impl_record.js` / `audit_experiment_log.js` —
  本 slice 起因の error なし（既存 `.jsonl` の pre-existing
  error は範囲外）
- `route_graph.js --focus docs/specs/vcs-integration.md` —
  `blocked`（`artifact-graph`、pre-existing repo 状態）。
  taskGraph は本 task を `active` として解決
- `review-protocol` — blocker なし（集計誤り・relation 修正済み）

## Risks

- doc graph の pre-existing broken relations（legacy `ISSUE-*`/
  `SPEC-*` token）が route/audit を阻害する可能性 — 本スライス起因の
  新規 broken relation のみ修正対象とし、既存分は記録に留める。
- ledger verdict の転記ミス — `vcs-06-fixture-matrix-ledger.md`
  との突合で確認済み（case-ledger は同 ledger の行を基に作成）。

## Follow-ups

- 実 GUI/IME/2Window/pixel/plugin dock 受入 — Issue #114 gate。
- 実 remote/auth/GC・他 provider・OS matrix・consumer precision —
  #97 REAL lanes / #219/#294。
- mockup HTML のリポジトリ収録と pixel レビュー — Issue #7 が
  別作業と明記。
- Clip Detail 独立 mount・Plugin State view・Rendered Audio Diff —
  後続 slice。
- `plugin_instances` の diff 可視化 — #324 系。
- Issue #7 本体の統合受入判定 — #97 Owner（本スライスは証拠を
  提供するのみで Issue を Close しない）。
