---
id: "DSGN-vcs05-monitoring-apply-surface"
type: "design"
status: "approved"
title: "VCS-05 monitoring kinds apply surface 拡張 design"
created: "2026-11-12"
updated: "2026-11-12"
owners: []
relations:
  source:
    - "SPEC-vcs05-monitoring-apply-surface"
  verified_by:
    - "TSPEC-vcs05-monitoring-apply-surface"
  implemented_by:
    - "TASK-vcs05-monitoring-apply-surface"
  derives_from:
    - "SPEC-vcs05-monitoring-apply-surface"
    - "DSGN-vcs05-apply-restore-headless"
    - "docs/designs/rcd-03-vcs-diff-monitor-classification-design.md"
  supersedes: []
  superseded_by: []
---

# VCS-05 monitoring kinds apply surface 拡張 design

## Overview

VCS-05 apply の entity 転置機構（`entity_value` → fingerprint →
`payload_for` → `write_entity`）に、diff 分類済みの monitoring
kind 2 種を載せる。両 kind とも既存 entity map に属さない特殊な
identity を持つため、`keyed!`/`keyed_insert!` macro ではなく
専用 arm で扱う。`plan.rs`・preflight・journal・engine の制御
フローは変更しない — whitelist・presence 規則・`Removed` 拒否は
すべて既存機構がそのまま効く。

```
SemanticObjectRef(kind=TrackMonitorDecl, stable_id=track-<hex>)
  → entity_value: TrackId::from_tagged(stable_id)
      → project.monitoring.tracks.get(&track) → Option<decl JSON>
  → write_entity: TrackId::from_tagged(stable_id)
      → project.monitoring.tracks.insert(track, decl)

SemanticObjectRef(kind=PunchMarks, stable_id=<project tag>)
  → entity_value: punch_in|punch_out が Some → Some(marks JSON)
                  全 None → None（absent = null）
  → write_entity: project.monitoring.punch = marks
```

## TrackMonitorDecl — keyed-synthetic view

`monitoring.tracks` は `BTreeMap<TrackId, TrackMonitorDecl>` で、
decl 自体は id を持たない。diff 側（PR #318）では `diff_map` が
key の `tagged()` を `stable_id` に使う keyed-synthetic
collection として分類されており、apply 側も同じ identity 規約に
従う。

- `entity_value`: `stable_id` を `TrackId::from_tagged` で parse
  し `project.monitoring.tracks` を参照する。`keyed!` macro と
  同じ「stable_id → id → map lookup」形だが、lookup 先が
  `project.tracks` ではなく `project.monitoring.tracks` であり、
  値型も `Track` ではなく `TrackMonitorDecl` — macro の型パラメ
  タでは表現できないため専用 arm とする。parse 失敗は
  `invalid_id`（`ApplyError::InvalidIntent`）で fail-closed。
- `write_entity`: `from_value::<TrackMonitorDecl>` で payload を
  復元し `monitoring.tracks.insert(track, decl)`。宣言は id を
  持たず `max_new_id` に寄与しない — 同一 apply で対象 track
  自体が `Added` なら `Track` write の `keyed_insert!` が
  reseed を担うため、ここで watermark を触る必要はない
  （comment で明記）。
- `protected_fields`: 追加なし。decl は membership list を持たず
  （`lane` は binding target であって親子 membership ではない
  — diff 分類 design と同じ位置付け）、reconcile 対象外。
- 存在規則は `plan_changes` の既存 gate がそのまま効く:
  `Added` は live に decl 不在、`Modified` は live に decl 存在
  を要求（不在なら `TargetMissing`、冗長なら `TargetConflict`）。

## PunchMarks — singleton + declaration-presence

`monitoring.punch` は project 全体の値型 singleton で、
`{punch_in: None, punch_out: None}` が「宣言なし」を表す
（`SetPunchMarks{None,None}` = 解除の authored 意味）。
diff classifier（`diff_punch_marks`）と同じ presence 規則を
entity view にも適用する:

- `is_declared(m) = m.punch_in.is_some() || m.punch_out.is_some()`
- 宣言あり → `entity_value` は `Some(to_value(marks))`
- 全 `None` → `Ok(None)` — absent-entity encoding（fingerprint
  では `null`）。serialized 上 field は常に存在するが authored
  意味では宣言不在であり、diff の `Added`/`Removed` 分類と
  presence を一致させる（「Removed なのに before に entity が
  あった」ように見える serialized 形状のずれは diff design で
  規則化済みのものをそのまま踏襲）。
- `stable_id` は project tag（diff が `project_id.to_tagged()`
  を使うのと同じ）だが、entity view は singleton のため
  `stable_id` を parse せず `project.monitoring.punch` を直接
  参照する。`RoutingModel`/`Project` の singleton view と同型。
