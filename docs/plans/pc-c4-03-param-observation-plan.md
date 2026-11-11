---
id: "PLAN-PCc4030bs3vN1pQ8wXyZ2aD"
type: "plan"
status: "approved"
title: "PC-C4-03 param observation plan — drain surface + tests + docs"
created: "2026-11-11"
updated: "2026-11-11"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/101"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/designs/pc-c4-03-param-observation-design.md"
  implemented-by: []
  depends-on:
    - "docs/designs/pc-c4-03-param-observation-design.md"
    - "docs/specs/pc-c4-03-param-observation.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "docs/specs/pc-c4-03-param-observation.md"
  defers: []
  deferred-by: []
---

# PC-C4-03 plan — reported observation 消費

## スコープ

| 領域 | ファイル |
| --- | --- |
| 実装 | `crates/synvelis-player/src/routing_control.rs`（`drain_output_reports`・`observe_cycle`・`ReportDrainReport`）、`crates/synvelis-player/src/lib.rs`（re-export） |
| テスト | `crates/synvelis-player/tests/pc_c4_03_observation.rs`（新規） |
| 文書 | 本チェーン 6 件 + 各 `docs/*/README.md` index 行 |

`dispatch.rs` は変更不要 — `record_reported`/`mark_reporting_unavailable`/
`observe_cycle`/`stage` は既に正しい ledger semantics を持つ。

## タスク分解

1. **task1 — drain surface + observe accessor + tests**
   - `ReportDrainReport`・`drain_output_reports`・`observe_cycle` を
     `RoutingControlDispatch` に追加。
   - `lib.rs` に `ReportDrainReport` を re-export。
   - `pc_c4_03_observation.rs` に 8 test（drain→Reported、Unavailable
     終端、generation/inactive gate、unbound/non-param/no_pending、
     host error typed、drain-once、generation swap）。

## 検証

- `mise run fmt-check` / `check` / `lint` / `test` / `test-rt` /
  `test-msrv` / `check-headless-deps` / `check-ruviform-rev`。
- 補助証拠: `cargo test -p synvelis-player --test pc_c4_03_observation`
  （canonical = mise）。

## リスク

- `ParamId` 衝突: session binding table が `(node, ParamId)` 一意を保証
  （`param_in_use`）— 逆引きは一意。
- report frame 帰属なし: `record_reported` は最新 eligible entry を
  `Reported` にする ledger semantics をそのまま使う。
- L1 `Unavailable` 実値: pin `b02942c2` に wire report verb なし →
  `NotRun`、注入 seam で分岐を検証。
