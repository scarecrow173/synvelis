---
id: "SPEC-034QMZiCjhKFlSgoIw9YaX"
type: "spec"
status: "approved"
title: "ED-B media-domain move + slip commit semantics (workspace editing PoC)"
created: "2026-11-03"
updated: "2026-11-03"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/65"
    - "https://github.com/scarecrow173/synvelis/issues/38"
    - "https://github.com/scarecrow173/synvelis/issues/11"
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
    - "docs/specs/ed-a-editing-model-selection-tool-snap-transaction-linkedcontext.md"
    - "docs/specs/mda-takegroup-comp-effective-projection-placement-contract.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/adr/ed-b-media-domain-gesture-positions-slip-trim-marquee-commit-semantics.md"
    - "docs/designs/ed-b-media-domain-editing-design.md"
    - "docs/specs/ed-a-editing-model-selection-tool-snap-transaction-linkedcontext.md"
    - "docs/specs/mda-takegroup-comp-effective-projection-placement-contract.md"
  refines: []
  refined-by: []
  derives-from:
    - "https://github.com/scarecrow173/synvelis/issues/11"
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# ED-B media-domain move + slip commit semantics (workspace editing PoC)

## What Should Be Built

Issue #65: ED-A (issue #38) の workspace editing PoC が `UnsupportedOperation`
として残していた headless 残件を、MD-A (issue #42) が着地させた
`ClipPlacement`/`ClipPlayback` 契約の上に実装する。`TimeAnchor::Absolute`
clip の move gesture → `ProjectOp::MoveClip { anchor: Absolute(..) }`、
Slip-style content move → `ProjectOp::SetClipPlayback`、混在 domain clip
への各操作の明示的結果 (成功 or 定義済みエラー)、および media-domain
marquee selection を `synvelis-workspace` の gesture → commit 経路に通す。

## Why Now

ED-A は編集モデルの 5 契約 (Selection/Tool/Snap/Transaction/Cross-View)
を固定したが、commit 経路は musical domain のみで、media-anchored clip の
move/trim は domain 分離のため `UnsupportedOperation`、slip は
「tick→source-frame rate mapping 未 model」として拒否していた。MD-A で
`TimeAnchor`/`ClipExtent`/`ClipPlayback`/`PlaybackMapping` の正規化契約が
project model に着地したため、media domain の gesture commit 意味論を
headless で固定できる状態になった。real-GUI lane へ渡す前に、media
domain の編集結果を契約として test 固定しておく必要がある。

## Users and Value

- DAW ユーザー: Absolute (media-time) anchor の clip (録音済み audio、
  tempo map 非追従の配置) が musical clip と同じ編集原則で
  move/trim/slip できる。意味の曖昧な編集は推測されず、明示的に拒否
  または定義済み変換として処理される。
- 開発者: slip の commit 意味が `PlaybackMapping`
  (Natural/Loop/SimpleStretch) 別に契約化され、混在 domain の扱いが
  「静黙変換なし」原則で固定される。後続の real-GUI・audio editor
  lane の基礎になる。

## Scope

### In Scope

1. **media-domain move**: `TimeAnchor::Absolute` clip の move gesture →
   `ProjectOp::MoveClip { anchor: Absolute(..) }`。gesture の
   `EditTimeDomain::Media` 経路を `build_commit` まで通す。
   `extent`/`playback` は不変 (`MoveClip` 契約どおり)。
2. **media-domain trim**: `Absolute` anchor + `Absolute` extent clip の
   Start/End trim → `MoveClip`/`SetClipExtent` commit。anchor 側 trim は
   anchor 更新と extent 短縮を `Composite` で atomic に行う (musical 経路
   と同構造)。
3. **Slip-style content move**: 固定 extent 下での `source_region` 移動 →
   `ProjectOp::SetClipPlayback` commit。`PlaybackMapping` 別の契約:
   - `Natural`: source window は media delta と 1:1 で移動。
   - `Loop`: loop する source span 自体が media delta と 1:1 で移動
     (repeat 対象の window を translate)。
   - `SimpleStretch(r)`: source window は `r` 倍に scale された source
     frame 数で移動 (media 1 frame = source r frames)。
   - anchor/extent は不変 (`SetClipPlayback` 契約どおり)。
4. **混在 domain clip の明示的結果**: `Musical` anchor × `Absolute`
   extent、または `Absolute` anchor × `Musical` extent の clip に対する
   move/trim/slip の結果を個別に明示する (定義済み変換 or
   `UnsupportedOperation` 等の定義済みエラー)。「静かに変換しない」
   原則 — 未宣言の domain/base 間変換を行わない。
5. **media-domain marquee**: `Media` domain の marquee gesture が
   `Absolute` anchor + `Absolute` extent clip を media span overlap で
   選択する。`TimeSelection::Media` 等の既存 media 型と整合させる。
6. **probe/gesture の時間型**: media domain の gesture 位置を domain 明示
   の型で表現する (musical position と media position を型で区別し、
   probe が報告する domain と anchor 由来の gesture domain の不整合を
   gesture 開始時に明示的に拒否する)。

### Out of Scope

- 実 GUI / px hit test / drag 閾値 / hover / pen/touch / OS keybinding /
  AutoScroll (ED-A と同じく headless 意味 probe のみ)。
- Musical anchor + Musical extent clip の slip (tick→source-frame の
  declared rate を持たず、rate mapping 未 model — 定義済み拒否とする)。
- `SplitClip` commit (caller 供給 `clip_b_source_offset` の rate mapping
  問題は本件でも未 model)、`DrawClip`/`StretchClip`/`FadeClip` commit。
- Take/Comp lane の slip・note/automation entity 編集・ripple delete。
- Source 長の上限検証 (model に source length が存在しない。
  `source_region` 末尾超過は検出不可 — 既知の制限として記録)。
- ruviform 側変更。

## Requirements

- ED-A の契約を維持する: preview は project/undo/engine を不変、commit は
  全件検証 → 1 回の atomic apply = 1 undo、stale revision 拒否、
  cancel 経路で project 不変、primary のみ snap・相対 offset 保持。
- 時間演算は checked 演算のみ (`checked_add`/`checked_sub`/`try_from`/
  i128 rational)。`Tick`/`TickDuration` の unchecked `Add`/`Sub` 不使用。
- media 時間の base 変換は exact rational (i128) で行い、最終境界で
  floor する — `MediaTimeValue::frames_at` が確立した既存の丸め規約を
  そのまま使い、新しい丸め規約を持ち込まない。変換経路はコード/docs
  に明記し、暗黙の単位混同をしない。
- gesture 位置は domain 明示の型を通す。musical tick を media 単位として
  解釈する (または逆) のような untagged scalar の使い回しをしない。
- mixed-domain clip の各操作結果は成功・定義済み変換・定義済み拒否の
  いずれかとして明示的に返す。未定義のまま通さない。
- Project 変更は `ProjectController` 経由のみ。複数対象の media move は
  `Composite` で 1 atomic apply。

## Acceptance Criteria

- [x] **ED-B-01**: `TimeAnchor::Absolute` clip の body drag → commit が
  `MoveClip { anchor: Absolute(..) }` を発行し、apply 後に
  `placement.anchor` が media delta 分だけ更新される。`extent`・
  `playback`・`muted`・lane 帰属は不変。複数 media clip の一斉 move は
  `Composite` 1 apply で relative offset 保持・1 undo で全件復旧。
- [x] **ED-B-02**: `Absolute` anchor + `Absolute` extent clip の trim が
  commit される。StartTrim は anchor 前進 + extent 短縮 (`Composite`)、
  EndTrim は extent 伸長で、clip 末端 (media time) が変わる。extent が
  0 以下になる commit は検証拒否。
- [x] **ED-B-03**: Slip gesture → `SetClipPlayback` commit で
  `source_region` が移動し、`anchor`/`extent`/`mapping` は不変。
  mapping 別の移動量契約 (Natural/Loop = 1:1 media frame、
  SimpleStretch = ratio × media frame) が test で固定される。slip で
  `source_region.start < 0` になる commit は `Negative` 系エラーで拒否。
- [x] **ED-B-04**: 混在 domain clip (Musical anchor × Absolute extent /
  Absolute anchor × Musical extent) に対し、move/trim/slip の各結果が
  明示的 (commit 成功・定義済み変換・または `UnsupportedOperation` 等の
  定義済みエラー) である。結果が黙って別 domain へ変換される経路は
  存在しない。
- [x] **ED-B-05**: `Media` domain の marquee が `Absolute` anchor +
  `Absolute` extent clip を media span overlap で選択し、musical clip・
  混在 clip は選択しない。`Musical` domain の marquee 動作は不変。
- [x] **ED-B-06**: probe が報告する時間 domain と gesture の fixated
  domain が不整合な場合、gesture は開始時に明示拒否される
  (musical position の media gesture 化、またはその逆は起きない)。
- [x] **ED-B-07**: `mise run fmt-check`/`check`/`lint`/`test` 全 green。
  impl record に mapping 別 slip 契約・混在 domain 結果表・未検証項目を
  正直に記録。

## Deferred Design Concerns

- Musical anchor + Musical extent clip の slip 意味 | tick→source-frame
  rate binding が model にない | source rate binding の model 化 |
  現状は定義済み拒否で安全側。
- `SplitClip` commit の source offset 解決 | 同上 (rate mapping 未
  model) | 同 model 化 | ED-A からの継続 deferred。
- `DrawClip`/`StretchClip`/`FadeClip` の commit 意味 | source binding /
  stretch ratio / fade model が gesture probe にない | 個別 domain
  algorithm 確定後 | 同 deferred。
- media snap 候補の発生源 (他 clip edge 列挙・playhead の media 表示) |
  view 実装層の `SnapEnvironment` 供給責務 | real-GUI lane | headless は
  caller 供給候補のみ。
- 異なる declared base を持つ media clip 群の一斉 move の丸め累積 |
  各 clip が自 base で floor する規約 | 高精度編集要求が出た時点 |
  sub-base-unit 未満の相対ズレ (規約明記済み)。

## Sources and Relations

- Issue #65 本文 (スコープ・受入条件 ED-B-*)
- Issue #38 / #11 (ED-A・編集モデル合意。混在 domain は #11 §9 deferred)
- `docs/specs/ed-a-editing-model-selection-tool-snap-transaction-linkedcontext.md`
  (ED-A spec・残件の出所)
- `docs/specs/mda-takegroup-comp-effective-projection-placement-contract.md`
  (MD-A placement/playback 契約)
- `docs/impl/ir/ed-a-editing-model-headless-poc.md` (ED-A impl record の
  未検証・follow-up 節)
- `docs/impl/ir/mda-takegroup-comp-effective-projection-placement.md`
  (MD-A impl record)
