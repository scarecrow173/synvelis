---
id: "PLAN-7c4e9a1b2d3f4e5a6b7c8f0a1"
type: "plan"
status: "approved"
title: "PC-C4-06 control op surface plan"
created: "2026-11-12"
updated: "2026-11-12"
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
    - "docs/designs/pc-c4-06-control-op-surface-design.md"
  implemented-by: []
  depends-on:
    - "docs/specs/pc-c4-06-control-op-surface.md"
    - "docs/designs/pc-c4-06-control-op-surface-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/pc-c4-06-control-op-surface.md"
    - "docs/designs/pc-c4-06-control-op-surface-design.md"
  derived-by: []
  verifies:
    - "docs/test-specs/pc-c4-06-control-op-surface.md"
  verified-by:
    - "docs/test-specs/pc-c4-06-control-op-surface.md"
  references:
    - "docs/specs/pc-c4-06-control-op-surface.md"
    - "docs/designs/pc-c4-06-control-op-surface-design.md"
  defers: []
  deferred-by: []
---

# PC-C4-06 control op surface plan Implementation Plan

## Goal

PC-C4 control surface の deferred API を `syn.control.*` op として
`AppService` に公開し、fast-control write・tick/report status・
observation/feed evidence を typed evidence のみで返す。

## Scope

単一 implementation stream — wire schema・handler・engine
accessor・fixture は同一 op 契約の表裏であり、分割すると中間状態
（op は登録済みだが evidence 投影が未検証）が生じる。worker task
は 1 件（task1）に集約する。

## Upstream Documents

- `docs/specs/pc-c4-06-control-op-surface.md`

## Design Inputs

- `docs/designs/pc-c4-06-control-op-surface-design.md`（approved）

## File Map

| Path | Responsibility |
| --- | --- |
| `crates/synvelis-app/Cargo.toml` | `ruviform-plugin-core` を `[dependencies]` へ追加・dev-dep 重複除去（Coordinator 承認済み） |
| `crates/synvelis-app/src/engine.rs` | `control_frame_base` / `control_last_block` read-only accessor 追加 |
| `crates/synvelis-app/src/ui_ops.rs` | `UiOpsState.last_control_drain`・wire projection/parse helper・`op_control_fast_control`/`op_control_status`/`op_control_observe`・`note_control_drain` |
| `crates/synvelis-app/src/service.rs` | `OPS` 3 エントリ + `dispatch` 3 route |
| `crates/synvelis-app/tests/pc_c4_06_control_op_surface.rs` | 新規: `SharedService` 経由 `UiRuntime::dispatch_op` で 4 test |
| `docs/specs/pc-c4-06-control-op-surface.md` | 新規 spec |
| `docs/designs/pc-c4-06-control-op-surface-design.md` | 新規 design |
| `docs/plans/pc-c4-06-control-op-surface-plan.md` | 本 plan |
| `docs/tasks/pc-c4-06-control-op-surface-task.md` | 新規 task |
| `docs/test-specs/pc-c4-06-control-op-surface.md` | 新規 test spec |
| `docs/impl/ir/pc-c4-06-control-op-surface.md` | 新規 implementation record |

## Implementation Sequence

- [x] T1 — dependency + additive plumbing: `Cargo.toml` に
  `ruviform-plugin-core`（Coordinator 承認済み）、`engine.rs` の
  `control_frame_base`/`control_last_block`、`ui_ops.rs` の
  `last_control_drain` field・wire helper・handler 骨格、
  `service.rs` の `OPS`/route 登録。`cargo check` green。
- [x] T2 — headless fixture `pc_c4_06_control_op_surface.rs`:
  `SharedService` wrapper + `UiRuntime::dispatch_op` で
  surface 無し fail-closed・typed outcome/status・observe stage
  選別 + Reported feed・generation gate の 4 test。green。
- [x] T3 — canonical verification: `mise run fmt-check`/`check`/
  `lint`/`test`/`test-rt`/`check-app-native`/`lint-app-native`/
  `test-msrv`/`build` を全て実走。
- [x] T4 — doc chain 完成 + impl IR 記録・audit。

## Dependencies and Risks

- T1→T2→T3→T4 は直列。
- Risk: `ParamWriteMeans` が `synvelis-app` lib から名指し不能
  — dev-dep のみで re-export 経路無し。Coordinator 承認済みで
  `[dependencies]` 追加により解消（player src 変更は不要だった）。
- Risk: `plan_generation` を caller が誤ると projection gate で
  `Scheduled` 残留 — fail-safe 側（誤配送ではない）。receipts の
  構造化 JSON が caller の学習経路。
- Risk: `observe` op 末尾の `ui_pump` が automation dispatch で
  plan generation を進める — test は receipts で適用済み世代を
  確認し、次 span の generation を +1 規則で組み立てる
  （fixture 内で決定的）。

## Task Breakdown

- task1（`docs/tasks/pc-c4-06-control-op-surface-task.md`）:
  dependency・engine accessor・ui_ops handler/wire・service 登録・
  fixture test・doc chain の全量。独立 stream なし。

## Verification

- [ ] `mise run fmt-check` — 差分なし
- [ ] `mise run check` — warning ゼロ
- [ ] `mise run lint` — `-D warnings` 通過
- [ ] `mise run test` — 新規 fixture 4 test + 既存全 test green
- [ ] `mise run test-rt` — RT lane green
- [ ] `mise run check-app-native` / `lint-app-native` — native lane green
- [ ] `mise run test-msrv` — MSRV clippy green
- [ ] `mise run build` — workspace build 成功

## Open Questions / Gaps

- `observe_cycle` の per-block call-window 自動供給・
  `drain_output_reports` engine 配線・production composition root
  は後続 slice（design Deferred 節）。
- `status` への live `plan_generation` 露出は player accessor が
  必要で範囲外 — observe receipts が代替証拠。

## Self-Review

- [x] Every spec requirement maps to a task.
- [x] No placeholders remain.
- [x] File paths, relations, status values, and terminology are consistent.

## Implementation Handoff

- Recommended mode: inline execution（worker が T1→T4 を直列実行）
- Independent work streams: none
- User approval required before dispatch: no（dispatch 済み worker task）

## Review Handoff

- `ruviform-plugin-core` の `[dependencies]` 追加は Coordinator
  承認済み — reviewer は承認の経緯（`ParamWriteMeans` が lib code
  から名指し不能だった blocker）との一致を確認すること。
- `observe` の span が全て caller 証拠である設計（捏造しない）の
  妥当性 — receipts 構造化が caller の generation 学習経路。
