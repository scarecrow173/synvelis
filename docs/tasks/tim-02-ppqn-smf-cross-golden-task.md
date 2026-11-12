---
id: "TASK-TiM02PpqnSmfXg04"
type: "task"
status: "done"
title: "TIM-02 — PPQN×SMF cross-golden fixture（SetProjectPpqn × smf_documents 全鎖）"
created: "2026-11-12"
updated: "2026-11-12"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/126"
    - "https://github.com/scarecrow173/synvelis/issues/125"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/tim-02-ppqn-smf-cross-golden-plan.md"
  implemented-by:
    - "docs/impl/ir/tim-02-ppqn-smf-cross-golden.md"
  depends-on:
    - "docs/plans/tim-02-ppqn-smf-cross-golden-plan.md"
    - "docs/tasks/tim-02-task1-smf-core.md"
    - "docs/tasks/tim-01-configurable-ppqn.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/plans/tim-02-ppqn-smf-cross-golden-plan.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/tim-02-ppqn-smf-cross-golden.md"
  references:
    - "docs/specs/tim-02-ppqn-smf-cross-golden.md"
    - "docs/designs/tim-02-ppqn-smf-cross-golden-design.md"
  defers: []
  deferred-by: []
---

# TIM-02 — PPQN×SMF cross-golden fixture

## Work

Issue #126 受入行の交差を headless fixture で固定する。
`crates/synvelis-project/tests/tim_02_ppqn_smf.rs`（新規）
で Format 1 division-480 SMF を project PPQN 960 の
project に import し、SMF-backed source を読む musical
clip を anchor した上で `SetProjectPpqn` を Exact
（960→480）と Round（960→1000・inexact anchor で Exact
reject 確認）の両 policy で適用し、save/load を経て
export@480 の bytes が PPQN 変更・永続化を跨いで golden
一致すること、export→reparse→reimport で
`SmfDocument` が `PartialEq` 一致し tempo×2/CC64×3 の
provenance と note 射影が保持されることを検証する。
`ppqn.rs` の source-native skip 不変条件を end-to-end
で確認する側であり `src/` は変更しない。doc chain 6 件
を併せて作成する。

## Classification

- `normal-plan-task`

## Upstream Documents

- `docs/specs/tim-02-ppqn-smf-cross-golden.md`
- `docs/designs/tim-02-ppqn-smf-cross-golden-design.md`
- `docs/plans/tim-02-ppqn-smf-cross-golden-plan.md`
- `docs/tasks/tim-02-task1-smf-core.md`（SMF core・done）
- `docs/tasks/tim-01-configurable-ppqn.md`（PPQN Command・
  done）

## Dependencies

- `synvelis-project` 公開 API のみ消費
  （`parse_smf`/`export_smf`/`ProjectOp::ImportMidiSmf`/
  `SetProjectPpqn`/`AddClip`/`InMemoryStore` 等 — 変更
  不要の経路）。
- `ruviform_media_core::Rounding`（`Round` policy 引数）。

## Affected Files

- `crates/synvelis-project/tests/tim_02_ppqn_smf.rs` —
  新規 fixture 2 テスト
- `docs/{specs,designs,plans,tasks,test-specs,impl/ir}/
  tim-02-ppqn-smf-cross-golden*` — doc chain 6 件

## Done When

- [x] import@480 → clip anchor → `SetProjectPpqn`
      Exact/Round → save/load → export@480 byte-golden →
      reimport doc equality の全鎖が green
- [x] 各段階で `smf_documents`/`midi_sources` が変更前
      clone と一致（source-native skip 不変条件）
- [x] `ConversionReport` が全 exact・`rounded_count=0`・
      `max_error=0`・`unsupported_events` 空
- [x] tempo/CC64 provenance が export→reparse→reimport
      で保持
- [x] `src/` 無変更で完結（実不変違反なし）
- [x] 全検証 lane exit 0

## Verification

- [x] `cargo test -p synvelis-project --test
      tim_02_ppqn_smf` — 2 green
- [x] `cargo clippy -p synvelis-project --all-targets --
      -D warnings` — clean
- [x] `mise run fmt-check` / `check` / `lint` / `test` /
      `test-rt` / `check-app-native` / `lint-app-native` /
      `test-msrv` / `build` — 結果は impl IR に記録
