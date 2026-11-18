---
id: "IR-561OpFactsUnverifiable07"
type: "impl"
status: "completed"
title: "Issue #561 impl IR — op_facts unverifiable cascade 実装記録"
created: "2026-11-18"
updated: "2026-11-18"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/561"
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
      - type: "file"
        path: "crates/synvelis-project/tests/sv561_unobservable_cascade.rs"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/issue-561-opfacts-unverifiable-cascade.md"
  implemented-by: []
  depends-on:
    - "docs/plans/issue-561-opfacts-unverifiable-cascade.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/ir/issue-550-strip-routing-ops.md"
    - "docs/impl/ir/issue-552-sendgain-seal.md"
    - "docs/impl/ir/issue-553-intent-facts.md"
    - "docs/impl/ir/issue-557-adopt-facts.md"
    - "docs/impl/exp/issue-561-opfacts-unverifiable-cascade.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/plans/issue-561-opfacts-unverifiable-cascade.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/issue-561-opfacts-unverifiable-cascade.md"
  references: []
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted:
      - "docs/impl/exp/issue-561-opfacts-unverifiable-cascade.md"
    rejected: []
---

# Issue #561 impl IR — op_facts unverifiable cascade 実装記録

## 実装サマリ

`op_facts`（`crates/synvelis-gateway/src/prepare.rs`）の
`SetSendGainTarget`・`RemoveTrack`・`RemoveMidiSource` 3 arm
に unverifiable 検査を追加し、`RemoveSend` arm の inline
ロジックを `ControlOwner` 版 helper
`control_closure_unverifiable` へ semantics-preserving に
抽出した。domain（`synvelis-project`）は非接触 — cascade
の発火条件は既存仕様のまま mirror するのみ。

## 変更内容

### `prepare.rs`

- `control_closure_unverifiable(project, owner) -> bool`:
  owner の control closure（`ParameterTarget`/`ModulationSource`
  集合）を収集し、`!sources.is_empty() || assignments.any(
  a.source ∈ sources || a.target ∈ targets ||
  a.depth_source ∈ targets)` で判定。domain
  `remove_control_owned`（`ops.rs:2666`）の削除条件と一致。
- `RemoveSend` arm: inline 収集+判定を helper 呼出に置換
  （同一文言 `"send-owned modulation entities are outside
  the observable model"` を維持 — semantics-preserving）。
- `RemoveTrack` arm: `project.monitoring.tracks
  .contains_key(track_id)` で無条件に unsupported を立てる
  （`"track monitor declaration deletion is outside the
  observable model"` — decl 削除は strip 有無に関わらず
  apply が無条件実行し、`TrackMonitorDecl` は
  `ResourceKind`/`detail_of` 双方に非存在）。さらに
  `strip_of_track` が strip を返すとき
  `ControlOwner::Strip(strip.id)` で helper を評価し、
  unverifiable なら `"strip-owned modulation entities are
  outside the observable model"` を push。
- `SetSendGainTarget` arm: apply の seal 解放発火条件
  （`previous` が `Some(old)`・`Some(old) != *gain_target`・
  旧 target が `ControlOwner::Send(send_id)` +
  `ParameterRole::SendLevel`）をミラーし、さらに旧 target
  に触れる assignment（`a.target == old ||
  a.depth_source == Some(old)` — `release_send_level_target`
  の assignment retain 条件と一致）の存在を検査して
  unsupported を立てる。
- `RemoveMidiSource` arm: `project.smf_documents
  .contains_key(source_id)` で unsupported を立てる
  （`"smf document deletion is outside the observable
  model"`）。
- `Composite` arm: `project.clone()` の scratch を
  `CounterIdGenerator::new(project.id_watermark)` で seed
  し、各 member の facts 評価後に `lower_bound_batch`
  （`prepare_engine.rs:1562-1626`）と同じ trial-apply
  手順（validate → clone → apply → 成功時のみ swap）で
  scratch を進める。member が validate/apply に失敗した
  場合 composite は commit 不能のため scratch は最後の
  到達可能状態に留まる。これにより batch 内で seal→
  release される `gain_target` 等の中間状態に対しても
  unverifiable 検査が正しく発火する（review 指摘対応 —
  pinned base 評価では `AddSend{gain_target}`+`RemoveSend`
  や seal→clear pair が under-report し得た。#550 の
  `RemoveSend` 検査にも同じ stale-state 穴があり本修正で
  まとめて解消）。`Operation`/`CounterIdGenerator` を
  import に追加。

