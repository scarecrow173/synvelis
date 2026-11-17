---
id: "TSPEC-j0rVeZntFdxTUwZBQefJ"
type: "test-spec"
status: "approved"
title: "Issue #530 test-spec — stateless PluginStateDecl 契約の検証仕様"
created: "2026-11-19"
updated: "2026-11-19"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/530"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/issue-530-plugin-state-decl.md"
    - "docs/designs/issue-530-plugin-state-decl.md"
    - "docs/adr/issue-530-plugin-state-decl.md"
  implemented-by: []
  depends-on:
    - "docs/specs/issue-530-plugin-state-decl.md"
    - "docs/designs/issue-530-plugin-state-decl.md"
    - "docs/adr/issue-530-plugin-state-decl.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/plans/issue-530-plugin-state-decl.md"
    - "evidence/sv530/verification.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/issue-530-plugin-state-decl.md"
  derived-by: []
  verifies:
    - "docs/specs/issue-530-plugin-state-decl.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# Issue #530 test-spec

## Feature: stateless PluginStateDecl 契約

`PluginStateDecl::stateless()` profile と `verify_plugin` の
`StateRestoreSupport::Unsupported` arm の新契約を、型レベル・
verdict matrix・selection 統合の 3 層で oracle 化する。

## Rule TS-1: stateless profile の型レベル oracle（`rcp_04.rs`）

`ex03_stateless_profile_is_typed_and_distinct_from_uncaptured`:

- `digest: None` + `quality: Unsupported` + `status: Unknown`/`Missing`
  → `stateless() == true`
- `digest: None` + capture 可能 quality（`BestEffort`/`Quiescent`/
  `Coordinated`）→ `stateless() == false`（未 capture — fail-closed 側）
- `digest: Some` または `status: Known` → `stateless() == false`
  （矛盾 decl — fail-closed 側）

## Rule TS-2: env/decl verdict matrix（`rcp_04.rs`）

`ex03_stateless_env_decl_verdict_matrix` が spec の真理値表を全行
pin する:

- env=`Unsupported` + stateless decl → `Live`（新規経路）
- env=`Unsupported` + state-bearing / 未 capture / 矛盾 decl →
  `StateUnsupported`
- env=`Verified` + stateless decl → `StateUnknown`（env/decl drift —
  fail-closed）
- env=`Verified` + state-bearing → `Live`、env=`Verified` + 未 capture
  → `StateUnknown`（既存経路の回帰 pin）
- env=`Unknown` + stateless decl → `StateUnknown`

## Rule TS-3: selection 統合 oracle（`rcp_04.rs`）

- `ex03_stateless_plugin_selects_live`: stateless decl +
  env=`Unsupported` → node verdict `Live` → selection `Exact`。
- `ex03_stateless_decl_env_drift_and_uncaptured_stay_blocked`:
  env=`Verified` + stateless decl → `StateUnknown` → `Blocked`;
  env=`Unsupported` + 未 capture decl → `StateUnsupported` →
  `Blocked`。

## Rule TS-4: workspace 統合 oracle（`rcp_selection_integration.rs`）

`rcp_ex01_stateless_plugin_evidence_is_live`: `ProcessorPluginEvidence`
の stateless profile + env `Unsupported` が `resolve_selection` →
`select` → `drive` で `Exact`/`Live`/`Published` に到達すること、
および同じ env に対し capture 可能 quality の `digest: None` 証拠が
`StateUnsupported`/`Blocked` に留まることを pin する。

## Rule TS-5: lane の正直記録

- headless/fixture lane の Pass は当該 lane が実際に exercise した
  挙動のみに適用する。
- 実 plugin・実 OS・RT/PCM 実測は NotRun として記録し、headless
  Pass を実機 Pass に昇格しない。gate: #109・#296・
  `scarecrow173/ruviform#630`・`scarecrow173/ruviform#631`。

## Examples

- Example 1: `mise run test synvelis-player` → `rcp_04` の新規
  oracle 4 件を含む全 test pass。
- Example 2: `mise run test synvelis-workspace` →
  `rcp_selection_integration` の新規 case を含む全 test pass。
- Example 3: `git diff` で `crates/synvelis-render/tests/
  rcp07_gate.rs`・`crates/synvelis-workspace/src/render_ui.rs` が
  無変更であることを確認（lease 外波及なしの証明）。
