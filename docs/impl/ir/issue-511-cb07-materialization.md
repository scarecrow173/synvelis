---
id: "IMPL-sv511hI0jK1lM3nO5pQ7rS9t"
type: "impl"
status: "completed"
title: "issue-511-cb07-materialization"
created: "2026-11-22"
updated: "2026-11-22"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/511"
    - "docs/plans/issue-511-cb07-materialization.md"
    - "docs/tasks/issue-511-cb07-materialization.md"
    - "docs/adr/issue-511-cb07-materialization.md"
  changes:
    added:
      - type: "file"
        path: "docs/discovery/issue-511-cb07-materialization.md"
      - type: "file"
        path: "docs/specs/issue-511-cb07-materialization.md"
      - type: "file"
        path: "docs/designs/issue-511-cb07-materialization.md"
      - type: "file"
        path: "docs/plans/issue-511-cb07-materialization.md"
      - type: "file"
        path: "docs/tasks/issue-511-cb07-materialization.md"
      - type: "file"
        path: "docs/test-specs/issue-511-cb07-materialization.md"
      - type: "file"
        path: "docs/adr/issue-511-cb07-materialization.md"
      - type: "file"
        path: "docs/impl/ir/issue-511-cb07-materialization.md"
      - type: "file"
        path: "docs/impl/exp/issue-511-cb07-materialization.md"
      - type: "file"
        path: "docs/impl/exp/issue-511-cb07-materialization-exp.jsonl"
      - type: "dir"
        path: "evidence/sv511/"
    modified:
      - type: "file"
        path: "crates/synvelis-project/src/variant/engine.rs"
      - type: "file"
        path: "crates/synvelis-project/src/apply/entity.rs"
      - type: "file"
        path: "crates/synvelis-project/src/ppqn.rs"
      - type: "file"
        path: "crates/synvelis-project/tests/cb07_arrangement.rs"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/issue-511-cb07-materialization.md"
    - "docs/tasks/issue-511-cb07-materialization.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/ir/cb-07-arrangement-adapter.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/tasks/issue-511-cb07-materialization.md"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "evidence/sv511/"
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted:
      - "docs/impl/exp/issue-511-cb07-materialization-exp.jsonl"
    rejected: []
---

# issue-511-cb07-materialization — Implementation Record

## 実装内容

### `apply/entity.rs`

- `removal_supported_kind(kind)` を追加 — `Clip | Note` のみ
  `true`。adopt 経路の `Removed` materialization 対象を限定し、
  他 kind は typed `Unsupported` を維持する fail-closed 述語。
- `write_entity` 冒頭に `ChangeKind::Removed` 分岐を追加し
  `remove_entity` へ委譲。
- `remove_entity`: Clip は `project.clips` 除去 + 全 lane の
  `clip_ids` retain + `note.clip == clip` の notes cascade 除去
  （`ProjectOp::RemoveClip` と同一 semantics）。Note は
  `project.notes` 除去（Clip cascade 先行に備え冪等）。
  未対応 kind は `Error::Validation`。

### `variant/engine.rs`

- `plan_adopt_writes` の removal ループを 3 分岐化:
  `origin ∩ live_scope − cand_scope` の entity が candidate に
  実在 → move-out `Unsupported`。candidate に不在 →
  `removal_supported_kind` なら `Removed` write（payload
  `serde_json::Value::Null`）、非対応 kind は `Unsupported`。
- VCS `apply_selected`（`plan.rs` `change_supported`）は無変更
  — adopt 経路限定の実装で共有 surface を分裂させない。

### `ppqn.rs`

- `reproject_ppqn` に `Captured` payload 存在チェックを追加 —
  いずれかの variant set の branch が `BranchContentRef::Captured`
  を保持する場合 `Error::Validation` で拒否。`validate`/`apply`/
  `preview_ppqn` の 3 経路が同一関数を通るため authoritative
  boundary は 1 箇所。identity no-op（old == new）は従来どおり
  早期 return で拒否対象外。

### tests（`cb07_arrangement.rs`）

- `cb_arr_11_delete_candidate_adopts_and_renders_materialized`:
  旧 `cb_arr_11_delete_candidate_editable_but_adopt_defers_typed`
  を新契約へ書き換え — adopt Applied/objects=2/revision+1/
  undo_steps=1、confinement、undo 完全復元、render 解決の
  composed materialize を pin。
- `cb_arr_08_mask_gating_clips_only_and_notes_only`: notes-only
  candidate の adopt pin を Unsupported → Applied/objects=2
  （Removed note + Added note）へ更新。
- `cb_arr_14_ppqn_rejects_captured_payloads_then_reprojects_scope_
  time`: fork 直後の SetProjectPpqn 拒否（live 不変）→
  `discard_branch` で全 Live 化 → 再射影成功・scope time 半減・
  boundary inventory 安定の 2 相構成へ。
- `cb_arr_16_sweep_delete_adopts_and_cascades_notes` 新規:
  clips-only mask で `RemoveClipsInRange` → clip のみ
  scope-owned removal（objects=1）・note cascade・lane
  membership 整理を pin。

## 設計判断の記録

- payload `Null`: `Removed` write の after entity は存在しない。
  `preflight_*`/`introduced_digests` は Added/kind 限定で自然
  スキップ — `apply/plan.rs` 無変更で整合。
- `Captured` 判定に frozen 履歴を含める: adopt 時に旧 adopted
  branch が `Captured` へ凍結され再 adopt 可能なため、編集中
  candidate と区別せず全 `Captured` を拒否対象とする保守的契約。
- absolute anchor: tempo-map 変換 policy 未確立のため
  `AbsoluteAnchor` fail-closed を維持（`scope.rs`/
  `arrangement.rs` 無変更）。

## 検証

`evidence/sv511/verification.md` に lane 別 Pass/NotRun を
記録。headless fixture lane のみ Pass 判定。
