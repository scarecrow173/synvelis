---
id: "PLAN-3vBnM7kQwE5rT9yU2iO8pA"
type: "plan"
status: "approved"
title: "SF-04 slice10 実装計画 — syn.probe.compare app op"
created: "2026-11-12"
updated: "2026-11-12"
owners: []
relations:
  source: []
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/sf-04-slice10-probe-compare-op.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/sf-04-slice10-probe-compare-op.md"
    - "docs/designs/sf-04-slice10-probe-compare-op-design.md"
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# SF-04 slice10 実装計画 — syn.probe.compare app op

## Goal

slice 6 の `ProbeTraceBridge::compare_probes` を `syn.probe.*`
canonical op shape で wire へ露出する。比較意味論・drain
所有権・session 相関は変更しない — op 登録・dispatch・
手動 JSON mapper・headless fixture のみ。

## File Map

| File | Change |
| --- | --- |
| `crates/synvelis-app/src/service.rs` | `syn.probe.compare` `OpInfo` + dispatch arm |
| `crates/synvelis-app/src/ui_ops.rs` | `op_probe_compare` + `probe_observation_json` 系 mapper helpers（`probe` contract import 追加） |
| `crates/synvelis-app/tests/sf04_probe_compare_op.rs` | headless fixture（新規・`AppService` dispatch 経由） |
| `docs/` | spec/design/plan/task/test-spec/impl-ir + managed index 再生成 |

`crates/synvelis-player/**`・`crates/synvelis-project/**`・
`crates/synvelis-workspace/**`・`ui/**` は変更なし
（consume-only）。

## Steps

1. `service.rs` — `OPS` に `syn.probe.compare` `OpInfo`、
   `dispatch` に arm を追加。
2. `ui_ops.rs` — `synvelis_project::probe` contract 型の
   import、`probe_*_json` mapper helpers、
   `op_probe_compare(&self)` 実装（`json_param` →
   engine check → `with_probe_bridge(compare_probes)` →
   `probe_observation_json`）。
3. `tests/sf04_probe_compare_op.rs` — `sf04_probe_wiring.rs`
   fixture 先例で `UiRuntime`/`AppService` dispatch 経由の
   検証を追加。
4. docs index 再生成・canonical `mise` 検証・commit/push/PR・
   Issue #107 コメント。

## Verification

- `mise run fmt-check` / `check` / `lint` / `test` /
  `test-rt` / `check-app-native` / `lint-app-native` /
  `test-msrv` / `build`（canonical 全件）。
- `crates/synvelis-app/tests/sf04_probe_compare_op.rs`:
  Aligned/Conditional/Incomparable・fail-closed reason・
  engine 未接続・malformed request・read-only 不変条件。

## Risks

- `#[non_exhaustive]` enum の将来 variant — `_ => "unknown"`
  で fail-closed 写像し、variant 追加時の exhaustive match
  がコンパイルエラーで気付ける構造を維持する。
- `AcousticObservation` の非 serde — mapper が clock id を
  `{domain,epoch}` で明示投影し、serde derive で暗黙に
  漏らさない（`ClockContextRef` 規則の継承）。
- drain 順序依存 — compare は最後の pump の証拠のみを見る。
  テストは `engine.run` + pump op の後に compare を呼ぶ
  順序で固定する。
