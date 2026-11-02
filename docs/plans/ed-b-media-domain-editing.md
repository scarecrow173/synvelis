---
id: "PLAN-034QN1oz8zutDLafJy6oDg"
type: "plan"
status: "approved"
title: "ED-B media-domain editing PoC 実装計画"
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
    - "docs/designs/ed-b-media-domain-editing-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/ed-b-media-domain-editing-design.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/ed-b-e2e-media-domain-editing.md"
  references: []
  defers: []
  deferred-by: []
---

# ED-B media-domain editing PoC 実装計画 Implementation Plan

## Goal

`EditTimePoint` の domain-tagged 一般化を土台に、media-domain move/
trim/slip/marquee の commit 経路と混在 domain の明示的結果を
`synvelis-workspace` に実装する (design 確定済み・単一 stream)。

## Scope

設計の混在 domain 結果表 (Musical×Musical / Absolute×Absolute /
Musical×Absolute / Absolute×Musical × move/trim/slip) と media marquee
を一つの変更 stream として実装する。`synvelis-project` 側は変更なし。

## File Map

- `crates/synvelis-workspace/src/selection.rs`: `EditTimePoint` enum +
  `domain()`/`value()`/`delta_to()` を新設。
- `crates/synvelis-workspace/src/tool.rs`: `HitProbe.position` を
  `EditTimePoint` 化。
- `crates/synvelis-workspace/src/gesture.rs`: `origin`/`position` を
  `EditTimePoint` 化、`drag_update` 引数を `EditTimePoint` 化。
- `crates/synvelis-workspace/src/snap.rs`: candidate/winner/`SnapUnit`/
  `resolve_snap` の position を i64 scalar 化。
- `crates/synvelis-workspace/src/workspace.rs`: `pointer_down` で probe
  domain を fixation domain に照合・不一致は開始時拒否、target-free op
  (Marquee/DrawClip) は probe domain を採用、`drag_update` の delta を
  origin-base へ exact rational floor で換算。
- `crates/synvelis-workspace/src/edit.rs`: `move_ops`/`trim_ops` に
  `Absolute` 分岐、`slip_ops` 新設 (declared rate・mapping 別
  `consumed_len`)、`marquee_ops` に `Media` domain 分岐。
- `crates/synvelis-workspace/tests/ed_b.rs`: 新規受入 test。
- `crates/synvelis-workspace/tests/ed_a.rs`: `HitProbe`/`drag_update` の
  API 追従。
- `crates/synvelis-workspace/src/lib.rs`: `EditTimePoint` export。

## Tasks

1. `ed-b-task-media-commit.md`: `EditTimePoint` + probe/gesture/snap 型
   追従 + move/trim/slip/marquee commit 経路 + ed_a test 追従。
2. `ed-b-task-acceptance-tests.md`: `tests/ed_b.rs` の ED-B-01..07
   受入 test。

## Risks

- ED-A test API の破壊的変更 → ed_a.rs を同 commit で追従。
- declared base 相違の一斉 move 丸め → 規約明記 (sub-base-unit ズレ許容)。
- `slip` の declared rate は extent denominator 依存 → `Musical` extent
  は定義済み拒否。

## Verification

- `mise run fmt-check` / `check` / `lint` / `test` 全 PASS。
- `docs/test-specs/ed-b-e2e-media-domain-editing.md` の test 全緑。
- ED-A test が追従後に全 PASS (musical 経路の非回帰)。

## Rollback

feature branch 上の差分 revert。`synvelis-project` 無変更のため
project 層への影響なし。`SnapUnit`/`WORKSPACE_SCHEMA` 不変で persisted
state の移行不要。
