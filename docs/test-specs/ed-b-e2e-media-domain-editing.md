---
id: "TSPEC-034QN1oYnTtpV7t5CPcc5e"
type: "test-spec"
status: "approved"
title: "ED-B e2e media-domain editing acceptance"
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
  implements: []
  implemented-by: []
  depends-on:
    - "docs/specs/ed-b-media-domain-move-slip-commit-semantics.md"
    - "docs/designs/ed-b-media-domain-editing-design.md"
    - "docs/plans/ed-b-media-domain-editing.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/ed-b-media-domain-move-slip-commit-semantics.md"
    - "docs/designs/ed-b-media-domain-editing-design.md"
  derived-by: []
  verifies:
    - "docs/specs/ed-b-media-domain-move-slip-commit-semantics.md"
    - "docs/designs/ed-b-media-domain-editing-design.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# ED-B e2e media-domain editing acceptance

## Purpose

issue #65 の受入条件 ED-B-01..07 が `synvelis-workspace` の gesture →
commit 経路で成立することを、`tests/ed_b.rs` の headless e2e test で
固定する。media-domain 編集意味の回帰防止が存在意義。

## Feature

media-anchored clip の move/trim、Slip による `source_region` 移動、
混在 domain clip の明示的結果、media-domain marquee。

## Rules

- ED-B-01: `MoveClip` は Absolute anchor のみ更新し extent/playback を
  不変に保つ。複数 media clip は `Composite` で 1 undo。
- ED-B-02: Absolute×Absolute trim は `StartTrim` が anchor+extent の
  `Composite`、`EndTrim` が `SetClipExtent`。
- ED-B-03: slip は `SetClipPlayback` で `source_region` のみ移動。
  Natural/Loop は 1:1、SimpleStretch は ratio 倍。負転落は拒否。
- ED-B-04: 混在 clip の各操作は設計の結果表どおり (commit or 定義済み
  拒否)。黙った domain 変換経路は存在しない。
- ED-B-05: media marquee は Absolute×Absolute clip のみ media span
  overlap で選択。musical marquee 動作は不変。
- ED-B-06: probe position と gesture domain の不整合は開始時拒否。
- ED-B-07: 全 mise 検証 PASS。

## Examples

- media move: anchor `{frames: 48000, base: 48000}` clip を
  `Media({frames: 24000, base: 48000})` 位置へ body drag → anchor が
  `{frames: 72000, base: 48000}` になる。
- slip: `Natural`・`source_region = 100..200` の clip を +1000 media
  frames (base 48000) slip → `source_region.start` が 1:1 で増加。
- 混在 slip: Musical anchor × Absolute extent clip は tempo map 経由の
  定義済み変換で `source_region` が動き、Absolute anchor × Musical
  extent clip は `UnsupportedOperation`。
- media marquee: `Media` span `0..96000@48000` で `Absolute×Absolute`
  clip のみ選択。

## Guarantees

- `MoveClip`/`SetClipPlayback` の不変条件 (extent・playback・anchor・
  mapping の保存) が apply 後の project state で検証される。
- 拒否パスは `WorkspaceError` の定義済み variant で返り、project は
  不変 (atomicity)。

## Non-goals

- 実 GUI・px 変換・device 挙動・audio DSP。
- fully-musical clip の slip 意味 (未 model・定義済み拒否を確認)。
- SplitClip/DrawClip/StretchClip/FadeClip commit。
