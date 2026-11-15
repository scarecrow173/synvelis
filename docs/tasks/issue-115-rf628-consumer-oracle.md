---
id: "TASK-KmJx9ONwsNiLw0BB4qupc7"
type: "task"
status: "done"
title: "Issue #115 rf#628 consumer oracle — synvelis#115 slice"
created: "2026-11-15"
updated: "2026-11-15"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/115"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/issue-115-rf628-consumer-oracle.md"
  implemented-by: []
  depends-on:
    - "docs/plans/issue-115-rf628-consumer-oracle.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/issue-115-rf628-consumer-oracle.md"
  references: []
  defers: []
  deferred-by: []
---

# Issue #115 rf#628 consumer oracle — synvelis#115 slice

## Tasks

- [x] T1: ruviform 最新 main（`abb9f372`・`pipeline/` 収録確認済み）へ
  pin bump — `cargo update -p ruviform-*`（28 package 単一 SHA）+
  `sync-ui-deps` + `check-ruviform-rev`/`--require-vendor` で REV 一致。
- [x] T2: `ruviform-audio-test` を workspace dep + `synvelis-recording`
  dev-dep に追加（production 依存方向は不変）。
- [x] T3: `tests/rf628_oracle.rs` — `rf628_record`/`drive_session`/
  `verify_scenario_media`/`assert_scenario_stream`/`assert_faulted_prefix`
  helper + SV-1〜SV-5 の 9 test を実装。
- [x] T4: `tests/rf628_oracle_rt_alloc.rs` — SV-6 `stats_alloc` 単一
  計測テスト（`rt_` prefix・warm-up 済み `Region`）。
- [x] T5: `evidence/sv115/` case matrix `rf#628-fixture` 列・ledger 行・
  本 slice 検証記録を実測で更新。
- [x] T6: doc chain 7 点 + `docs/*/README.md` index 再生成。
- [x] T7: 正規検証（fmt-check/check/lint/test synvelis-recording/
  test-rt/check-headless-deps/check-ruviform-rev/prek）+
  review-protocol + commit/push/PR（`Refs #115`）。

## Notes

- `tests/common/mod.rs`・production `src/`・既存 test file は非変更。
  `RealDeviceRig::open` の `&dyn Device` 経路をそのまま再利用した。
- `PumpReport` 駆動のため fake `deliver` とは別経路 — 共通化せず
  `rf628_oracle.rs` 内に閉じた helper とした（design D3/D4）。
- `Sine` segment は `AbsEpsilon(1e-5)`、他 pattern は `Exact` で
  `pcm_assert_eq` 照合（ruviform `SINE_EPS` 規約踏襲）。
