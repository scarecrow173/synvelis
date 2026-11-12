---
id: "PLAN-034WxmzaM84mm5775ZdoeD"
type: "plan"
status: "approved"
title: "PC-C6 TR02/TR06 realpath oracle 実装計画"
created: "2026-11-12"
updated: "2026-11-12"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/103"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/pc-c6-tr02-tr06-realpath-oracle.md"
  implemented-by: []
  depends-on:
    - "docs/specs/pc-c6-tr02-tr06-realpath-oracle.md"
    - "docs/adr/pc-c6-tr02-tr06-realpath-oracle.md"
    - "docs/designs/pc-c6-tr02-tr06-realpath-oracle-design.md"
    - "docs/plans/pc-c6-tr-ledger-oracle-plan.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/tasks/pc-c6-tr02-tr06-realpath-oracle-task.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/pc-c6-tr02-tr06-realpath-oracle.md"
    - "docs/designs/pc-c6-tr02-tr06-realpath-oracle-design.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/pc-c6-tr02-tr06-realpath-oracle.md"
  references: []
  defers: []
  deferred-by: []
---

# PC-C6 TR02/TR06 realpath oracle 実装計画 Implementation Plan

## Goal

既存 `crates/synvelis-player/tests/pc_c6_oracle_tr_dispatch.rs` に
TR02/TR06 の dispatch-ledger 実経路 oracle test を追加し、project
層で証拠済みの残件 2 行を実経路で再証拠化する。`src/`・他 crate・
Cargo 設定・他 TR 行は変更しない。

## Scope

単一 implementation stream。直前 slice の rig・fixture・独立
oracle をそのまま再利用し、test 2 本と doc comment 更新のみを
追加する。spec の Out of Scope と同一。

## Upstream Documents

- spec: `docs/specs/pc-c6-tr02-tr06-realpath-oracle.md`
- adr: `docs/adr/pc-c6-tr02-tr06-realpath-oracle.md`
- design: `docs/designs/pc-c6-tr02-tr06-realpath-oracle-design.md`
- test-spec: `docs/test-specs/pc-c6-tr02-tr06-realpath-oracle.md`

## Design Inputs

- 既存実経路 rig（`host_plugin`/`ledger_rig`/`fabricated_adapter`/
  `observe_cycle` span）と独立 oracle（`oracle_*`/
  `assert_trace_agrees`）— design doc が正本。
- 先例 test: 同 file の `tr03_context_change_stale_real_ledger`
  （live evidence・scenario table・`assert_trace_agrees`）と
  `tr04_plugin_unavailable_vs_probe_states`（channel state 別分類）。
- 行契約の一次情報: `docs/test-specs/pc-c6-e2e-oracle-rel-tr.md`
  Rule(TR02)/Rule(TR06)。

## File Map

| file | 変更 |
| --- | --- |
| `crates/synvelis-player/tests/pc_c6_oracle_tr_dispatch.rs` | TR02/TR06 test 2 本追加 + 冒頭 doc comment 更新 |
| `docs/specs|adr|designs|plans|tasks|test-specs/pc-c6-tr02*` | 本 doc-chain（7 file） |
| `docs/impl/ir/pc-c6-tr02-tr06-realpath-oracle.md` | 実装記録（完了時） |
| `docs/<type>/README.md` | 触れた dir の index を doc-maintenance `index apply` で再生成 |

## Implementation Sequence

- [x] TR02 — `tr02_context_mismatch_dimensions_real_ledger`:
      実経路で live evidence を構成し、7 次元の単独 mismatch と
      全次元 mismatch（`ProjectRevision` 優先）を独立 oracle と
      field 照合、`is_coherent()` 維持を検証。
- [x] TR06 — `tr06_silence_vs_unobserved_real_ledger`:
      sibling target で `Silence`（Current）と `Missing`/
      `Disabled`/`Lost`/未 touch（`NoEvidence`）の区別を実経路で
      検証し、probe 状態が target 間で漏れないことを確認。
- [x] file 冒頭 doc comment を TR01〜TR08 全行対象に更新。
- [x] canonical 検証実行 + doc index 再生成 + impl record。

## Dependencies and Risks

- `src/`・他 crate・manifest・mise・CI・他 TR 行の ledger は変更
  しない。
- oracle と実装の差異が出た場合は実装を oracle に合わせて変更せず、
  契約解釈を spec/design/test-spec に立ち返って確認する。
- TR02 の composite mismatch は documented check order
  （`ProjectRevision` 先）に依存 — test はその順序を独立導出で
  固定し、実装変更時に fail で顕在化させる。
- TR06 の sibling 分離は adapter の probe channel が target 単位で
  独立であることを前提 — 既存 rig の `record_probe` が target key
  で分離されることを確認済み。
- `GraphRuntime` 経路は決定的 headless で実時間に依存しない。

## Task Breakdown

- T1: TR02 test 追加（context 次元行列 + composite 順序）
- T2: TR06 test 追加（silence vs unobserved sibling 分離）
- T3: doc comment 更新 + canonical 検証 + index 再生成 + impl record

## Verification

- [ ] `mise run fmt-check` / `mise run check` / `mise run lint` /
      `mise run test` / `mise run test-rt` /
      `mise run check-app-native` / `mise run lint-app-native` /
      `mise run test-msrv` / `mise run build` — 全 exit status を
      impl record に記録。
- [ ] `pc_c6_oracle_tr_dispatch` で 8 test 全 pass（tr02/tr06 含む）。
- [ ] test-spec `docs/test-specs/pc-c6-tr02-tr06-realpath-oracle.md`
      の各 Feature が fixture で担保されること。
- [ ] `docs/*/README.md` index が `index apply` 後 `index plan` で
      in-sync。

## Open Questions / Gaps

- なし。

## Self-Review

- scope は spec の In Scope と一致。`src/`・他 crate・Cargo 設定への
  変更なし。oracle 独立性は design/ADR の決定どおり。既存 6 test
  の変更なし。

## Implementation Handoff

- Recommended mode: inline execution（dispatched worker）。
- Independent work streams: none。
- User approval required before dispatch: no（worker spec で実装
  済み、review-protocol で承認代替）。
- Executed task-by-task with verification checkpoints。

## Review Handoff

- TR02 の 7 次元が `TraceKey` の実フィールドと 1:1 で対応すること、
  composite 順序の独立導出が documented check order と一致する
  ことを reviewer が確認すること。
- TR06 の `Silence`/`Missing`/`Disabled`/`Lost`/`NoEvidence` の
  分類が test-spec Rule と一致することを確認すること。
- headless/fake-plugin の証拠境界（実 GUI/plugin/OS/RT/consumer は
  #114・#101・#102・Ruviform gate に委譲、`NotRun` 継続）を
  PR/impl record で明示すること。
