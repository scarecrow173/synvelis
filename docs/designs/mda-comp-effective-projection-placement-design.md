---
id: "DESIGN-YFKePNnF4peFnskSfrnSc2"
type: "design"
status: "approved"
title: "MD-A Design — TakeGroup/Comp スキーマ・Effective Comp 投影・正規化 Placement/Playback 時間契約"
created: "2026-11-03"
updated: "2026-11-03"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/42"
    - "https://github.com/scarecrow173/synvelis/issues/10"
    - "https://github.com/scarecrow173/synvelis/issues/12"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/mda-takegroup-comp-effective-projection-placement-contract.md"
  depends-on:
    - "docs/adr/mda-normalized-placement-comp-schema.md"
    - "docs/designs/vs-a-project-recording-foundation-stable-id-command-poc.md"
  derives-from:
    - "docs/specs/mda-takegroup-comp-effective-projection-placement-contract.md"
    - "docs/adr/mda-normalized-placement-comp-schema.md"
---

# MD-A Design — TakeGroup/Comp・Effective Comp 投影・Placement/Playback 時間契約

## Overview

```
Take (録音候補, 実装済み)                Clip (正規化後)
  └─ TakeGroup { owner_lane_id,          ├─ placement: ClipPlacement
       target_range, take_ids,            │    { anchor: Musical|Absolute,
       active_comp_id, source_rate }      │      extent: Musical|Absolute }
       └─ Comp (採用区間の組合せ)          └─ playback: ClipPlayback
            ├─ CompRegion[] (順序付き・      { source_region: SourceRange,
            │   非重複 [start,end))         │   mapping: Natural|Loop|
            │   candidate: Take/ソース/     │     SimpleStretch(r) }
            │   Silence + mapping
            └─ CompTransition[] (独立ID,
                 boundary=left.end=
                 right.start, overlap,
                 fade_out/fade_in)

CompOp ──► ProjectController ──► validate(全件) ──► atomic apply ──► undo
                                     │
                                     ▼
                          project_comp(project, comp_id) ── 純粋導出
                          EffectiveComp { segments(順序付き,
                          provenance+source binding), transitions }
                                     │
                          EditorDisplayTarget / EditorEditingTarget
                          (Comp(CompId) | EditableSource(v>=1))
```

全型は `synvelis-project` 所有 (DAW 意味論)。`synvelis-recording` の
commit 経路と `synvelis-player` の lowering は新 `Clip` スキーマに
機械的適合させるのみ。

## 時間ドメインと正本

| 正本 | 型 | 意味 |
| --- | --- | --- |
| Placement anchor | `TimeAnchor = Musical(MusicalTick) \| Absolute(MediaTimeValue)` | timeline 上の**点**。保存・undo の正本 |
| Extent | `ClipExtent = Musical(MusicalDuration) \| Absolute(MediaDuration)` | anchor からの長さ (点ではなく duration) |
| Playback | `ClipPlayback { source_region: SourceRange, mapping }` | source 読み出し窓 + 写像 |
| Comp project 区間 | `ProjectRange = Musical(MusicalRange) \| Media(MediaSpan)` (旧 `TimeAnchor` enum rename) | region/transition の半開区間 |
| Boundary | `ProjectTime` (導出値) | transition の切替点 |

- Media↔Musical 変換は `TickTimeMapping` (Ruviform TempoMap) のみ経由。
  Anchor は保存正本、media/musical 両方向の派生位置は resolve で再導出。
- `source_region.start()` が旧 `source_offset` の役割 (Source Native
  Frame)。TempoMap 編集・Comp 編集で不変。
- `PlaybackMapping`: `Natural` (1:1) / `Loop` (window を繰返し) /
  `SimpleStretch(StretchRatio{num,den}>0)` (source 消費 = media 長 ×
  num/den, floor)。Warp は後続。

## resolve 契約 (ClipPlacement)

`resolve(placement, playback, map, rate) -> ResolvedClip` が唯一の
変換実装 (lowering もこれを消費):

- `media_start` = Musical anchor → `map.tick_to_media(tick)`;
  Absolute anchor → media value を `rate` base へ floor 変換。
- `media_end` = extent Musical → `tick_to_media(anchor_tick + dur)`
  (Absolute anchor は `media_to_tick` で anchor tick を一度だけ再導出);
  extent Absolute → `media_start + dur`。
- `source_start = source_region.start()` — TempoMap 非依存・不変。
- `source_len` = Natural/Loop → media 長 frames; Stretch →
  `floor(media_len × num/den)`。
- 検証: extent>0、region 非空、消費長 ≤ window (Loop は window>0 のみ
  要求・繰返しで超過を許容)。

## Comp スキーマ

