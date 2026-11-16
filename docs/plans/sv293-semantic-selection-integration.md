---
id: "PLAN-6ikEcrQXHM0iMHiCWLDIsj"
type: "plan"
status: "approved"
title: "sv293 semantic→selection integration 実装計画 (synvelis#293 deferred residual slice)"
created: "2026-11-16"
updated: "2026-11-16"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/293"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/designs/sv293-semantic-selection-integration.md"
  implemented-by: []
  depends-on:
    - "docs/designs/sv293-semantic-selection-integration.md"
    - "docs/specs/sv293-semantic-selection-integration.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/tasks/sv293-semantic-selection-integration.md"
    - "docs/test-specs/sv293-semantic-selection-integration.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/sv293-semantic-selection-integration.md"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "docs/tasks/sv293-semantic-selection-integration.md"
  defers:
    - "https://github.com/scarecrow173/synvelis/issues/296"
  deferred-by: []
---

# sv293 semantic→selection integration 実装計画

## スコープ分割

単一 slice。変更は `synvelis-workspace` 内に閉じる。

## ファイルマップ

| ファイル | 変更 |
| --- | --- |
| `crates/synvelis-workspace/Cargo.toml` | `ruviform-graph`/`ruviform-audio-core` 追加（承認済み） |
| `Cargo.lock` | dep 追加に伴う最小差分 |
| `crates/synvelis-workspace/src/lib.rs` | `pub mod rcp_selection` + re-export |
| `crates/synvelis-workspace/src/rcp_selection.rs` | 新規 producer + drive 配線 |
| `crates/synvelis-workspace/tests/rcp_selection_integration.rs` | 新規 oracle（公開 API のみ） |
| `docs/{specs,designs,plans,tasks,test-specs,discovery,impl/ir,impl/exp}/sv293-semantic-selection-integration.md` | doc chain |
| `docs/impl/exp/sv293-semantic-selection-integration-exp.jsonl` | experiment log |
| `evidence/sv293/acceptance-ledger.md` | 受入台帳（新規） |
| `evidence/sv293/verification.md` | 検証記録（新規） |

## 依存グラフ

```
docs/discovery → docs/specs → docs/designs → docs/plans → docs/tasks
                                     └→ docs/test-specs → 実装 → docs/impl/{ir,exp}
実装 → evidence/sv293/{acceptance-ledger,verification}
```

## タスク分解

1. producer module 実装（snapshot/capability/render index/
   select 呼出し）。
2. `drive_selection_plan`（reconciler → stale gate → engine
   prepare/publish → reconciler publish）。
3. oracle test（Intent 経路・fail-closed・stale）。
4. doc chain + evidence。
5. review-protocol → merge origin/main → 再検証 → commit/push/PR。

## リスクと対策

| リスク | 対策 |
| --- | --- |
| semantic port/edge 写像の誤り | oracle で edge kind・chain 順・sink を pin |
| eligibility 誤昇格 | `CandidateReport` をそのまま写し、gate は select 側契約に委譲 |
| stale selection の engine 到達 | `selection_pin_fault` を prepare 前に適用 + oracle で pin |
| host instance consume による連続 drive 失敗 | test 側で drive 毎に host 再武装（capability_generation=0 で drift なし） |
| `RoutingCompileRequest` private 経路 | `synvelis_project` re-export を使用（compile で確認済み） |

## 検証マトリクス

| 検証 | コマンド | 対象 |
| --- | --- | --- |
| format | `mise run fmt-check` | workspace |
| check | `mise run check synvelis-workspace` | 変更 crate |
| lint | `mise run lint synvelis-workspace` | 変更 crate |
| test | `mise run test synvelis-workspace` | 変更 crate（新規 test target 含む） |
| 非回帰 | `mise run check synvelis-app` / `check synvelis-render` | 下流 consumer |
| hooks | `mise exec -- prek run --all-files` | 全ファイル |

NotRun: `*-full`/`*-msrv`/3OS/GUI lane/実 Plugin ABI/実 signal clock/
仮想 OS endpoint/実 Window runner/実 OS GUI/実行・製品 gate —
POLICIES.md と task spec の指定どおり Daily/手動 CI・別 gate へ委譲。
