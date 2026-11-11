---
id: "TSPEC-1O5VKTXpMQMvUNMWnJXn1a"
type: "test-spec"
status: "proposed"
title: "RCD-03 VCS semantic diff monitor 宣言分類 fixture matrix"
created: "2026-11-11"
updated: "2026-11-11"
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
  implements: []
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/rcd-03-vcs-diff-monitor-classification-design.md"
  derived-by: []
  verifies:
    - "docs/specs/rcd-03-vcs-diff-monitor-classification.md"
    - "docs/designs/rcd-03-vcs-diff-monitor-classification-design.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# RCD-03 VCS semantic diff monitor 宣言分類 fixture matrix

## Purpose

schema v12 で永続化される authored monitor 宣言
（`TrackMonitorDecl`/`PunchMarks`）が VCS-02 semantic diff の
`ObjectKind`/`Relation` 分類に正しく現れ、add/remove/modify が
区別され、既存 kind の振る舞いを変えないことを headless
fixture で固定する。実 GUI・VCS-05 apply 適用は対象外。

## Feature

Monitor declaration diff classification —
`ObjectKind::TrackMonitorDecl`（`monitoring.tracks` map 照合）・
`ObjectKind::PunchMarks`（singleton declaration-presence）が
`Recording` domain の `SemanticChange` として決定的に現れる。

## Rules

- **R1 TrackMonitorDecl の存在分類**: `monitoring.tracks` の key
  存在差は `Added`/`Removed`（keyed-synthetic — Track raw と
  衝突を捏造しない）、同一 key の内容差は `Modified` で
  `lane`/`channel_map`/`arm`/`monitor` の field evidence を持つ。
- **R2 PunchMarks の declaration-presence**: 全 mark `None` は
  宣言なし。非宣言→宣言 = `Added`、宣言→非宣言 = `Removed`、
  宣言→宣言(内容差) = `Modified`（`punch_in`/`punch_out`
  evidence）。両 mark `Some` の宣言は `affected_range` に
  `Musical [in, out)` を持つ。
- **R3 domain・決定性・非回帰**: 両 kind は `Recording` domain
  で `included_domains` に従う。monitor 宣言のみの差で
  `overall = Different`、monitor を含む AA は `Equal`。
  monitor 宣言の追加が Track/Lane 等他 kind の change を捏造
  しない。既存 `vcs02_diff.rs`/`vcs06_diff_matrix.rs` fixture は
  変更なしで pass。
- **R4 identity 規則の継承**: decl key の raw が snapshot 間で
  異 kind に衝突する場合、`AmbiguousIdentity(CrossKindId)` が
  出力され decl の Added/Removed/Modified は出ない。

## Examples

- **E1 decl upsert/clear**: `SetTrackMonitor` で宣言を追加 →
  `TrackMonitorDecl Added`（track 本体に change なし）。
  `ClearTrackMonitor` → `Removed`。AB/BA で向きが反転。
- **E2 decl field 変更**: 同一 decl の `arm`/`monitor`/
  `channel_map`/`lane` 変更 → `Modified`、evidence の field
  path が変更 field を含む。
- **E3 punch set/clear/変更**: `{None,None}`→`{in,out}` =
  `Added` + `affected_range = Musical`。`{in,out}`→
  `{None,None}` = `Removed`。`{in,out}`→`{in',out'}` =
  `Modified` + `punch_in`/`punch_out` evidence。
- **E4 domain filter・非回帰**: Recording domain を外した spec
  では monitor change が出ない。monitor 宣言を持つ snapshot の
  AA は `Equal`。既存分類 fixture は全 pass。

## Guarantees

- monitor 宣言のみが異なる snapshot 対は `Different`（`Equal`
  に誤昇格しない）。
- monitor 宣言の change は Track/Lane 等他 kind の change を
  重複報告しない。
- `ChangeSet` の sort・`ChangeId`・summary 規則は既存と同一
  （deterministic 順序に monitor change が参加する）。

## Non-goals

- monitor 宣言 change の VCS-05 apply/restore — apply surface 外
  （fail-closed `Unsupported`）。
- session 観測値・実 device monitor・実 GUI — 非永続/外部 gate。
- `Moved`/`Reordered` 相当の monitor 分類 — 現行意味論に該当
  しない（lane binding は `Modified`、decl は unordered map）。

## Risk

- `monitoring.tracks` を `raw_kind_map` に登録し忘れても
  登録してしまっても壊れる — 未登録だと raw 衝突検出をすり
  抜け、登録すると全 monitor track が `AmbiguousIdentity` を
  捏造。R4 の衝突 fixture と R1 の存在分類 fixture が両方向を
  固定する。
- `PunchMarks` の presence 合成（`{None,None}` = 不在）が
  serialized の常時存在 field とズレる解釈 — R2 の 4 ケースで
  分類を固定する。