### `tests/prepare.rs`（wire pin 10 件）

ファイルローカル fixture（`ProjectController` で構築、共有
`rig` fixture は非接触）+ pin test:

- denied 7 件: `sv561_set_gain_target_clear/rebind_
  unobservable_assignment_denied`・`sv561_set_gain_target_
  clear_unobservable_depth_source_denied`・`sv561_send_
  remove_unobservable_assignment_denied`・`sv561_track_
  remove_unobservable_strip_cascade_denied`・`sv561_track_
  remove_monitor_decl_denied`・`sv561_midi_source_remove_
  smf_document_denied`・`sv561_batch_seal_release_
  unobservable_denied` — `PreparationRejected`
  "unverifiable" + proposal_count 0 を assert。
- positive control 2 件: `sv561_set_gain_target_clear_
  without_assignment_accepted`・`sv561_track_remove_
  plain_strip_accepted` — 受理 + 観測可能 delete が
  effects に現れることを assert（過剰拒否なし）。
- fixture は `Sv561Assignment` enum（`OnTarget`/
  `ViaDepthSource`）で assignment の target/depth_source
  両 leg を網羅し、`sv561_batch_seal_controller` が
  Composite 逐次評価の pin を構築する。

### `synvelis-project/tests/sv561_unobservable_cascade.rs`（domain pin 7 件）

`release_send_level_target` 発火（target/depth_source 両
leg）・`remove_control_owned` の send-owned target leg・
`remove_strip_owned` cascade・`smf_documents` 削除・
`monitoring.tracks` decl 削除・同一 target 非 release の
7 件で domain 側の発火条件を固定し、gateway mirror の
乖離を防ぐ。

## 判断記録

- **`monitoring.tracks` decl**: 初版は「TrackId key の
  strictly-1:1 companion で受理内」と判断したが、レビュー
  で `smf_documents` との非整合が指摘された — decl は
  optional（0-or-1）で独立 content を持つ keyed companion
  であり、strip chain/ports の様な必須内部構造ではない。
  同一修正内で SMF を unverifiable とした判断と整合させ、
  `contains_key` 検査を追加した（apply は strip 有無に
  関わらず無条件で decl を削除）。
- **Composite stale-state**: `op_facts` が全 member を
  pinned base に対して評価するため、batch 内で seal→
  release される状態を見逃す under-report を review で
  指摘された。`lower_bound_batch` の trial-apply と同型の
  逐次 scratch 評価に修正 — member の reads/impacted/
  preconditions も中間状態を反映し従来より正確。
- **`SetSendGainTarget` の unsupported 条件**: apply の
  発火条件を厳密にミラー — 条件を満たさない rebind/clear
  では立てない（過剰拒否 = 既存受理の回帰を防ぐ）。
  positive control + 既存 cb06 `send_set_gain_target_op`
  （assignment なし clear が variant edit で受理継続）で
  反証。
- **helper 統合範囲**: `RemoveSend`/`RemoveTrack` のみ。
  `RemoveMidiSource` の `smf_documents` 検査は別種の
  cascade なので統合しない。

## 残件

- `SetProcessorBinding` の `sync_processor` が
  `plugin_instances` の旧 `PluginInstanceId` を退役させる
  cascade は本 slice 範囲外（main 既存・別クラスの
  persisted identity）。別 Issue での検討事項として記録。
- `docs/impl/{ir,exp}/README.md` index への行追加は
  index 再生成の lease 外。

## 検証

`evidence/sv561/verification.md` 参照 — `mise run
{check,lint,test} synvelis-gateway`・`{check,lint,test}
synvelis-project`・`fmt-check`・`check-headless-deps`・
`prek run --all-files` 全て exit 0。red→green は
`evidence/sv561/reproduction.md`（初版 4 件）と
`verification.md`（review 追加分 2 件 — prepare.rs を
一時 revert して `Completed` 受理を実測確認後に修正を
復元）。
