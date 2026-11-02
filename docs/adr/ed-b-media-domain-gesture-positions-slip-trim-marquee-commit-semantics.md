---
id: "ADR-034QN06oqUD5VvvsC9aedM"
type: "adr"
status: "accepted"
title: "ED-B media-domain gesture positions + slip/trim/marquee commit semantics"
created: "2026-11-03"
updated: "2026-11-03"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/65"
    - "https://github.com/scarecrow173/synvelis/issues/38"
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
    - "docs/adr/ed-a-editing-model-placement-synvelis-workspace-gesture-tool-snap-state-machines.md"
    - "docs/adr/mda-normalized-placement-comp-schema.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/ed-b-media-domain-move-slip-commit-semantics.md"
    - "docs/designs/ed-b-media-domain-editing-design.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/ed-b-media-domain-move-slip-commit-semantics.md"
    - "docs/adr/ed-a-editing-model-placement-synvelis-workspace-gesture-tool-snap-state-machines.md"
    - "docs/adr/mda-normalized-placement-comp-schema.md"
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
metadata:
  adr:
    decision-makers: []
    consulted: []
    informed: []
---

# ED-B media-domain gesture positions + slip/trim/marquee commit semantics

## Context and Problem Statement

ED-A の gesture 経路は位置を `MusicalTick` で固定しており、commit 経路は
musical domain のみだった。issue #65 は `TimeAnchor::Absolute` clip の
move/trim、`SlipClip` の `SetClipPlayback` commit、混在 domain clip の
明示的結果、media-domain marquee を要求する。中心の設計問題は 2 つ:
(a) media domain の gesture 位置を型としてどう表すか、(b) domain/base の
異なる時間値を commit 単位へ落とす変換をどう規約化するか。

## Decision Drivers

* 「静かに変換しない」原則 (#11 §9-4・issue #65): domain 混在を型で
  表面化し、未宣言変換を禁止。
* `MediaTimeValue::frames_at` が確立した丸め規約 (exact rational →
  final boundary floor) を再利用し、新しい丸め規約を増やさない。
* `ClipPlayback.source_region` は `SourceFrame` (source-native、非負)
  であり、declared source rate は `ClipExtent::Absolute` の denominator
  が担うという `validate_clip_contract` の既存規約。
* ED-A の境界: gesture/snap/preview は `synvelis-workspace` の session
  state、commit/undo/検証は `synvelis-project` の `ProjectController`。
* checked 演算のみ (AGENTS.md lint 規約)。

## Considered Options

* Option 1: gesture/probe 位置を domain-tagged enum
  (`EditTimePoint::Musical | EditTimePoint::Media`) に一般化し、delta は
  fixation origin の declared base の scalar (i64) とする。base 変換は
  commit/update 時に exact rational floor で行う。
* Option 2: `MusicalTick` を bare scalar のまま media domain でも使い回す
  (「media のときは単位が違う」規約)。
* Option 3: 全位置を `MediaTimeValue` に統一し、musical 位置も media に
  正規化して保持する。

## Decision Outcome

Chosen option: "Option 1", because domain tag 付きの型は「何の単位か」を
呼び出し側の規約ではなく値自身が担い、probe domain と fixated gesture
domain の不整合を `pointer_down` で明示拒否できる。Option 2 は musical
tick を media 単位として読む silent reinterpretation を型で防げない。
Option 3 は musical→media の tempo map 依存変換を gesture 層に蔓延させ、
「anchor が権威」の MD-A 契約と衝突する。

### Consequences

* Good, because `HitProbe`/`GestureFixation.origin`/`GesturePreview.position`
  が domain を型で報告し、`build_commit` の match が domain 別の意味を
  明示的に分岐できる。
* Good, because delta が origin-base の scalar のままなら snap の既存
  engine (i64 距離/hysteresis/priority) を domain filter だけで再利用
  できる (snap candidate は caller が gesture と同じ declared base で
  供給する契約)。
* Bad, because `HitProbe`/`drag_update` の signature が変わり、ED-A test
  API に破壊的変更が入る (PoC 内の意図的変更として記録)。
* Bad, because declared base の異なる複数 media clip の一斉 move は各
  clip の base で別々に floor し、sub-base-unit の相対ズレが起きうる
  (規約として明記)。

### Confirmation

* `pointer_down` が probe position の domain と target clip の anchor
  domain の不一致を拒否することを ED-B-06 test で確認。
* `MoveClip`/`SetClipPlayback` commit の anchor/extent/playback 不変条件を
  ED-B-01/03 test で確認。
* `mise run test` で全経路 (media move/trim/slip/混在/marquee) を確認。

## Implementation Plan

* Affected paths: `crates/synvelis-workspace/src/{selection,tool,gesture,
  snap,edit,workspace}.rs`, `crates/synvelis-workspace/tests/{ed_a,ed_b}.rs`
  (ed_b 新規・ed_a 追従)。`synvelis-project` 側は MD-A 契約のまま変更なし。
* Patterns to follow: `MediaTimeValue::frames_at` の floor 規約、
  `Composite` による atomic multi-op、unsupported を値で返す ED-A の
  commit 表。
* Patterns to avoid: untagged scalar の domain 横断使用、tempo map を使わ
  ない tick→source-frame 推測、declared rate のない clip への変換。
* Dependencies or configuration: 新 dep なし。

## Verification

* [ ] `mise run fmt-check` / `check` / `lint` / `test` 全 PASS。
* [ ] ED-B-01..07 の headless 受入 test が存在し PASS。
* [ ] `synvelis-project` の op/schema 変更がないこと。
* [ ] impl record に混在 domain 結果表・未検証項目が正直に記録される。

## Pros and Cons of the Options

### Option 1

* Good, because domain が値の一部になり、不一致は boundary で拒否できる。
* Bad, because gesture/probe/snap の型変更が広がる。

### Option 2

* Good, because diff が最小。
* Bad, because 「media のときは tick ではない」を型が表さず、
  silent conversion の温床になる。

### Option 3

* Good, because 内部表現が一つに統一される。
* Bad, because musical anchor の権威を media 変換に通す必要があり、
  tempo map 不在の headless gesture で扱えない場合が出る。

## More Information

- 上位 spec: `docs/specs/ed-b-media-domain-move-slip-commit-semantics.md`
- 設計: `docs/designs/ed-b-media-domain-editing-design.md`
- 前提 ADR: `docs/adr/ed-a-editing-model-placement-synvelis-workspace-gesture-tool-snap-state-machines.md`,
  `docs/adr/mda-normalized-placement-comp-schema.md`
