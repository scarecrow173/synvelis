---
id: "PLAN-QRGcQW6CvUDaVfCwWADz2f"
type: "plan"
status: "in-progress"
title: "SF-XR-01 実装計画 — CTL-01/CTL-02 cross-gate oracle fixture"
created: "2026-11-10"
updated: "2026-11-10"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/109"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/sfxr01-ctl-crossgate-oracle-fixture.md"
  implemented-by: []
  depends-on:
    - "docs/specs/sfxr01-ctl-crossgate-oracle-fixture.md"
    - "docs/designs/sfxr01-ctl-crossgate-oracle-fixture.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/sfxr01-ctl-crossgate-oracle-fixture.md"
    - "docs/designs/sfxr01-ctl-crossgate-oracle-fixture.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/sfxr01-ctl-crossgate-oracle-fixture.md"
  references:
    - "docs/specs/sfxr01-ctl-crossgate-oracle-fixture.md"
    - "docs/designs/sfxr01-ctl-crossgate-oracle-fixture.md"
  defers: []
  deferred-by: []
---

# SF-XR-01 実装計画 — CTL-01/CTL-02 cross-gate oracle fixture

## Scope

fixture-only slice。変更対象は
`crates/synvelis-player/tests/sf_xr_01.rs`（新規）・
`evidence/reports/sf-06-cross-gate-ledger.md`（SF-XR-01 行 +
evidence row）・`docs/` チェーンのみ。`crates/*/src/` と他
crate（library/app/workspace/render/recording）は不変。

## Steps

1. coverage 棚卸し — `ctl_01.rs`/`ctl_02.rs`/`sf_02.rs`/
   `sf06_oracle.rs`/`sf03_l1.rs` の既存 coverage と重複しない
   cross-gate 観点を確定（persisted→serialization/undo→
   lowering の一気通貫 + CreateSend consent matrix）。
2. `sf_xr_01.rs` 実装 — helper は `sf03_l1.rs`/`sf_02.rs` の
   規約を踏襲（controller/apply/strip_port/intent/lower_for_test）。
3. focused test 実行 → fmt/clippy 修正 → green 確認。
4. docs チェーン（spec/adr/design/plan/task/test-spec/impl-ir）
   と ledger 更新。
5. `git fetch origin main && git merge origin/main` の後、
   `mise run fmt-check` / `check` / `lint` / `test` を実行。
6. Conventional Commits で commit → branch push → PR 作成
   （本文日本語・`Refs #109`）。

## Verification matrix

| 検証 | 方法 |
|---|---|
| fixture green | `cargo test -p synvelis-player --test sf_xr_01`（補助証拠）+ `mise run test`（canonical） |
| lint | `mise run lint` |
| format | `mise run fmt-check` |
| 型 | `mise run check` |

## Risks

- `lower_semantic` の内部仕様変更で fixture が脆くなる →
  公開 API（`RoutingSourceMap`/`GraphSpec`）のみを根拠にし、
  実装 helper には依存しない。
- ledger の lane 判定を過大評価するリスク → headless 実測分のみ
  Pass、他は `NotRun` を維持。
