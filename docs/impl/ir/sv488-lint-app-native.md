---
id: "IMPL-034YnLdVeKPAUlSteoc0Z7"
type: "impl"
status: "completed"
title: "Issue #488 lint-app-native clippy::large_enum_variant 是正"
created: "2026-11-16"
updated: "2026-11-16"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/488"
    - "https://github.com/scarecrow173/synvelis/actions/runs/43784707716"
  changes:
    added:
      - type: "file"
        path: "docs/specs/sv488-lint-app-native.md"
      - type: "file"
        path: "docs/designs/sv488-lint-app-native.md"
      - type: "file"
        path: "docs/plans/sv488-lint-app-native.md"
      - type: "file"
        path: "docs/tasks/sv488-lint-app-native.md"
      - type: "file"
        path: "docs/test-specs/sv488-lint-app-native.md"
      - type: "file"
        path: "docs/discovery/sv488-lint-app-native.md"
      - type: "file"
        path: "docs/impl/ir/sv488-lint-app-native.md"
      - type: "file"
        path: "docs/impl/exp/sv488-lint-app-native.md"
      - type: "file"
        path: "docs/impl/exp/sv488-lint-app-native-exp.jsonl"
      - type: "file"
        path: "evidence/sv488/"
    modified:
      - type: "file"
        path: "crates/synvelis-workspace/src/library_ui.rs"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/tasks/sv488-lint-app-native.md"
    - "docs/plans/sv488-lint-app-native.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/sv488-lint-app-native.md"
    - "docs/designs/sv488-lint-app-native.md"
    - "docs/test-specs/sv488-lint-app-native.md"
    - "docs/impl/exp/sv488-lint-app-native.md"
    - "evidence/sv488/lint-app-native-verification.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "crates/synvelis-workspace/src/library_ui.rs"
    - "crates/synvelis-app/src/ui_ops.rs"
    - "mise.toml"
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted:
      - "docs/impl/exp/sv488-lint-app-native.md"
      - "docs/impl/exp/sv488-lint-app-native-exp.jsonl"
    rejected: []
---

# Issue #488 lint-app-native clippy::large_enum_variant 是正

## Summary

Issue #488。main `f05bdbf` の Full CI（run 43784707716）で
`verify (ubuntu-latest)` の `mise run lint-app-native` が
`clippy::large_enum_variant`（`library_ui.rs:454` の
`LibraryAction`、ImportSubmit ≥305B vs ImportRebase ≥80B）で
失敗し、fail-closed の `evidence`・`ci-summary` が連鎖失敗
していた。原因は `synvelis-app` の `native` feature が
`gpui → gpui_perf` 経由で `serde_json/preserve_order` を
feature unification し、`serde_json::Value` が肥大化して
variant サイズ差が lint 閾値を超えたこと。`ImportSubmit.
target` を `Option<Box<ImportTargetWire>>` に `Box` 化する
構造修正で解消し、serde wire form・公開シグネチャ・
crate 外 consumer を不変に保った。

## Context

- Issue: <https://github.com/scarecrow173/synvelis/issues/488>
- CI 失敗 run: <https://github.com/scarecrow173/synvelis/actions/runs/43784707716>
- doc chain: `docs/discovery/sv488-lint-app-native.md` /
  `docs/specs/sv488-lint-app-native.md` /
  `docs/designs/sv488-lint-app-native.md` /
  `docs/plans/sv488-lint-app-native.md` /
  `docs/tasks/sv488-lint-app-native.md` /
  `docs/test-specs/sv488-lint-app-native.md`
- 前提: 混入元 PR scarecrow173/synvelis#485（ALIB-05、
  `LibraryAction` 追加）は merge 済みで main `f05bdbf` に存在。
  作業は origin/main `f05bdbf` をベースにブランチ
  `fix/488-large-enum-variant` で実施。
- 根本原因の特定: `cargo tree -p synvelis-app --features
  native -e features -i serde_json` で `gpui_perf →
  serde_json/preserve_order` の経路を確認。`preserve_order`
  で `serde_json::Map` が `IndexMap` 化し `Value` が肥大化、
  `ImportSubmit` が保持する 3 `Value`（`import_id`・
  `target.placement`・`target.playback`）経由で variant が
  ≥305B に達した。`lint synvelis-workspace` 単独では
  `preserve_order` が無効で lint が発火しない非対称性を
  ローカルで再現・確認した。

## Implementation

### Changes

- `crates/synvelis-workspace/src/library_ui.rs`:
  `LibraryAction::ImportSubmit.target` を
  `Option<ImportTargetWire>` → `Option<Box<ImportTargetWire>>`
  に変更し、field doc comment に Box 化の理由
  （feature unification 下での `serde_json::Value` 肥大化と
  `clippy::large_enum_variant` への言及）を記録。
- 同ファイル `apply()` の match arm: `target.as_ref()` →
  `target.as_deref()` に整合（`import_submit` の
  `Option<&ImportTargetWire>` シグネチャは不変）。
