---
id: "TASK-Rn6S1d7f6t58waba4wZN2X"
type: "task"
status: "approved"
title: "CTL-02 canonical persistence tasks"
created: "2026-11-27"
updated: "2026-11-27"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/122"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/ctl-02-canonical-persistence.md"
  implemented-by:
    - "docs/impl/ctl-02-canonical-persistence.md"
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/ctl-02-canonical-persistence.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "evidence/sv122/canonical-persistence-verification.md"
  defers: []
  deferred-by: []
---

# CTL-02 canonical persistence tasks

## 完了

- [x] `canonical_internal_spec_deviations` と
  `normalize_canonical_internal_spec` を `control.rs` に追加した。
- [x] v15→v16 migration step を ordered ladder に追加した。
- [x] `SCHEMA_VERSION` を 16 に更新した。
- [x] `Project::validate_references` に canonical internal spec oracle を
  追加した。
- [x] `ctl_02.rs` に v15 normalization、v16 reject、fresh/v9 oracle pass
  fixture を追加した。
- [x] `cb01_variant.rs`、`tim_02_smf.rs`、`md_a.rs` の固定期待値を v16
  に更新した。
- [x] docs chain（spec/ADR/design/plan/task/test-spec/impl record）を追加した。

## 完了処理

- [x] 指定 verification lane を実行し、
  `evidence/sv122/canonical-persistence-verification.md` に OS・exit code・
  fixture 観測結果を記録した。canonical `mise run fmt-check` の環境依存
  failure と package-scoped rustfmt の補助証拠も記録した。
- [x] 最新 main が branch base `175b7b0d621f59e8b89cf0e5a0acfc784bbee940`
  と同じであることを確認し、`Cargo.lock` に差分がないことを確認した。
- [x] commit、push、PR #682 作成を完了した。
- [x] PR 本文に受入条件ごとの進捗、NotRun/Unsupported、docs index 未整備
  を明記した。

## 後続 slice / gate

- [ ] GUI / Inspector / Mixer の表示分離、Pending/Blocked/Observed surfacing。
- [ ] 実 plugin binding、SourceMap/ControlTrace 統合、Graph generation 照合、
  誤配送ゼロ実測。
- [ ] Channel mismatch approval flow、Realtime/Offline/2Window、AudioLens
  非回帰。
- [ ] EXT-L3 / GUI-L2 / 3OS / provisioned runner。
- [ ] docs index 整備（Coordinator 統合時）。
