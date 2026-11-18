---
id: "DESIGN-565ProcessorBindingCascade03"
type: "design"
status: "approved"
title: "Issue #565 design — 退役 cascade 監査の着地（契約明記 + test pin）"
created: "2026-11-18"
updated: "2026-11-18"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/565"
    - "docs/specs/issue-565-processor-binding-cascade.md"
    - "docs/discovery/issue-565-processor-binding-cascade.md"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on:
    - "docs/specs/issue-565-processor-binding-cascade.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/plans/issue-565-processor-binding-cascade.md"
    - "crates/synvelis-project/src/plugin_instance.rs"
    - "crates/synvelis-gateway/src/prepare.rs"
    - "crates/synvelis-gateway/src/prepare_engine.rs"
---

# Issue #565 design — 退役 cascade 監査の着地（契約明記 + test pin）

## 全体構造

監査の結論は「不変条件健全・実害なし」。したがって本 slice の
変更は動作を変えない 3 系統に限定される:

1. **契約明記（コメントのみ）**: wire 層の 2 箇所
   （`prepare.rs` op_facts arm・`prepare_engine.rs` op-table
   entry）に、persisted plugin-instance identity の退役・
   再発行が `ResourceRef` で名指せない internal structure
   であり、binding 変更自体は `Processor` `Write` detail で
   観測可能である旨を明記する。domain 層の op 定義コメント
   （`ops.rs`）は退役 id が再発行されないことを補強してよい。
2. **test pin**: op 経路の identity 退役
   （`plugin_instance_persist.rs`）と wire の Write detail
   観測性（`tests/prepare.rs`）を固定する。
3. **記録**: doc chain + `evidence/sv565/` の監査記録と
   `PluginStateProbe` staleness の `synvelis#128` defer 判定。

`plugin_instance.rs` 本体は不変条件が既に module doc で
完全に明文化されており変更しない。`unsupported` 追加・
store cascade 変更・`ResourceKind` 拡張は全て非対象。

## 契約明記の設計

### `prepare_engine.rs` op-table entry

`synvelis.processor.set-binding` entry（:703-709）の直上に、
`insert-processor` の parenthetical
「ports/plugin-instance identity are internal structure」と
send 系コメント（:710-720）の様式で、set-binding 固有の
posture を 1 文で明記する。要点:

- rebind は persisted plugin-instance identity を退役させ
  新 id を発行する（`sync_processor` semantics）
- 退役・再発行される identity は `ResourceRef` で名指せない
  internal structure（`ResourceKind`/`ENUMERATED_KINDS` 非存在）
- binding 変更自体は対象 `Processor` の `Write` detail に
  現れる（`detail_of` が `binding` を含む `Processor` 全体を
  serialize）

枠組みコメント（:655-660）の「`param.set-base`/
`processor.set-binding` write existing control/routing
entities only」は事実として正しいが persisted identity の
退役に踏み込んでいない — entry 直上のコメントで補う形が
最小で既存様式とも整合する。

### `prepare.rs` op_facts arm

`SetProcessorBinding` arm（:1412-1419）の `read`/
`precondition` 群の直前または直後に、同じ posture を
1-2 文で明記する。`AddSend`/`RemoveSend` arm の
「internal structure」コメント（:1430-1435）と同じ
トーンに揃える。

### `ops.rs` op 定義コメント（補強・任意）

既存コメント（:530-541）は「Resolved rebinds mint a fresh
persisted plugin-instance identity … `Unresolved` drops the
identity entry」と既に正確。退役した id が watermark により
再発行されないことを 1 句補うと、Issue の主題（退役
cascade）に対して契約が自己完結する。apply arm 内コメント
（:2389-2393）は既に「post-apply oracle re-checks the
bijection」まで明記しており変更不要と判断 — op 定義側のみ
補強する。

## test pin 設計

### `plugin_instance_persist.rs`（op 経路 pin・3 件）

既存 fixture（`controller()`/`strip_project()`/
`insert_processor()`/`instance_id()`）を再利用し、
`ProjectOp::SetProcessorBinding` を `apply_op` で適用する。

