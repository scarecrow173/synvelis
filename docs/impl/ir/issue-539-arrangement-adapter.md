---
id: "IMPL-539aArrngmntAdptr08"
type: "impl"
status: "completed"
title: "Issue #539 impl record — ArrangementAdapter 登録 slice"
created: "2026-11-18"
updated: "2026-11-18"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/539"
  changes:
    added:
      - type: "dir"
        path: "docs/discovery"
      - type: "dir"
        path: "docs/specs"
      - type: "dir"
        path: "docs/designs"
      - type: "dir"
        path: "docs/adr"
      - type: "dir"
        path: "docs/plans"
      - type: "dir"
        path: "docs/tasks"
      - type: "dir"
        path: "docs/test-specs"
      - type: "dir"
        path: "docs/impl/ir"
      - type: "dir"
        path: "docs/impl/exp"
      - type: "dir"
        path: "evidence/sv539"
    modified:
      - type: "file"
        path: "crates/synvelis-gateway/src/variant.rs"
      - type: "file"
        path: "crates/synvelis-gateway/tests/cb11_ai_vcs_off.rs"
      - type: "file"
        path: "docs/impl/ir/cb-11-cross-gate.md"
      - type: "file"
        path: "docs/impl/exp/cb-11-cross-gate.md"
      - type: "file"
        path: "docs/test-specs/cb-11-cross-gate.md"
      - type: "file"
        path: "evidence/sv288/coverage.md"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/issue-539-arrangement-adapter.md"
    - "docs/tasks/issue-539-arrangement-adapter.md"
  implemented-by: []
  depends-on:
    - "docs/plans/issue-539-arrangement-adapter.md"
    - "docs/tasks/issue-539-arrangement-adapter.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/exp/issue-539-arrangement-adapter.md"
    - "evidence/sv539/verification.md"
    - "evidence/sv288/coverage.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/plans/issue-539-arrangement-adapter.md"
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted:
      - "docs/impl/exp/issue-539-arrangement-adapter.md"
    rejected: []
---

# Issue #539 impl record

## 実施内容

### production 変更（`crates/synvelis-gateway/src/variant.rs`）

- `use synvelis_project::variant::{...}` に
  `ArrangementAdapter` を追加。
- `VariantPorts::standard` の adapters map に
  `VariantScopeKind::Arrangement → Arc::new(ArrangementAdapter::new())`
  を登録（6 adapter → 7 adapter で全 variant 網羅）。
- 陳腐化 doc comment 4 箇所を更新:
  - `VariantScopeKind` 節の「Arrangement has no adapter」系記述
  - `VariantPorts.adapters` の「intentionally absent」記述
  - `standard()` の「kinds without an adapter」記述
  - `adapter_for` の doc — fail-closed 機構は維持し、
    未配線 kind が他 adapter を借用しない旨を明記。
- `adapter_for` の `VariantError::Unsupported` 経路自体は
  無変更で維持。

### test 変更（`crates/synvelis-gateway/tests/cb11_ai_vcs_off.rs`）

- モジュール doc の honest boundary 節を更新
  （Arrangement は `VariantPorts::standard` 経由で wired）。
- `arrangement_scope()` helper の doc を demo fixture の
  実形状に合わせて更新。
- `move_op()` helper 追加（clip 13 → `Musical(960)`）。
- 旧 `cb11_e2e_07_arrangement_scope_fails_closed_until_gateway_wiring`
  を `cb11_e2e_07_arrangement_scope_composite_adopt_undo_scope_isolation`
  へ置換。fork → mute+move composite edit → live 不変確認 →
  ApproveToApply agent adopt（初回 `ApprovalRequired` →
  human receipt retry）→ CommitGate commit →
  単一 CAS=1 undo・provenance・scope 外 bitwise 不変・
  adopted branch 確定を assert。

### gap 記録の解消

- `evidence/sv288/coverage.md`: gateway 経路行を Pass 化、
  fixture 表・domain matrix・「判明した gap」節に解消記録。
- `docs/impl/ir/cb-11-cross-gate.md`: slice 2 記述に解消注記、
  Risks/Follow-ups を更新。
- `docs/impl/exp/cb-11-cross-gate.md`: E8 に解消追記。
- `docs/test-specs/cb-11-cross-gate.md`: Example・Non-goals の
  gap 記述を更新。

## 判断メモ

- clip-local note op は composite に含めなかった
  （mute+move の clip ops のみで CB-E2E-03 契約を満たすため
  `tests/rig/mod.rs` の fixture 拡張は不要と判断）。
- `adapter_for` の Unsupported 経路は全 variant 網羅後
  到達不能だが、将来の scope kind 追加への防御として維持。
  AC3 の fail-closed 維持確認は既存 fail-closed 境界
  （Unapproved/CAS/scope violation）の非回帰で代替。

## 検証

`evidence/sv539/verification.md` を参照。
