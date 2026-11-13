---
id: "PLAN-uztPCmkgSUh3hCroP9JswU"
type: "plan"
status: "approved"
title: "SF-04 probe adapter — Issue #107 SF-PROBE-01..08 受入集約 実装計画"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "docs/designs/sf-04-probe-adapter-design.md"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/sf-04-probe-adapter.md"
    - "docs/designs/sf-04-probe-adapter-design.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/sf-04-probe-adapter.md"
    - "docs/designs/sf-04-probe-adapter-design.md"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "docs/specs/sf-04-probe-adapter.md"
    - "docs/designs/sf-04-probe-adapter-design.md"
  defers: []
  deferred-by: []
---

# SF-04 probe adapter — Issue #107 SF-PROBE-01..08 受入集約 実装計画

## スコープ分割

単一スライス。本タスクは受入集約であり、コード差分は新規 headless
テスト 1 ファイルのみ。`synvelis-project`/`synvelis-player` の src
変更は監査で contract 違反が見つかった場合の最小修正に限定する
（現時点の監査では違反なし）。

## ファイルマップ

| ファイル | 変更 |
| --- | --- |
| `crates/synvelis-player/tests/sf04_probe_acceptance.rs` | 新規。entity 消失 unbind・再 attach fresh bind・per-key 冪等 dedup・session 境界の 2 テスト |
| `docs/{specs,designs,plans,tasks,test-specs}/sf-04-probe-adapter*.md` | 本チェーン |
| `docs/impl/ir/sf-04-probe-adapter.md`・`docs/impl/exp/sf-04-probe-adapter.md` | 実装記録・実験記録 |
| `evidence/sv107/coverage.md`・`verification.md`・`external-gates.md` | 被覆表・検証結果・外部 Gate 判定 |
| `docs/{specs,designs,plans,tasks,test-specs}/README.md`・`docs/impl/README.md`（該当 index） | doc-maintenance で再生成 |

## 依存グラフ

spec → design → plan → task → test-spec → impl(ir/exp)。実装は
既存 slice1..10 のコードに依存し、新規コード依存は追加しない。

## リスク登録

| リスク | 影響 | 対応 |
| --- | --- | --- |
| entity 消失テストが `Unresolved` ではなく別 state を返す | contract 違反の可能性 | `on_publish`→`resolve_probe_point` の返却を確認し、違反なら所有境界で最小修正。src 変更が必要なら Coordinator に範囲確認 |
| `check-ruviform-rev` の vendor REV 不一致（lock `fd1d4d5f` vs vendor `20a24a66`） | 検証コマンド失敗 | 本タスクの編集範囲外（`ui/vendor`・`Cargo.lock`）。`mise run sync-ui-deps` は生成物更新のため範囲外変更に該当 — 実行可否を確認し、不可なら evidence/PR に NotRun として記録 |
| per-key dedup テストが session 層 lease と混同される | 設計誤認 | player 層は key 単位 dedup のみを主張し、cross-window 集約は slice5 の契約と明記 |

## ロールバック

新規テストファイルと文書・evidence のみの追加で、既存コードへの
変更がないため revert はコミット単位で安全。src 修正が発生した
場合はそのコミットを個別に revert 可能なよう論理単位で分割する。

## 検証マトリクス

| 検証 | コマンド | 対象 |
| --- | --- | --- |
| fmt | `mise run fmt-check` | workspace |
| check | `mise run check synvelis-project` / `mise run check synvelis-player` | 変更 crate + 依存 |
| lint | `mise run lint synvelis-project` / `mise run lint synvelis-player` | 同上 |
| test | `mise run test synvelis-project` / `mise run test synvelis-player` | headless のみ |
| test-rt | `mise run test-rt`（該当タスクがあれば package 指定） | RT fixture |
| ruviform rev | `mise run check-ruviform-rev` | lock↔vendor 整合 |

統合受入は merge 後の対象 SHA の必須 CI 証拠（Coordinator 確認）。
ローカル検証は変更範囲（project/player headless）に比例させる。
