---
id: "IMPL-034QO4tHWa37Uyya3N6ttQ"
type: "impl"
status: "completed"
title: "ED-B media-domain move/trim/slip + marquee commit semantics"
created: "2026-11-02"
updated: "2026-11-02"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/65"
    - "https://github.com/scarecrow173/synvelis/issues/11"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-workspace/tests/ed_b.rs"
      - type: "file"
        path: "docs/specs/ed-b-media-domain-move-slip-commit-semantics.md"
      - type: "file"
        path: "docs/adr/ed-b-media-domain-gesture-positions-slip-trim-marquee-commit-semantics.md"
      - type: "file"
        path: "docs/designs/ed-b-media-domain-editing-design.md"
      - type: "file"
        path: "docs/plans/ed-b-media-domain-editing.md"
      - type: "file"
        path: "docs/tasks/ed-b-task-media-commit.md"
      - type: "file"
        path: "docs/tasks/ed-b-task-acceptance-tests.md"
      - type: "file"
        path: "docs/test-specs/ed-b-e2e-media-domain-editing.md"
      - type: "file"
        path: "docs/impl/ir/ed-b-media-domain-move-trim-slip-marquee-commit-semantics.md"
    modified:
      - type: "file"
        path: "crates/synvelis-workspace/src/edit.rs"
      - type: "file"
        path: "crates/synvelis-workspace/src/selection.rs"
      - type: "file"
        path: "crates/synvelis-workspace/src/gesture.rs"
      - type: "file"
        path: "crates/synvelis-workspace/src/snap.rs"
      - type: "file"
        path: "crates/synvelis-workspace/src/tool.rs"
      - type: "file"
        path: "crates/synvelis-workspace/src/workspace.rs"
      - type: "file"
        path: "crates/synvelis-workspace/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-workspace/tests/ed_a.rs"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/ed-b-media-domain-editing.md"
    - "docs/tasks/ed-b-task-media-commit.md"
    - "docs/tasks/ed-b-task-acceptance-tests.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/adr/ed-b-media-domain-gesture-positions-slip-trim-marquee-commit-semantics.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/ed-b-media-domain-move-slip-commit-semantics.md"
    - "docs/designs/ed-b-media-domain-editing-design.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/ed-b-e2e-media-domain-editing.md"
  references: []
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# ED-B media-domain move/trim/slip + marquee commit semantics

## Summary

