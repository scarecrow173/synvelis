---
id: "SPEC-034YnF7HCn1ybs61M9xuu5"
type: "spec"
status: "implemented"
title: "Issue #488 lint-app-native clippy::large_enum_variant 是正"
created: "2026-11-16"
updated: "2026-11-16"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/488"
    - "https://github.com/scarecrow173/synvelis/actions/runs/43784707716"
  changes:
    added: []
    modified:
      - type: "file"
        path: "crates/synvelis-workspace/src/library_ui.rs"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by:
    - "docs/plans/sv488-lint-app-native.md"
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/discovery/sv488-lint-app-native.md"
  refines: []
  refined-by: []
  derives-from:
    - "https://github.com/scarecrow173/synvelis/issues/488"
    - "docs/discovery/sv488-lint-app-native.md"
  derived-by:
    - "docs/designs/sv488-lint-app-native.md"
    - "docs/test-specs/sv488-lint-app-native.md"
  verifies: []
  verified-by:
    - "docs/test-specs/sv488-lint-app-native.md"
  references:
    - "crates/synvelis-workspace/src/library_ui.rs"
  defers: []
  deferred-by: []
---

# Issue #488 lint-app-native clippy::large_enum_variant 是正

## What Should Be Built

`crates/synvelis-workspace/src/library_ui.rs` の `LibraryAction`
enum に対する `clippy::large_enum_variant` を、lint 属性の追加・
緩和を使わない構造修正で解消し、`mise run lint-app-native`
（`cargo clippy -p synvelis-app --all-targets --features native
-- -D warnings`）が pass する状態にする。

`#[serde(tag = "action")]` の wire form（`"action":
"import.submit"` 等の JSON shape）は変更せず、crate 外
consumer（`synvelis-app` の `op_library_action`）の変更なしで
収まる設計とする。

## Why Now

main `f05bdbf`（PR #487 merge）の Full CI（`ci.yml`、run
43784707716、2026-11-16T01:10Z）が `verify (ubuntu-latest)` の
`Lint synvelis-app native feature` step で失敗し、fail-closed の
`evidence`・`ci-summary` が連鎖失敗した。本失敗は sv#115 等の
対象 SHA における Full-CI 証拠 gate をブロックしている。

原因は `synvelis-app` の `native` feature が `gpui` 経由で
`serde_json/preserve_order` を feature unification し、
`serde_json::Value` が肥大化した結果、`LibraryAction::
ImportSubmit`（≥305B）が第二位 `ImportRebase`（≥80B）より
200B 超大きくなったこと。`lint-app-native` は ubuntu-latest
限定 lane のため windows/macos の verify は success だった。

## Users and Value

- CI 運用者・後続タスクの Worker: Full CI の `verify
  (ubuntu-latest)`・`evidence`・`ci-summary` が復旧し、対象 SHA
  の統合証拠が取得できる。
- `synvelis-app` の `syn.library.action` op 利用者: wire form が
  不変のため、JSON payload の互換性は保たれる。

## Scope

### In Scope

- `LibraryAction::ImportSubmit` の large variant の構造修正
  （`target: Option<ImportTargetWire>` →
  `Option<Box<ImportTargetWire>>`）
- 同 crate 内の構築・pattern match・test の整合
- wire/serde 互換の維持（`#[serde(tag = "action",
  deny_unknown_fields)]`・derive・`#[non_exhaustive]`）
- 本タスクの doc-chain・実装記録・検証証拠の作成

### Out of Scope

- `#[allow]`/`#[expect]` による lint 隠蔽、`workspace.lints` /
  `[lints]` / `mise.toml` / `mise.ci.toml` / `Cargo.toml` /
  `Cargo.lock` の変更
- `crates/synvelis-workspace/` 以外の crate の変更
- 新規依存・feature flag の追加
- `docs/**/README.md` 管理 index の再生成
- `ruviform-dep-access` の fingerprint 注記行への対応
  （本失敗の原因ではない）
- `ci-issue-tracker.yml` 等 `.github/` の変更

## Requirements

- `mise run lint-app-native` が `synvelis-workspace` の
  `LibraryAction` に対し `clippy::large_enum_variant` を出さず
  pass すること。
- `#[serde(tag = "action")]` の wire form を変えないこと。
- `LibraryAction` の derive・`#[non_exhaustive]`・
  `deny_unknown_fields` を維持すること。
- 変更は `crates/synvelis-workspace/`・`evidence/sv488/`・
  doc chain の slug `sv488-lint-app-native` に限定すること。

## Acceptance Criteria

- [x] `mise run lint-app-native` が `clippy::large_enum_variant`
  なしで pass する（ローカル実測。AC1 の ubuntu-latest/
  toolchain 1.99.0 条件は merge 後 CI 証拠に委譲）
- [ ] `ci.yml` の main 向け Full CI が修正後の対象 SHA で
  `verify (ubuntu-latest)`・`evidence`・`ci-summary` まで
  success する（merge 後の CI 証拠で Coordinator が確認）

## Deferred Design Concerns

- `serde_json/preserve_order` feature unification が他の
  `serde_json::Value` 保持構造へ与える影響範囲の棚卸し | gpui
  依存グラフ由来で synvelis 側の制御外 | ruviform/依存構成の
  見直し Issue で扱うとき | 同種の lint 再発リスク |

## Sources and Relations

- Issue: <https://github.com/scarecrow173/synvelis/issues/488>
- CI 失敗 run: <https://github.com/scarecrow173/synvelis/actions/runs/43784707716>
- 混入元: PR scarecrow173/synvelis#485（ALIB-05、`LibraryAction`
  追加）— merge 済み、main `f05bdbf` に存在
- 調査記録: `docs/discovery/sv488-lint-app-native.md`
- 設計: `docs/designs/sv488-lint-app-native.md`
