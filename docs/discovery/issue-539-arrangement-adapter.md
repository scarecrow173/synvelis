---
id: "DISC-539aArrngmntAdptr01"
type: "discovery"
status: "resolved"
title: "Issue #539 discovery — VariantPorts::standard への ArrangementAdapter 登録調査"
created: "2026-11-18"
updated: "2026-11-18"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/539"
    - "https://github.com/scarecrow173/synvelis/issues/288"
    - "https://github.com/scarecrow173/synvelis/issues/284"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/ir/cb-11-cross-gate.md"
    - "docs/impl/exp/cb-11-cross-gate.md"
    - "docs/test-specs/cb-11-cross-gate.md"
    - "evidence/sv288/coverage.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by:
    - "docs/specs/issue-539-arrangement-adapter.md"
  verifies: []
  verified-by: []
  references:
    - "crates/synvelis-gateway/src/variant.rs"
    - "crates/synvelis-project/src/variant/arrangement.rs"
---

# Issue #539 discovery — VariantPorts::standard への ArrangementAdapter 登録調査

## 背景

CB-E2E-03（arrangement scope の variant fork→edit→adopt）は
project 軸では `cb11_e2e_03_arrangement_composite_adopt_undo_scope_isolation`
で Pass 済みだが、gateway 経路では `VariantPorts::standard` の
adapters map が `ArrangementAdapter` を登録していなかったため、
`adapter_for` が `VariantError::Unsupported`
（"no domain adapter wired for arrangement scopes"）で
fail-closed していた。これは CB-10 slice の意図的な未配線であり、
`evidence/sv288/coverage.md` の「判明した gap」および
`docs/impl/exp/cb-11-cross-gate.md` E8 に follow-up として記録済み。

## 調査結果

- `ArrangementAdapter` 本体は `synvelis-project` に merge 済み
  （`crates/synvelis-project/src/variant/arrangement.rs`、
  `variant/mod.rs` で export 済み）。scope validation・
  candidate edit・preflight adopt check を実装済み。
- gateway 側の wire 経路は既に汎用:
  `VariantScope::Arrangement` の Deserialize・
  `VariantScopeKind::parse`/`of`・`scope_roots` Arrangement arm・
  `lower_variant` の EDIT/ADOPT/DISCARD dispatch・
  CommitGate Variant arm が全て存在する。
- `VariantPorts::standard` の adapters map には
  AudioClip/TakeUsage/MidiClip/InstrumentSlot/ProcessingRange/
  StripFields の 6 件のみ insert されており、
  `VariantScopeKind::Arrangement` が欠落していた — これが
  Issue #539 の root cause。
- `standard()` の consumer は synvelis-cli・mcp-poc 両 host と
  test rig。登録を `standard()` に入れれば両 host へ自動伝播し、
  host 側変更は不要。
- 前提の sv#518 JobSystem 配線（`GateDeps.render`）とは独立 seam
  （`GateDeps.variant`）で非干渉。arrangement adopt は
  CommitGate fence 内で `adopt_branch_adapted` を通し
  RenderDispatch を経由しない。

## gap の性質

fail-closed は安全側の挙動であり headless 契約違反ではなかったが、
CB-E2E-03 の gateway 経路 dispatch には src 変更（adapter 登録）が
必要だった。本 Issue はその登録 slice。
