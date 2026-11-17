---
id: "PLAN-034ZjKIkevNThVWL0ubz8g"
type: "plan"
status: "approved"
title: "Issue #283 slice — strip-fields gateway exercise plan"
created: "2026-11-17"
updated: "2026-11-17"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/283"
    - "docs/specs/issue-283-strip-gateway-exercise.md"
    - "docs/designs/issue-283-strip-gateway-exercise.md"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-gateway/tests/cb06_strip_gateway.rs"
      - type: "file"
        path: "docs/discovery/issue-283-strip-gateway-exercise.md"
      - type: "file"
        path: "docs/specs/issue-283-strip-gateway-exercise.md"
      - type: "file"
        path: "docs/designs/issue-283-strip-gateway-exercise.md"
      - type: "file"
        path: "docs/plans/issue-283-strip-gateway-exercise.md"
      - type: "file"
        path: "docs/tasks/issue-283-strip-gateway-exercise.md"
    modified:
      - type: "file"
        path: "docs/test-specs/cb-06-track-bus-adapter.md"
      - type: "file"
        path: "docs/impl/ir/cb-06-track-bus-adapter.md"
      - type: "file"
        path: "docs/impl/exp/cb-06-track-bus-adapter.md"
      - type: "file"
        path: "evidence/sv283/acceptance-ledger.md"
      - type: "file"
        path: "evidence/sv283/verification.md"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/issue-283-strip-gateway-exercise.md"
    - "docs/designs/issue-283-strip-gateway-exercise.md"
  implemented-by:
    - "docs/tasks/issue-283-strip-gateway-exercise.md"
  depends-on:
    - "docs/designs/issue-283-strip-gateway-exercise.md"
    - "docs/discovery/issue-283-strip-gateway-exercise.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "evidence/sv283/verification.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/issue-283-strip-gateway-exercise.md"
    - "docs/specs/issue-283-strip-gateway-exercise.md"
  derived-by:
    - "docs/tasks/issue-283-strip-gateway-exercise.md"
  verifies: []
  verified-by: []
  test-spec-skip: "本 slice は gateway commit-gate 経路の exercise であり test-spec 文書を新設しない（doc chain lease は discovery/specs/designs/plans/tasks の 5 点）。検証意図は docs/test-specs/cb-06-track-bus-adapter.md の「Gateway commit-gate 経路カバレッジ」節に追記済みで、実行検証は本 plan の Verification 節と evidence/sv283/verification.md に記録する"
  references: []
  defers: []
  deferred-by: []
---

# Issue #283 slice — strip-fields gateway exercise plan Implementation Plan

## Goal

`VariantPorts::standard` 経由で strip-fields scope の CB-10 typed
contract lifecycle（fork → typed edit reject → adopt CAS/approval/
digest pin → discard → describe/list/compare）を headless で
exercise し、gateway commit-gate 経路の strip-fields 証拠を
`cb06_strip_gateway.rs` として追加する。

## Scope

単一 slice。test 追加 + doc chain + evidence 追記のみ。
production code（`synvelis-gateway/src`・`synvelis-project`）の
変更は含まない。

## Upstream Documents

- `docs/specs/issue-283-strip-gateway-exercise.md`
- `docs/designs/issue-283-strip-gateway-exercise.md`
- `docs/discovery/issue-283-strip-gateway-exercise.md`

## Design Inputs

- `docs/designs/issue-283-strip-gateway-exercise.md`

## File Map

- `crates/synvelis-gateway/tests/cb06_strip_gateway.rs` —
  strip-fields gateway commit-gate exercise（新規・主成果物）
- `docs/discovery/issue-283-strip-gateway-exercise.md` —
  調査記録（wire 表面に strip op capability が無い実測）
- `docs/specs/issue-283-strip-gateway-exercise.md` — 受入仕様
- `docs/designs/issue-283-strip-gateway-exercise.md` — 設計判断
- `docs/plans/issue-283-strip-gateway-exercise.md` — 本 plan
- `docs/tasks/issue-283-strip-gateway-exercise.md` — task 分解
- `docs/test-specs/cb-06-track-bus-adapter.md` — gateway 経路
  検証の追従記録
- `docs/impl/ir/cb-06-track-bus-adapter.md` — 実装記録の追従
- `docs/impl/exp/cb-06-track-bus-adapter.md` — 実験記録の追従
- `evidence/sv283/acceptance-ledger.md` — gateway lane 追記
- `evidence/sv283/verification.md` — 検証証拠追記

## Implementation Sequence

- [x] T1: `cb06_strip_gateway.rs` 新規 — strip fixture
      （`AddTrackStrip` + `InsertProcessor`、revision 2）・
      fork・typed edit reject・stale pin reject・agent
      approval adopt・discard・query 群・domain boundary/
      capability 補助 test
- [x] T2: doc chain 5 点（discovery/specs/designs/plans/tasks、
      slug `issue-283-strip-gateway-exercise`）作成
- [x] T3: cb-06 doc chain 追従（test-specs/impl ir/impl exp）
- [x] T4: `evidence/sv283/` ledger・verification 追記
- [ ] T5: `review-protocol` レビュー・blocker 0 確認
- [ ] T6: 正規検証（mise）実行・merge origin/main・commit・
      push・PR・worker_done

## Dependencies and Risks

- wire 表面に strip-domain inner op capability が無い →
  positive mask-gated edit は表現不能。edit 面は typed reject
  の一様性で証明し、routing/external op の fail-closed を
  境界証拠とする（design で確定済み）。
- adopted branch edit は live edit のため confinement 負例は
  candidate branch のみで構成。
- `AdoptOutcome::NoChange` ではなく `Applied` を期待 —
  scope-identical candidate でも pointer swap/freeze は実
  project truth（engine 実測済み）。
- `docs/**/README.md` index は Coordinator が統合時に再生成 —
  Worker は編集・`index apply` 実行をしない。

## Task Breakdown

- T1 → T2 → T3 → T4 → T5 → T6（逐次。T3/T4 は T1 の test 結果を
  根拠に記述）。

## Verification

- [x] `mise run check synvelis-gateway` — compile
- [x] `cargo test --test cb06_strip_gateway`（補助証拠）—
  2 test Pass
- [x] `mise run fmt-check`
- [x] `mise run lint synvelis-gateway`
- [x] `mise run test synvelis-gateway`（新規 test 含む）
- [x] `mise run check-headless-deps`
- [x] `mise run check synvelis-project`（非回帰）
- [x] `mise exec -- prek run --all-files`
- [ ] `review-protocol` — blocker 0

## Open Questions / Gaps

- なし。wire 表面の strip op capability 追加は本 slice 範囲外
  （spec の Deferred Design Concerns に記録）。

## Self-Review

- [x] Every spec requirement maps to a task.
- [x] No placeholders remain.
- [x] File paths, relations, status values, and terminology are
      consistent.

## Implementation Handoff

- Recommended mode: inline execution（dispatch worker が直接実装）
- Independent work streams: none
- User approval required before dispatch: no（review-protocol の
  blocker 0 を承認として扱う）

## Review Handoff

- reviewer focus: wire 表面に strip op capability が無いという
  実測に基づく「positive mask-gated edit 非表現」の扱いが
  spec/design と test の主張で一貫しているか。typed reject の
  網羅性（ordinary op 3 種 + stale pin 2 種）と provenance
  assertion の正確性。
