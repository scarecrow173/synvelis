---
id: "PLAN-4eUlpRzzgdpGu0FbUVLgtc"
type: "plan"
status: "approved"
title: "Target Architecture 設計レビュー実施計画 — docs chain + evidence/sv22 台帳の構成"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "docs/specs/target-architecture.md"
    - "docs/designs/target-architecture-design.md"
  changes:
    added:
      - type: "file"
        path: "docs/plans/target-architecture.md"
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/target-architecture.md"
  implemented-by: []
  depends-on:
    - "docs/designs/target-architecture-design.md"
    - "docs/specs/target-architecture.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/tasks/target-architecture-task.md"
    - "docs/test-specs/target-architecture.md"
    - "docs/impl/ir/target-architecture.md"
    - "docs/impl/exp/target-architecture.md"
    - "evidence/sv22/case-ledger.md"
    - "evidence/sv22/dependency-dag.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/target-architecture-design.md"
    - "docs/specs/target-architecture.md"
  derived-by:
    - "docs/tasks/target-architecture-task.md"
  verifies: []
  verified-by:
    - "docs/test-specs/target-architecture.md"
  references:
    - "evidence/sv22/case-ledger.md"
    - "evidence/sv22/dependency-dag.md"
  defers: []
  deferred-by: []
---

# Target Architecture 設計レビュー実施計画

## 目的

spec/design が固定した TA-001〜012 + GUI 方針 2 条件の設計
レビューを、docs chain 7 文書と `evidence/sv22/` 台帳として
完結させる。**コード・manifest・lockfile・CI は変更しない**
docs/evidence-only slice。

## スコープ

### In Scope

- `docs/specs|designs|plans|tasks|test-specs/target-architecture.md`
- `docs/impl/ir|exp/target-architecture.md`
- `evidence/sv22/case-ledger.md`・`evidence/sv22/dependency-dag.md`
- 対象 dir の docs index 再生成（`doc-maintenance`）
- canonical 検証: `mise run fmt-check`・`mise run
  check-ruviform-rev`・`mise run check-headless-deps`
- `cargo metadata --locked` による DAG cycle 検査（補助証拠）

### Out of Scope

- `Cargo.toml`/`Cargo.lock`/`mise.toml`/CI/本番 crate
- Ruviform 共通基盤の実装（別 repo・読み取り参照のみ）
- AudioLens 製品コード・受入
- 旧 WebView ADR の再承認・GPUI/実 Plugin OS Gate 完了宣言
- sv#20/#21 の docs ファイル（並行 Worker 占有・参照のみ）
- pre-existing broken relations の修復（route graph blocked）

## 実施ステップ

| # | ステップ | 成果物 | 検証 |
| --- | --- | --- | --- |
| 1 | v0.7.1・関連 spec/design/ADR・Issue 群の収集 | 調査メモ（コンテキスト） | — |
| 2 | Cargo resolve グラフ取得・cycle 検査・辺分類 | `evidence/sv22/dependency-dag.md` | cycle = 0（通常依存） |
| 3 | Ruviform resolved rev の型漏出走査・fixture 存在 pin | case-ledger C02〜C12 | grep/ファイル存在 |
| 4 | ADR/feature guard の GUI 方針照合 | case-ledger C13/C14 | status 照合 + check-headless-deps |
| 5 | canonical 検証実行 | case-ledger Environment | fmt-check・check-ruviform-rev・check-headless-deps |
| 6 | docs chain 7 文書執筆 | specs/designs/plans/tasks/test-specs/impl/{ir,exp} | front matter relation 実在 |
| 7 | index 再生成 + lint | docs/*/README.md | doc_status.js lint |
| 8 | review-protocol（客観・独立・敵対・検証・severity） | review 記録 | blocker なし |
| 9 | merge origin/main → index 再生成 → commit → push → PR | PR | Conventional Commits |

## リスクと対処

| リスク | 対処 |
| --- | --- |
| fixture を「実走済み」と誤記 | case-ledger で Pass=実行/参照・NotRun=未実行を分離。ソース pin は「契約実在」のみを意味すると明記 |
| v0.7.1 と現行コードの差異を blocker 扱い | P2/P3 観測として分離（crate 数ドリフト・PPQN 範囲差・theme fixture） |
| GPUI 最終採用を誤って PASS 宣言 | #384 が proposed・#262/#388/#389 が実機 Gate であることを台帳に固定し Deferred 記録 |
| pre-existing broken relations に引きずられる | route graph の blocked 状態を範囲外として記録のみ。修復しない |
| relation target の不存在で lint 失敗 | 追加した全 relation を `test -f` で検証してから commit |

## 検証マトリクス

| 検証 | コマンド | 期待 |
| --- | --- | --- |
| fmt | `mise run fmt-check` | exit 0 |
| rev 一致 | `mise run check-ruviform-rev` | exit 0（fd1d4d5 で lock/vendor/manifest 一致） |
| headless guard | `mise run check-headless-deps` | exit 0 |
| DAG cycle | `cargo metadata` + DFS（dependency-dag.md 記載コマンド） | 通常依存 cycle = 0 |
| docs lint | `doc_status.js lint`（触った dir） | 新規 error なし |
| index 整合 | `doc_maintenance.js index apply` + 確認 | 生成 index が最新 |

`mise run check`/`test`/`test-rt`/`lint` は package 引数必須で
あり、本 docs/evidence-only slice では対象 crate 変更がないため
**実行しない**（Issue 転記の「照合記録に留める」に従い、
case-ledger に NotRun 根拠を記録する）。
