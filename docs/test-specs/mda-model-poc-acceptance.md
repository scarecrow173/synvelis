---
id: "TSPEC-SXc0ENzb4xBlpJDsiIaUA8"
type: "test-spec"
status: "approved"
title: "MD-A 検証仕様 — TakeGroup/Comp・Effective Comp 投影・Placement/Playback 時間契約"
created: "2026-11-03"
updated: "2026-11-03"
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
    - "docs/designs/mda-comp-effective-projection-placement-design.md"
  derived-by: []
  verifies:
    - "docs/specs/mda-takegroup-comp-effective-projection-placement-contract.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# MD-A Test Spec — TakeGroup/Comp・Effective Comp 投影・Placement/Playback

検証本体: `crates/synvelis-project/tests/md_a.rs`。headless model PoC —
`ProjectController` + `Command<CompOp|ProjectOp>` のみ使用し、
GUI/RT/DSP は対象外。`CounterIdGenerator` による deterministic ID。

## Feature: MD-A-01 Comp 構築→投影の再現性・Source 不変

- session/pass/stream/segment/take + source/asset を ops で構築し、
  `RegisterTakeGroup`→`CreateComp`→`AddCompRegion` (Take/ExistingSource/
  Silence 各候補)→`SetActiveComp`→`AddCompTransition` が受理される。
- `project_comp` が `region_ids` 順・非重複・provenance (Take/
  ExistingSource/Silence) 付きの EffectiveComp を返す。
- 同一 Project に対する 2 回の投影が完全一致 (再現可能)。
- 投影呼出し前後で `project.sources`/`assets`/`recording.takes` が
  同一 (Source 不変)。
- save→`DirectoryBundleStore`/`InMemoryStore` reload 後の投影が同一。

## Feature: MD-A-02 検証拒否と atomicity

各ケースで validate 失敗・revision 不変・部分適用なし:

- 既存 region と重複する `AddCompRegion`。
- 別 comp の region を参照する `AddCompTransition`。
- `left.end != right.start` の非隣接 region 対への transition。
- overlap が boundary を含まない/`[left.start,right.end]` を外れる
  transition。
- Take 候補で `source_offset+consumed` が take coverage (segment
  source_range union) を超過する region/transition handle。
- overlap 同士が重複する 2 本目の transition。
- transition 付着 region への `SetCompRegionRange`。
- 他 lane take による `RegisterTakeGroup`、他 group の comp を指す
  `SetActiveComp`。

## Feature: MD-A-03 Anchor 再導出 (TempoMap 跨ぎ)

120BPM→ tempo event 追加 (tick 以降 60BPM) の前後で `resolve`:

- Musical anchor + Musical extent + Natural: tick 位置不変・media
  span が新 map で再導出・`source_region.start` 不変。
- Absolute anchor + Absolute extent + Natural: media 位置不変・
  musical 位置が新 map で再導出・source frame 不変。
- Musical anchor + Absolute extent + Natural: anchor tick 不変・
  media end が新 map で再導出・extent 分の media 長が保存 anchor 起点で
  再導出。
- Musical anchor + Musical extent + SimpleStretch(2:1): consumed source
  長が media 長×2 で一貫・source_region.start 不変。
- 上記は `AddTempoEvent` op (controller 経路) で tempo map を変更し、
  `MoveClip`/`SetClipPlayback`/`SetClipExtent` も同一経路で検証。

## Feature: MD-A-04 境界の一意帰属・限定重複

- region A [a,b)・B [b,c) に対し、位置 b が B のみに帰属 (半開区間)。
- `segment_at(pos)`: region 内は一意、gap は 0 件、境界は右 region。
- `transition_at(pos)`: overlap 内は 1 件、外は 0 件。
- EffectiveTransition の left/right binding が post/pre handle として
  overlap 内に限定される (overlap 外での重複再生なし)。

## Feature: MD-A-05 undo/redo・save/reload

- Comp 構築の全 op 列を undo で逆順に元 ID/状態復元 (削除された
  region/transition/comp/group が同 ID で戻る)。
- redo で再現。save→load→同一 projection。
- `SnapshotEnvelope.schema_version == 2` で保存される。

## Feature: MD-A-06 Editor target 型契約

- `SourceVersionRef::new(src, 0)` が拒否 (Raw Capture)。
- `EditableSource(SourceVersionRef::new(src, v>=1))` と `Comp(id)` が
  `EditorEditingTarget` として構築可能。
- `EditorDisplayTarget` の 3 variant が型で区別される。

## 検証コマンド

- `mise run fmt-check` / `mise run check` / `mise run lint` /
  `mise run test` — 全 green。
- `cargo test -p synvelis-project --test md_a` — MD-A-* 本体。