- `write_entity`: `from_value::<PunchMarks>` で payload を復元し
  `project.monitoring.punch = marks` を代入。id を持たないため
  `max_new_id` 対象外。
- 存在規則: `Added` は live punch が全 `None`、`Modified` は
  live punch が宣言済みを要求。宣言→全 `None` の遷移は diff が
  `Removed` に分類するため apply 面では `Unsupported` で
  fail-closed（cascade deferred）。

## 変更しない面

- `supported_kind`: `matches!` に 2 variant を追加するのみ。
  `PluginInstance`（#324 で diff 分類済み）は追加しない —
  apply transplant/preflight の設計は別 slice。
- `plan.rs`: `change_supported` の `Removed` → `Unsupported`、
  kind 別 `Reordered`/`Reconnected`/`Replaced`/`Moved` gate、
  existence 規則、`check_singleton_fields`（Project/RoutingModel
  のみ）はすべて既存のまま monitoring kind に正しく効く。
  変更不要。
- preflight（`introduced_digests`/`preflight_assets`/
  `preflight_plugins`/`preflight_ports`）: monitoring payload は
  digest・plugin binding・port 参照を含まないため `_` arm で
  素通し — 変更不要。
- `fingerprint`: `entity_value` 経由で自動的に monitoring entity
  を canonical JSON 化。`{kind}|{stable_id}|{value}` の material
  は kind を含むため、同一 `stable_id`（track tag）を共有する
  `Track` change と `TrackMonitorDecl` change は fingerprint 上
  別 entity として区別される。
- journal/receipt/undo: `ApplySelectedOp` は project 全体の
  scratch clone に `write_all` する単一 `Operation` —
  `project.monitoring` は `Project` の一部として undo/redo・
  validation oracle（`validate_references`）に自動的に乗る。

## Failure / Edge Cases

- `stable_id` が `TrackId` tagged 形式でない
  `TrackMonitorDecl` object → `from_tagged` 失敗 →
  `InvalidIntent`/`InvalidId` で fail-closed（diff が生成しない
  異常入力への防御であり、通常経路では発生しない）。
- `Added` decl が live 既存 decl と衝突 → `TargetConflict`。
  `Modified` decl が live 不在 → `TargetMissing`。
- apply 対象 track 自体が未存在（decl のみ選択で track が
  live に無い）→ `Modified` は `TargetMissing`、`Added` は
  `monitoring.tracks` に挿入自体は成功するが track 不在の decl
  は `validate_references` が load/apply 時に拒否する
  （decl.lane の整合と同じ oracle 経路）— oracle が最終判定者。
- punch `Modified` で payload が不正 JSON → `from_value` の
  `Error::Validation` → `ApplyError::Rejected` で fail-closed。

## Compatibility / Migration Notes

- `ObjectKind`・serde 名・`DIFF_POLICY_VERSION`（main 上 3 —
  #324 の `PluginInstance` 追加による）は変更しない。本 slice
  は apply surface のみの拡張で diff semantics は不変。
- 既存 project（monitoring 空含む）の apply 挙動は不変 —
  monitoring change を含まない selection は同じ plan/payload を
  通る。monitoring change を含む selection は旧来 `Unsupported`
  だったものが適用可能になる意図的な契約変更であり、
  `expected_target_fingerprint` を caller が再計算する既存
  手順どおりに動く（fingerprint material は entity_value の
  canonical JSON で、whitelist 追加前は計算自体が失敗していた
  ため旧 fingerprint との衝突は起きない）。

## Risks / Trade-offs

- `PunchMarks` の presence 合成（全 `None` = absent）は diff と
  apply で同一規則を 2 箇所に持つ — 片方だけ変更されると
  fingerprint/presence がずれる。規則の正本は diff classifier
  であり、本実装はそれに合わせる旨を code comment に明記した。
  将来的に drift した場合は test-spec の presence 遷移 fixture
  が検出する。
- `TrackMonitorDecl` の `lane` field は `Option<LaneId>` —
  payload の lane が live/after で無効な lane を指す場合、
  write 後の `validate_references` が拒否する（oracle fail-closed
  の既存契約。apply 層での事前検査は追加しない — YAGNI、
  single-writer oracle が権威）。
- `Removed` を許可しない判断は本 Spec で確定: decl clear /
  punch clear の apply は「削除 cascade」ではなく単純な値戻しに
  見えるが、selection が track/punch entity の不在を意味する
  以上、他 kind の `Removed` と同じ cascade 設計問題を共有する。
  部分的に monitoring だけ `Removed` を許すと「Removed は全 kind
  deferred」という apply surface の単純な不変条件が崩れるため、
  本 slice では許可しない。

## Open Questions / Gaps

- `Removed` cascade・`PluginInstance` apply・recording/comping/
  SMF 領域の apply は別設計（spec Deferred Design Concerns 参照）。