```rust
struct TakeGroup { id, owner_lane_id, target_range: ProjectRange,
                   take_ids: Vec<TakeId>, active_comp_id: Option<CompId>,
                   source_rate: u32 }
struct Comp { id, take_group_id, region_ids: Vec<CompRegionId>,
              transition_ids: Vec<CompTransitionId> }
struct CompRegion { id, comp_id, project_range: ProjectRange,
                    candidate: CompCandidateRef, playback: PlaybackMapping }
enum CompCandidateRef {
    Take { take_id, source_offset: SourceFrame },          // region 先頭↔offset
    ExistingSource { source_id, source_range: SourceRange }, // window 明示
    Silence,
}
struct CompTransition { id, comp_id, left_region_id, right_region_id,
                        overlap_range: ProjectRange,
                        fade_out: FadeShape, fade_in: FadeShape }
    // boundary = right.start (= left.end) を導出・非保存
enum FadeShape { Linear, EqualPower }
```

不変条件 (validate_references + op validate で二重に強制):

- region は comp 内で `region_ids` 順 = range 昇順・非重複・
  `group.target_range` ⊆・同一 project domain。
- transition は同一 comp の**隣接** region 対のみ (順序上の連続ペア)、
  `left.end == right.start`、overlap が boundary を**内部に**含み、
  `[left.start, right.end]` 内、comp 内 overlap 同士は非重複。
- candidate: Take は `group.take_ids` 所属・`[source_offset,
  offset+consumed)` ⊆ take coverage (= segment source_range union);
  ExistingSource は source 存在・consumed ≤ window; Silence は常に可。
- `source_rate` で project media 長→source frame を換算 (MD-A 単一
  レート近似)。

## CompOp (project-domain Operation)

`RegisterTakeGroup` / `UnregisterTakeGroup` (comp 残存時は拒否) /
`CreateComp` / `RemoveComp` (cascade: region/transition + active 解除) /
`SetActiveComp` / `AddCompRegion` / `SetCompRegionCandidate` (採用替え) /
`SetCompRegionRange` (transition 付着時は拒否 — 先に transition 調整) /
`RemoveCompRegion` (付着 transition も原子削除) / `AddCompTransition` /
`SetCompTransition` / `RemoveCompTransition`。

全て `Command<CompOp>` で `ProjectController` 経由。undo は controller の
whole-project snapshot が機械的にカバー。

## Effective Comp 投影

`project_comp(&Project, CompId) -> Result<EffectiveComp>`:

```rust
struct EffectiveComp { comp_id, segments: Vec<EffectiveSegment>,
                       transitions: Vec<EffectiveTransition> }
struct EffectiveSegment { region_id, project_range,
    binding: Option<SourceBinding>,   // None = Silence
    provenance: CompProvenance }      // Take(id)|ExistingSource(id)|Silence
struct SourceBinding { source_id, window: SourceRange, mapping: PlaybackMapping }
struct EffectiveTransition { transition_id, boundary: ProjectTime,
    overlap_range, fade_out, fade_in,
    left: Option<SourceBinding>,  // left region の延長 handle
    right: Option<SourceBinding> } // right region の先行 handle
```

- 再現可能: 同一 Project → 同一 EffectiveComp (BTreeMap 順序・
  region_ids 順で決定的)。
- Source 不変: 投影は `&Project` 読み取りのみ。
- 位置問合せ: `segment_at(pos)` は高々 1 件 (非重複の構造保証)、
  `transition_at(pos)` も高々 1 件 — 境界の一意帰属を型で表現。
- Overlap 中は left の post-handle (boundary→overlap.end) と right の
  pre-handle (overlap.start→boundary) がそれぞれ binding として導出
  される = 「限定された重複再生」。

## Editor target 型 (§15.2)

```rust
enum EditorDisplayTarget { EffectiveComp(CompId), TakeSource(TakeId),
                           Compare(CompId) }
enum EditorEditingTarget { Comp(CompId),
                           EditableSource(SourceVersionRef) }
struct SourceVersionRef { source_id, version: u64 } // version>=1 = derived
```

- `SourceVersionRef::new` は version 0 (Raw Capture) を拒否 —
  「生 Capture の直接編集」を型で不可能にする。
- Display/Editing binding は View state (project mutation ではない
  §15.7) — MD-A は型契約と検証のみ。

## Consumer 適合

- `synvelis-recording::commit`: `ClipEntry`(manifest 不変) →
  `placement{Musical(tick_in), Musical(tick_out-tick_in)}` +
  `playback{[source_offset,source_end), Natural}` を生成。整合検査は
  同一値のまま。
- `synvelis-player::lowering`: `clip.placement+playback` を `resolve`
  で解決 (旧 anchor と同一の media span を Natural で再現)。Loop/
  Stretch は `SkippedClip::UnsupportedMapping` で明示 skip。
- `SCHEMA_VERSION = 2`、新 registry/map に `#[serde(default)]`。

## Deferred / capability gaps

- take project placement、per-source native rate、Grouped Comp、
  Comp MIDI Phrase、Warp、fade パラメータ実値、lowering の
  Loop/Stretch 実行、synvelis-workspace 連携 — 各々後続 Issue。