- doc-chain 9 文書（discovery/spec/design/plan/task/
  test-specs/impl-ir/impl-exp md+jsonl）と
  `evidence/sv488/` を新規作成。

### Decisions During Implementation

- **Box 化の粒度は field 単位**: variant 全体の
  `ImportSubmit(Box<Payload>)` newtype 化は、serde の
  internally-tagged enum における newtype variant と
  `deny_unknown_fields` の相互作用検証が必要で diff も
  大きい。`target` が variant の支配的 payload
  （`ImportTargetWire` ≈ 2 String + 2 `Value`）であり、
  field 単位の `Box` 化で variant 間サイズ差が閾値を
  十分下回るため最小の整合的変更を採用した。
- **serde 透過性の利用**: `Option<Box<T>>` は serde に
  対して透過的で `Option<T>` と同一の JSON shape に
  serialize/deserialize されるため、`#[serde(tag =
  "action", deny_unknown_fields)]`・derive・
  `#[non_exhaustive]`・field の `#[serde(default)]` を
  すべて維持したまま wire form を不変に保てた。
- **crate 外 consumer の無変更確認**: `synvelis-app` の
  `op_library_action`（`ui_ops.rs`）は
  `serde_json::from_value` による deserialize のみで、
  wire form 不変のため変更不要。`test synvelis-app` で
  `library_browser.rs` の `import.submit` JSON 経路を含む
  全 suite の pass を実測した。
- **docs index の非再生成**: `docs/**/README.md` は lease
  外（Coordinator が統合時に再生成）のため Worker は
  再生成・手編集していない。

## Related Experiments

### Adopted

- `docs/impl/exp/sv488-lint-app-native.md` /
  `docs/impl/exp/sv488-lint-app-native-exp.jsonl`:
  feature unification（`gpui_perf → serde_json/
  preserve_order`）の特定、失敗再現、修正粒度の比較
  （field 単位 Box vs variant 全体 Box vs lint 隠蔽）、
  および検証実測を記録した実験ログを採用した。

### Rejected

- variant 全体の `Box` 化（`ImportSubmit(Box<Payload>)`）:
  serde internally-tagged enum の newtype variant と
  `deny_unknown_fields` の相互作用検証が必要で diff が
  大きく、field 単位の Box で十分なため不採用。
- `#[allow(clippy::large_enum_variant)]`・lint 設定緩和:
  Issue 受入条件で禁止のため不採用。

## Validation

- `mise run lint-app-native`: 修正前 exit 101
  （`large_enum_variant`、ImportSubmit ≥305B vs
  ImportRebase ≥80B）を再現 → 修正後 exit 0
  （`synvelis-workspace` の lint が `large_enum_variant`
  なしで pass）。Windows worktree、rustc 1.99.1。
- `mise run check synvelis-workspace`: exit 0。
- `mise run lint synvelis-workspace`: exit 0。
- `mise run test synvelis-workspace`: exit 0（全 suite
  pass。`malformed_and_implicit_actions_are_typed`・
  `import_committed_live_with_receipt` の `ImportSubmit`
  constructor を含む）。
- `mise run test synvelis-app`: exit 0（47 test binaries
  全て 0 failed。`library_browser.rs` の `syn.library.
  action`/`import.submit` JSON deserialize 経路を含む —
  wire 互換の実測）。
- `mise run fmt-check`: exit 0。
- 補助証拠（canonical = mise）: `cargo tree -p
  synvelis-app --features native -e features -i serde_json`
  で根本原因の feature 経路を確認。
- AC1 の `mise.ci.toml` pin 1.99.0・ubuntu-latest 相当での
  確認は merge 後の対象 SHA の CI 証拠に委譲
  （`evidence/sv488/` に区別を記録）。`*-msrv` 系は
  ユーザー指定により未実行（NotRun）。
- review-protocol: 小規模修正のため diff 精読 + lint/test
  実測による self-review 1 cycle を実施。blocker 0
  （`#[allow]` 不使用・wire form 不変・crate 外変更なし・
  検証は実測のみを確認）。

## Risks

- `serde_json/preserve_order` feature unification が他の
  `serde_json::Value` 保持構造へ与える影響範囲は未棚卸し。
  同種の `large_enum_variant` 再発リスクは残るが、gpui
  依存グラフ由来で synvelis 側の制御外であり、spec の
  Deferred Design Concerns に記録した。
- `ImportSubmit` 以外の variant に大きな `Value` payload が
  追加された場合は再発しうる。再発時は同じく large
  payload の `Box` 化で対処する運用を doc comment に残した。

## Follow-ups

- `serde_json/preserve_order` の feature unification 影響範囲の
  棚卸し（workspace 内の `serde_json::Value` 保持構造の
  サイズ監査）は別 Issue 候補。
- `docs/**/README.md` 管理 index の再生成は Coordinator が
  統合時に実施。
- Issue #488 の Close・merge は Coordinator が行う
  （PR は `Refs #488`）。