| test | 操作 | 期待 |
| --- | --- | --- |
| `set_binding_rebind_retires_old_instance_id` | Resolved(A) 挿入 → `SetProcessorBinding` で Resolved(B)（descriptor drift） | 新 id=2 発行・entry.plugin=B・watermark=3・旧 id=1 は store 内のどの entry にも存在しない・`resolve(proc, A)`→`None`・`resolve(proc, B)`→`Some(2)`・`validate_references` 通過 |
| `set_binding_same_plugin_keeps_instance_id` | Resolved(A) 挿入 → 同一 PluginRef で `SetProcessorBinding` | id=1 維持・watermark=2 のまま（`ensure` 冪等）・`validate_references` 通過 |
| `set_binding_unresolved_retires_instance_id` | Resolved(A) 挿入 → `Unresolved` へ `SetProcessorBinding` | entry 削除・id=1 退役（watermark=2 のまま後退しない）・`resolve`→`None`・`validate_references` 通過。続けて別 processor 挿入で id=2 が発行され id=1 が再発行されないことも確認できるが、既存 `remove_then_insert_never_reuses_id` が同型を担保するため本 test では entry 削除と oracle 通過に留める |

`resolve` は `pub` なので integration test から直接呼べる
（`plugin_instance.rs:178` は `#[must_use] pub fn`）。

### `tests/prepare.rs`（wire pin・1-2 件）

sv#561 節（:1174-）の様式に倣い、ファイルローカル
fixture で `ProjectController` に track+strip+Resolved
processor を op で構築し、`sv561_commit` 相当の helper で
pinned base に commit してから wire prepare する。

pin 内容:

- `synvelis.processor.set-binding` を別 `PluginRef` の
  `Resolved` binding に向けた prepare が `Completed` で
  proposal を発行する（`unsupported` なし — 過剰拒否なし
  の反証を兼ねる）
- `Write` 効果が `ResourceRef::Processor(proc)` に対し、
  before/after detail の `binding` フィールド差分を運ぶ
- `record.prepared.candidate.plugin_instances` で新 id 発行・
  旧 entry 置換・watermark 前進を確認（candidate は
  `Arc<Project>` で直接読める — prepare の trial-apply が
  `sync_processor` を通った証拠）

fixture は `demo_project()`（rig 共有）を clone するか、
`ProjectController::new(Project::new(...))` で最小構成を
op 構築する — sv561 の `sv561_track_strip_controller` と
同型の file-local builder を新設する（共有 `rig` fixture
`tests/rig/mod.rs` は非接触）。

## リスクと対策

- **コメントの drift**: posture コメントは code 参照を
  名指し（`sync_processor`/`detail_of`/`ResourceKind`）で
  曖昧さを避ける。将来 `ResourceKind` に PluginInstance が
  追加された場合はコメントの前提が変わるため、その時点で
  見直す旨をコメントに含めない（YAGNI — 現行契約のみ記述）。
- **test の脆さ**: `instance_id` helper は既存のまま使い、
  id の具体値（1, 2）を pin する — watermark 単調性は
  既存 test が担保済みで、本 slice は「rebind で +1」の
  相対関係を中心に assert する。
- **sv#559 との干渉**: `apply/entity.rs`・`variant/*` は
  読み取りのみ。監査でそれらへの修正が必要と確定した場合は
  `ask` で escalate する（現時点で不要と判断）。

## 残件（記録のみ・本 slice 非対象）

- `PluginStateProbe::evidence(ProcessorId)` の staleness:
  evidence が plugin/instance identity を持たず rebind 後に
  旧 plugin 用 evidence が新 plugin に attach し得る。
  in-tree 実装は fixture のみで実害実測なし。`synvelis#128`
  （TIM-04 plugin state snapshot 一貫性）へ defer — port
  契約の世代区別は TIM-04 の設計に属する。
- `docs/**/README.md` index は Coordinator が統合時に再生成。