Issue #65 — ED-A (#38) が `UnsupportedOperation` として拒否していた
media-anchored clip の編集を、MD-A (#42, merge 2cb5010) の
`ClipPlacement`/`ClipPlayback` 契約上に実装した。gesture 位置を
domain 付き `EditTimePoint` に一般化し、media-domain の
move/trim/slip commit 経路・mapping 別 slip 意味論・混在 domain
の明示的結果表・media-domain marquee を着地。受入条件
ED-B-01..07 を `tests/ed_b.rs` 20 件で検証した。

## Context

Spec: docs/specs/ed-b-media-domain-move-slip-commit-semantics.md
ADR: docs/adr/ed-b-media-domain-gesture-positions-slip-trim-marquee-commit-semantics.md
Design: docs/designs/ed-b-media-domain-editing-design.md
Plan: docs/plans/ed-b-media-domain-editing.md
Test spec: docs/test-specs/ed-b-e2e-media-domain-editing.md

## Implementation

### Changes

- `synvelis-workspace/src/selection.rs`: `EditTimePoint`
  (Musical|Media、domain タグ付き位置)、`delta_since` (origin の
  宣言 base への signed delta、media は exact rational rescale)、
  `media_rescale`/`floor_div_i128` (i128 exact rational、最終境界で
  1 回だけ floor、`None` on overflow)。
- `synvelis-workspace/src/gesture.rs`: `GestureFixation.origin` /
  `GesturePreview.position` を `EditTimePoint` 化。
- `synvelis-workspace/src/snap.rs`: snap candidate/winner/auto_step
  を gesture 宣言 base の `i64` scalar 化 (domain フィルタは
  `EditTimeDomain`)。
- `synvelis-workspace/src/tool.rs`: `HitProbe.position` を
  `EditTimePoint` 化。
- `synvelis-workspace/src/workspace.rs`: `pointer_down` で
  clip-targeted op の probe/fixation domain 不整合を
  `MixedTimeDomains` で開始時拒否 (ED-B-06)。clip を持たない
  target 集合は probe domain を採用し `fixation.domain ==
  origin.domain()` を構造的に保証。`drag_update` は
  `delta_since` + `primary_snap_position` (EndTrim は anchor では
  なく drag される end edge を snap) に一般化。
- `synvelis-workspace/src/edit.rs`: `move_ops` に Absolute anchor
  経路 (gesture delta を clip 各自の宣言 base へ rescale、複数
  clip は 1 `Composite` = 1 undo)。`media_trim_op` (Absolute×
  Absolute、`StartTrim` = `MoveClip`+`SetClipExtent` の atomic
  composite)。`slip_op` → `SetClipPlayback` (extent 宣言
  denominator を declared source rate として delta を media frame
  化 → mapping の `consumed_len` で source shift)。media marquee
  は `media_clip_overlaps` の exact rational overlap。
- `synvelis-workspace/src/lib.rs`: `EditTimePoint` 再 export。
- `tests/ed_b.rs`: ED-B-01..07 受入 test 20 件。
- `tests/ed_a.rs`: 新 API へ移行 (挙動不変、6 件全 green)。

### Slip mapping 別意味論 (ED-B-03)

| mapping | source shift | 備考 |
| --- | --- | --- |
| `Natural` | media delta を 1:1 で source frame 化 | window は extent 以上必須 (MD-A 契約) |
| `Loop` | media delta を 1:1 で looped window ごと平行移動 | window は非空のみ必須 |
| `SimpleStretch(p/q)` | media delta × `p/q` を source frame 化 | `consumed_len` が比率を適用 |

符号は `consumed_len(|delta|)` の符号反転で往復対称。start < 0
となる slip は `Error::Negative` で defined rejection (clamp
しない)。`Musical` extent は declared rate を持たないため slip は
`UnsupportedOperation` 拒否。

### 混在 domain 結果表 (ED-B-04)

| anchor × extent | move | trim | slip |
| --- | --- | --- | --- |
| Musical × Musical | commit (tick delta) | commit (ED-A 経路) | reject (rate なし) |
| Absolute × Absolute | commit (base rescale) | commit | commit |
| Musical × Absolute | commit | reject (`UnsupportedOperation`) | commit (tempo map 変換) |
| Absolute × Musical | commit | reject (`UnsupportedOperation`) | reject (rate なし) |

probe domain ≠ anchor domain は `pointer_down` 開始時に
`MixedTimeDomains` 拒否 (ED-B-06)。静黙変換なし。

### Decisions During Implementation

- **`StartTrim` の extent は単一 floor で計算**: review で
  `floor(end) − floor(new_anchor)` は真値 `floor(end − new_anchor)`
  より最大 1 extent unit 大きく「固定端」を越え得る (かつ本来
  reject すべき extent 0 を commit してしまう) 欠陥を確認。
  `floor(extent − applied_delta)` を extent 宣言 base で 1 式
  評価する形に修正 — commit される end は固定端を超えず、最大
  1 extent unit 手前に収まる (declared base で表現可能な最近値)。
- **media marquee の overlap は exact rational 比較**: end を
  anchor base に floor して `MediaSpan` 化すると真の overlap を
  false-negative で落とす欠陥を確認。商/余り比較で
  `lo < anchor + extent` を変換なしに評価する
  `media_clip_overlaps` に置換 (共有する `extent.numerator`
  factor を remainder 積から約分し、既存 `lt` と同じ i128
  bound に収めた)。i128 中間値が溢れる極端入力のみ
  `Error::Overflow` の defined rejection。
- **`EndTrim` の snap は動く端を基準に**: 従来 (ED-A 由来) は
  常に anchor start を snap 基準にしており、EndTrim で phantom
  snap になる欠陥を修正 (musical 経路にも同じ修正が効く)。
- **`media_rescale` は `Option` 返し**: silent saturation から
  checked contract に整合するよう変更 (呼出側は `Overflow`
  伝搬または `None` = snap/no-match 扱い)。
- **trim は playback (`source_region`) を変えない**: design が
  規定する 2-op composite どおり。`StartTrim` で content は
  anchor と共にずれる (ED-A musical trim と同一意味論)。
  content-fixed trim (DAW 的な slip+trim 合成) を望むなら
  `SetClipPlayback` を composite に加える別 issue の設計変更。

## Related Experiments

### Adopted

なし。

### Rejected

なし。

## Validation

`git merge origin/main` (Already up to date) 後に実施:

- `mise run fmt` / `mise run fmt-check` — PASS。
- `mise run check` (`cargo check --workspace --all-targets`) — PASS。
- `mise run lint` (`cargo clippy --workspace --all-targets --
  -D warnings`) — PASS (pre-existing の `pc_b_rt` fixture
  too_many_lines は `#[expect]` 付与で解消 — origin/main 上でも
  失敗していた repo 状態の修正)。
- `mise run test` (`cargo test --workspace --locked`) — 全 crate
  green。`tests/ed_b.rs` 20 件 / `tests/ed_a.rs` 6 件を含む。
- `doc_status audit`: ED-B chain の finding は 0 (残る 69 件は
  vs-c 系など pre-existing repo findings)。index は
  `doc_maintenance apply --force-index` で再生成済み。
- review-protocol: reviewer A (spec 適合) / B (敵対) を dispatch、
  CONFIRMED P1/P2 を全て修正 (StartTrim 二重 floor・EndTrim snap
  基準・marquee exact overlap・media_rescale saturation)。

## Risks / 未検証項目

- **source 実長の上限は未検証**: slip/trim が source 実長を
  超える場合の挙動は headless PoC では source store が実長を
  持たないため契約上検出不能 (window 契約のみ検証)。
- **`SnapCandidate` は base を持たない**: caller 契約として
  「候補は gesture 宣言 base の scalar」を doc で規定済み。
  異なる base の候補を渡すと mis-snap する (型で不変条件を
  表現できない)。将来 base タグ付き candidate を検討。
- **i128 中間値 bound**: `value × u32 × u32` 積の和が
  `i128::MAX` を超える極端入力では `Overflow` defined
  rejection (panic しない)。`frames_at`/`lt` が受け入れる
  bound と同等。
- **GUI/device/px 経路は未検証**: headless PoC。`HitProbe` を
  生成する実 UI 層は対象外。
- **review 所見の残件**: trim が `source_region` を変えない
  意味論 (content slide) は spec 規定どおりだが、DAW 標準の
  content-fixed trim との差異は follow-up 候補。

## Follow-ups

- content-fixed `StartTrim` (composite に `SetClipPlayback` を
  含める) の要否を issue 起票して判断する。
- `SnapCandidate` に base を持たせる型レベルの強化を検討。
- source store が実長を持つ層で slip/trim の source-length
  上限検証を追加する。
