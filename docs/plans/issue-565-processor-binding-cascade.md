---
id: "PLAN-565ProcessorBindingCascade04"
type: "plan"
status: "approved"
title: "Issue #565 plan — 退役 cascade 監査 slice"
created: "2026-11-18"
updated: "2026-11-18"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/565"
    - "docs/specs/issue-565-processor-binding-cascade.md"
    - "docs/designs/issue-565-processor-binding-cascade.md"
  changes:
    added:
      - type: "file"
        path: "docs/discovery/issue-565-processor-binding-cascade.md"
      - type: "file"
        path: "docs/specs/issue-565-processor-binding-cascade.md"
      - type: "file"
        path: "docs/designs/issue-565-processor-binding-cascade.md"
      - type: "file"
        path: "docs/plans/issue-565-processor-binding-cascade.md"
      - type: "file"
        path: "docs/tasks/issue-565-processor-binding-cascade.md"
      - type: "file"
        path: "docs/test-specs/issue-565-processor-binding-cascade.md"
      - type: "file"
        path: "docs/impl/ir/issue-565-processor-binding-cascade.md"
      - type: "file"
        path: "docs/impl/exp/issue-565-processor-binding-cascade.md"
      - type: "file"
        path: "docs/impl/exp/issue-565-processor-binding-cascade-exp.jsonl"
      - type: "dir"
        path: "evidence/sv565/"
    modified:
      - type: "file"
        path: "crates/synvelis-project/src/ops.rs"
      - type: "file"
        path: "crates/synvelis-gateway/src/prepare.rs"
      - type: "file"
        path: "crates/synvelis-gateway/src/prepare_engine.rs"
      - type: "file"
        path: "crates/synvelis-project/tests/plugin_instance_persist.rs"
      - type: "file"
        path: "crates/synvelis-gateway/tests/prepare.rs"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on:
    - "docs/designs/issue-565-processor-binding-cascade.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/tasks/issue-565-processor-binding-cascade.md"
    - "docs/test-specs/issue-565-processor-binding-cascade.md"
---

# Issue #565 plan — 退役 cascade 監査 slice

## スコープ

`SetProcessorBinding` → `plugin_instances.sync_processor` の
PluginInstanceId 退役経路を監査し、不変条件 3 軸
（再読み込み・復元・外部参照）の照合結果を doc chain と
`evidence/sv565/` に記録する。実装変更は契約明記
（コメント 3 箇所）と test pin（project 3 件・gateway
1-2 件）に限定し、`unsupported` 追加や store cascade の
動作変更は行わない。

## ファイルマップ

| ファイル | 変更 |
| --- | --- |
| `crates/synvelis-project/src/ops.rs` | `SetProcessorBinding` op 定義コメントに退役 id 非再発行を補強（コメントのみ） |
| `crates/synvelis-gateway/src/prepare.rs` | `SetProcessorBinding` op_facts arm に internal-structure posture コメント明記 |
| `crates/synvelis-gateway/src/prepare_engine.rs` | `synvelis.processor.set-binding` entry に同 posture コメント明記 |
| `crates/synvelis-project/tests/plugin_instance_persist.rs` | op 経路 pin 3 件追加（rebind 退役・同一 plugin 冪等・Unresolved 退役） |
| `crates/synvelis-gateway/tests/prepare.rs` | wire pin 追加（Write detail の binding 差分 + candidate store 同期） |
| `docs/{discovery,specs,designs,plans,tasks,test-specs,impl/ir,impl/exp}/issue-565-*.md` | doc chain 新規 |
| `evidence/sv565/` | audit.md・verification.md |

## 依存グラフ

```
discovery → spec → design → plan → task → test-spec → impl(ir/exp)
                                              ↘ 実装・検証
```

## リスクと rollback

- **コメントのみの production 変更**: 動作差分ゼロのため
  rollback はコメント削除のみ。test pin は追加のみで
  既存 pin を変更しない。
- **過剰拒否の反証**: wire pin が `Completed` + proposal
  発行を assert するため、`unsupported` を誤って立てる
  変更が混入すれば即座に検出される。
- **sv#559 干渉**: 占有パスは読み取りのみ。監査で占有
  ファイルへの修正が必要と確定した場合は `ask` で
  escalate（現時点で不要と判断）。

## verification matrix

| 検証 | コマンド | lane |
| --- | --- | --- |
| project check/lint/test | `mise run {check,lint,test} synvelis-project` | OS-L1 |
| gateway check/lint/test | `mise run {check,lint,test} synvelis-gateway` | OS-L1 |
| fmt | `mise run fmt-check` | OS-L1 |
| headless deps | `mise run check-headless-deps` | OS-L1 |
| hooks | `mise exec -- prek run --all-files` | OS-L1 |

GUI-L2・EXT-L3・MSRV・3OS 横断・Full CI は POLICIES.md §5
により日常必須検証に含めない（Daily/手動 Full CI に委譲）。
