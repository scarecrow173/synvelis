---
id: "DESIGN-034QN0IIBPRdrHiOv3uTvq"
type: "design"
status: "approved"
title: "ED-B media-domain editing design"
created: "2026-11-03"
updated: "2026-11-03"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/65"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/ed-b-media-domain-move-slip-commit-semantics.md"
  implemented-by: []
  depends-on:
    - "docs/specs/ed-b-media-domain-move-slip-commit-semantics.md"
    - "docs/adr/ed-b-media-domain-gesture-positions-slip-trim-marquee-commit-semantics.md"
    - "docs/designs/ed-a-editing-model-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/ed-a-editing-model-design.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/ed-b-media-domain-move-slip-commit-semantics.md"
    - "docs/adr/ed-b-media-domain-gesture-positions-slip-trim-marquee-commit-semantics.md"
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# ED-B media-domain editing design

## Context

Issue #65: ED-A が残した media-domain commit 残件を、MD-A の
`ClipPlacement` (`TimeAnchor` + `ClipExtent`) / `ClipPlayback`
(`SourceRange` + `PlaybackMapping`) 契約の上に実装する詳細設計。
ADR Option 1 に従い、gesture/probe の位置を domain-tagged
`EditTimePoint` へ一般化し、commit 側で domain 別の意味を明示分岐する。

## Scope

### In Scope

- `selection.rs`: `EditTimePoint` 新設。
- `tool.rs`/`gesture.rs`/`snap.rs`/`workspace.rs`: 位置型の追従 +
  probe-domain 不整合の開始時拒否。
- `edit.rs`: media move/trim/slip/marquee の commit 経路。
- `tests/ed_b.rs`: ED-B-01..07 受入 test。`tests/ed_a.rs`: API 追従。

### Out of Scope

- `synvelis-project` 側の op/schema 変更 (MD-A 契約のまま)。
- fully-musical clip の slip、Split/Draw/Stretch/Fade commit、実 GUI。

## Architecture

### 時間位置の型

`EditTimePoint` — gesture 経路でやり取りする位置。`Musical(MusicalTick)`
または `Media(MediaTimeValue)`。`domain()`/`value()` (declared base の
raw scalar) を提供。`HitProbe.position`、`GestureFixation.origin`、
`GesturePreview.position`、`drag_update` の引数をこの型へ。

delta は fixation origin の declared base の i64 scalar (musical は
tick)。probe が別 declared base の `Media` position を供給した場合、
origin base へ exact rational (i128) floor で変換してから delta を取る
— `frames_at` と同じ丸め規約。

### probe/domain 不整合の拒否

`pointer_down` は resolve 後に `probe.position.domain()` と fixation
domain を照合する。対象 clip の anchor domain から導出される gesture
domain と probe が報告した domain が異なれば `MixedTimeDomains` 相当の
開始時拒否 (musical position を media gesture へ、またはその逆の
silent reinterpretation を防ぐ)。Marquee/DrawClip 等の target-free op は
probe の domain が gesture domain になる。

### snap の scalar 化

`SnapCandidate.position`/`SnapWinner.position`/`SnapUnit::Explicit`/
`SnapEnvironment.auto_step`/`resolve_snap` の position を i64 scalar に
一般化 (declared base の単位で比較)。candidate は `domain` で filter
済みのため跨 domain 比較は起きない。media domain では caller が gesture
と同一 declared base で候補を供給する契約。`SnapUnit` の wire 形式は
i64 scalar のまま (`WORKSPACE_SCHEMA` 不変)。

`primary_anchor_start`: media gesture では `Absolute` anchor の value を
fixation origin の declared base へ exact rational floor で写し、
primary のみ snap・co-mover に同一 delta を適用する規則は不変。

## Commit Semantics

### Move (`MoveClips`)

target ごとに `anchor += delta` を anchor 自身の domain/base で適用:

- `Musical` anchor: tick delta を `checked_add` (ED-A 経路と同一)。
- `Absolute` anchor: delta を origin base → anchor declared base へ
  exact rational floor で変換し `value` に加算。`MoveClip` は extent/
  playback を不変に保つ。混在 clip の move は anchor domain の更新のみで
  extent domain を跨がないため両 anchor kind で成立。

### Trim (`TrimClip`)

- `(Musical, Musical)`: ED-A 経路そのまま。
- `(Absolute, Absolute)`: media delta。`StartTrim` は `Composite`
  `{ MoveClip(anchor+δ_anchor), SetClipExtent(extent−δ_extent) }`、
  `EndTrim` は `SetClipExtent(extent+δ_extent)`。δ は各 declared base で
  exact rational floor。
- 混在 (anchor/extent domain が異なる): `UnsupportedOperation` の定義済み
  拒否 — 跨 domain trim は未宣言変換を要求するため黙って行わない。

### Slip (`SlipClip`)

`SetClipPlayback { playback: ClipPlayback { source_region: region+shift,
mapping: 不変 } }` を commit。`anchor`/`extent`/`mapping` は不変。

1. declared source rate `r` の確定: `ClipExtent::Absolute(d)` →
   `r = d.denominator()` (`validate_clip_contract` の既存規約)。
   `ClipExtent::Musical` → declared rate なし → 定義済み
   `UnsupportedOperation` (fully-musical clip の slip は未 model)。
2. media delta → source frame:
   - gesture domain `Media`: delta を `r` frames/sec で frame 化
     (exact rational floor)。
   - gesture domain `Musical` (混在 clip: Musical anchor × Absolute
     extent): `project.tick_time_mapping()` 経由で
     `tick_to_media(anchor+Δ) − tick_to_media(anchor)` を `r` base で
     求める定義済み変換。
3. source shift = `sign(Δ) × consumed_len(|Δ_frames|)`:
   `Natural`/`Loop` = Δ 1:1、`SimpleStretch(ratio)` = ratio × Δ。
4. `source_region` を shift で translate (`SourceFrame` checked、
   負転落は `Error::Negative` で拒否)。end 側の source 長上限は model
   になく検出不可 (既知の制限)。

### Marquee

- `Musical` domain: ED-A 経路 (Musical×Musical clip のみ選択)。
- `Media` domain: `MediaSpan` を origin/position から構成し、
  `(Absolute, Absolute)` clip の media span との overlap で選択
  (`MediaTimeValue::lt`/`ge` の exact rational 比較 — base 相違も厳密
  比較、変換しない)。混在 clip はどちらの domain の marquee 対象にも
  ならない。

## 混在 domain 結果表

| clip 形態 | move | trim | slip |
| --- | --- | --- | --- |
| Musical × Musical | tick delta commit | tick delta commit | 定義済み拒否 (rate 未 model) |
| Absolute × Absolute | media delta commit | media delta commit | source_region translate commit |
| Musical × Absolute | anchor domain で commit | 定義済み拒否 | tempo map 経由の定義済み変換 commit |
| Absolute × Musical | anchor domain で commit | 定義済み拒否 | 定義済み拒否 (declared rate なし) |

## Error Handling

- probe/gesture domain 不整合: `pointer_down` 開始時拒否。
- base/rate 未定義・未宣言変換要求: `WorkspaceError::UnsupportedOperation`
  または `MixedTimeDomains`。
- 時間演算の失敗: `synvelis_project::Error` (`Negative`/`Overflow`/
  `Validation`/`Inverted`) を `WorkspaceError::Project` 経由で返す。
- commit 前検証は既存の `validate_clip_contract` + scratch atomic apply
  に委譲 (全件失敗時は正本不変)。

## Testing Strategy

`crates/synvelis-workspace/tests/ed_b.rs` に ED-B-01..07 を対応する
受入 test を配置。fixture は `add_media_clip` (Absolute×Absolute)、
混在 clip 2 形態、mapping 3 種で構成し、`FakeWindowHost` +
semantic `HitProbe` の既存経路を使う。
