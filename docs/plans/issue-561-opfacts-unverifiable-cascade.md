---
id: "PLAN-561OpFactsUnverifiable04"
type: "plan"
status: "approved"
title: "Issue #561 plan — op_facts unverifiable cascade 修正 slice"
created: "2026-11-18"
updated: "2026-11-18"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/561"
    - "docs/specs/issue-561-opfacts-unverifiable-cascade.md"
    - "docs/designs/issue-561-opfacts-unverifiable-cascade.md"
  changes:
    added:
      - type: "file"
        path: "docs/discovery/issue-561-opfacts-unverifiable-cascade.md"
      - type: "file"
        path: "docs/specs/issue-561-opfacts-unverifiable-cascade.md"
      - type: "file"
        path: "docs/designs/issue-561-opfacts-unverifiable-cascade.md"
      - type: "file"
        path: "docs/plans/issue-561-opfacts-unverifiable-cascade.md"
      - type: "file"
        path: "docs/tasks/issue-561-opfacts-unverifiable-cascade.md"
      - type: "file"
        path: "docs/test-specs/issue-561-opfacts-unverifiable-cascade.md"
      - type: "file"
        path: "docs/impl/ir/issue-561-opfacts-unverifiable-cascade.md"
      - type: "file"
        path: "docs/impl/exp/issue-561-opfacts-unverifiable-cascade.md"
      - type: "file"
        path: "docs/impl/exp/issue-561-opfacts-unverifiable-cascade-exp.jsonl"
      - type: "dir"
        path: "evidence/sv561/"
    modified:
      - type: "file"
        path: "crates/synvelis-gateway/src/prepare.rs"
      - type: "file"
        path: "crates/synvelis-gateway/tests/prepare.rs"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on:
    - "docs/designs/issue-561-opfacts-unverifiable-cascade.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/tasks/issue-561-opfacts-unverifiable-cascade.md"
    - "docs/test-specs/issue-561-opfacts-unverifiable-cascade.md"
---

# Issue #561 plan — op_facts unverifiable cascade 修正 slice

## スコープ

`op_facts` の `SetSendGainTarget`・`RemoveTrack`・
`RemoveMidiSource` 3 arm に unverifiable 検査を追加し、
`RemoveSend` arm のロジックを `ControlOwner` 版ヘルパへ
semantics-preserving に抽出する。wire pin test 6 件と
domain pin test 4 件を追加し、doc chain と evidence を
整備する。

## ファイルマップ

| ファイル | 変更 |
| --- | --- |
| `crates/synvelis-gateway/src/prepare.rs` | `control_closure_unverifiable` helper 追加、`RemoveSend`/`RemoveTrack`/`SetSendGainTarget`/`RemoveMidiSource` 4 arm 更新 |
| `crates/synvelis-gateway/tests/prepare.rs` | sv561 fixture helper + pin test 6 件追加（import 拡張） |
| `crates/synvelis-project/tests/sv561_unobservable_cascade.rs` | 新規 — domain cascade 発火条件 pin 4 件 |
| `docs/{discovery,specs,designs,plans,tasks,test-specs,impl/ir,impl/exp}/issue-561-*.md` | doc chain 新規 |
| `evidence/sv561/` | reproduction.md・verification.md |

## 依存グラフ

```
discovery → spec → design → plan → task → test-spec → impl(ir/exp)
                                              ↘ 実装・検証
```

## リスクと rollback

- **`RemoveSend` 回帰**: helper 抽出は semantics-preserving
  （同一収集・判定・文言）。cb06 の sv#550/sv#553 pin が
  非回帰の証拠。rollback は helper 呼出を inline に戻すだけ。
- **過剰拒否**: positive control test 2 件で「unobservable
  entity が無ければ受理」を pin。既存受理 op への影響は
  `mise run test synvelis-gateway` 全 suite で検証。
- **mirror 乖離**: domain pin test で `ops.rs` の発火条件を
  固定。将来 domain cascade が変われば domain test が先に
  落ち、gateway mirror の更新を促す。

## verification matrix

| 検証 | コマンド | lane |
| --- | --- | --- |
| gateway check/lint/test | `mise run {check,lint,test} synvelis-gateway` | OS-L1 |
| project check/lint/test | `mise run {check,lint,test} synvelis-project` | OS-L1 |
| fmt | `mise run fmt-check` | OS-L1 |
| headless deps | `mise run check-headless-deps` | OS-L1 |
| hooks | `mise exec -- prek run --all-files` | OS-L1 |

GUI-L2・EXT-L3・MSRV・3OS 横断・Full CI は POLICIES.md §5
により日常必須検証に含めない（Daily/手動 Full CI に委譲）。
