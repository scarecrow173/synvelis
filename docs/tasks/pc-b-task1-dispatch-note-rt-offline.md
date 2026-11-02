---
id: "TASK-034PCB06Rm1aUv7bDeFgHi"
type: "task"
status: "done"
title: "PC-B: dispatch/note-scope/RT/offline 実装タスク"
created: "2026-11-04"
updated: "2026-11-04"
owners: []
relations:
  source: []
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/pc-b-parameter-dispatch.md"
  implemented-by: []
  depends-on:
    - "docs/specs/pc-b-parameter-dispatch-note-scope-rt-offline.md"
    - "docs/designs/pc-b-parameter-dispatch-note-scope-design.md"
    - "docs/adr/pc-b-dispatch-boundary-note-entity-prepared-eval.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/pc-b-e2e-parameter-dispatch.md"
  references: []
  defers: []
  deferred-by: []
---

# PC-B 実装タスク

## T1 — Note entity + scope 解決 (synvelis-project)

- `Note { id, clip, key, start, len }` を project.rs に追加、
  `Project.notes: BTreeMap<NoteId, Note>`(serde default)。
- `ProjectOp::AddNote`/`RemoveNote` + apply/validate。
- `resolve_scope_pos` の `Note` arm を実装(clip arm 合成 + domain
  チェック)。
- `Project::validate_references` に note 参照整合性を追加、
  `SCHEMA_VERSION = 4`、clip 削除時の note cascade。
- lib.rs exports。

## T2 — 記録経路 scope 正規化 (synvelis-workspace)

- `record_points(project, target, points)` で scope 座標へ変換、
  domain 外点を skip。
- `record_write`/`gesture_end` に `&Project` を追加、
  `transport_advance`/`session.rs`/`pc_a.rs` 適合。

## T3 — prepared eval + rt_ テスト (synvelis-project)

- `EvalPlan` に prepared target データ(series/assignments/
  capability)を保持、`eval_rt` で alloc-free 評価。
- `tests/pc_b_rt.rs`: `assert_no_alloc` + stats_alloc スタイルの
  rt_ テスト(dev-dep `assert-no-alloc` 追加)。

## T4 — dispatch 経路 (synvelis-player)

- `src/dispatch.rs`: `ParamDispatch`/`DispatchError`。
- Cargo.toml に `ruviform-plugin-host`/`ruviform-audio-plugin-bridge`
  追加(workspace deps にも登録)。
- `tests/pc_b.rs`: PluginHost + fixture stub + attach + graph +
  RtRuntime で endpoint 配送・拒否伝播・ledger 観測を検証。

## T5 — offline parity + 検証 + impl record

- player `pc_b.rs`: 同一 plan の playback drive vs `render()` で
  (param, absolute frame, value) 一致。
- `git merge origin/main` → 生成物再生成 → `mise run fmt/fmt-check/
  check/lint/test` → `docs/impl/ir/pc-b-*.md` → commit。
