---
id: "EXP-034YnNeGvNqVgT4B8hWzKd"
type: "experiment"
status: "completed"
title: "Issue #488 lint-app-native clippy::large_enum_variant 是正の試行 (synvelis#488)"
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
        path: "crates/synvelis-workspace/src/library_ui.rs"
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by:
    - "docs/impl/ir/sv488-lint-app-native.md"
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/sv488-lint-app-native.md"
    - "docs/test-specs/sv488-lint-app-native.md"
    - "docs/impl/exp/sv488-lint-app-native-exp.jsonl"
    - "evidence/sv488/"
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
---

# Issue #488 lint-app-native clippy::large_enum_variant 是正の試行

## 問題設定

main `f05bdbf` の Full CI（run 43784707716）で `verify
(ubuntu-latest)` の `mise run lint-app-native` が
`clippy::large_enum_variant`（`library_ui.rs:454` の
`LibraryAction`、ImportSubmit ≥305B vs ImportRebase ≥80B）で
失敗し、`evidence`/`ci-summary` が連鎖失敗した。同一 lint が
`lint synvelis-workspace` 単独では発火しない非対称性の原因
特定と、wire form を維持した構造修正方針の決定が試行の核心
だった。

## 試行と結果

### E1: 失敗の再現と lane 依存性の確認

ローカル Windows worktree（rustc 1.99.1、mise 管理）で
`mise run lint-app-native`（`cargo clippy -p synvelis-app
--all-targets --features native -- -D warnings`）を実行し、
CI と同一の lint・同一のサイズ値（ImportSubmit ≥305B /
ImportRebase ≥80B / enum ≥312B）で exit 101 を再現した。
一方 `mise run lint synvelis-workspace` 単独は exit 0 で
非再現 — 失敗が `synvelis-app` の feature 組立てに依存する
ことを確認した。

### E2: 根本原因の特定（feature unification）

`cargo tree -p synvelis-app --features native -e features -i
serde_json` で `serde_json feature "preserve_order"` ←
`gpui_perf` ← `gpui_util_macros` ← `gpui` ←
`ruviform-ui-gpui` ← `synvelis-app feature "native"` の経路を
確認した。`preserve_order` で `serde_json::Map` が
`BTreeMap`→`IndexMap` に切り替わり `Value` が肥大化、
`ImportSubmit` が保持する 3 個の `Value`（`import_id` と
`target.placement`/`target.playback`）経由で variant サイズが
閾値を超えたと結論付けた。

### E3: 修正粒度の比較と採用

候補を比較した。(a) `target` field の
`Option<Box<ImportTargetWire>>` 化 — 最小 diff、serde 透過、
variant の支配的 payload（`ImportTargetWire` ≈ 2 String +
2 Value）をヒープへ逃がせる。(b) variant 全体の
`ImportSubmit(Box<Payload>)` newtype 化 — より確実だが
internally-tagged enum の newtype variant と
`deny_unknown_fields` の相互作用検証が必要で diff も大きい。
(c) `#[allow]`/lint 緩和 — Issue 受入条件で禁止。
(a) を採用し、`ImportSubmit.target` を
`Option<Box<ImportTargetWire>>` に変更、`apply()` match arm を
`target.as_deref()` に整合した。`import_submit`/
`decode_target` のシグネチャと wire form は不変。

### E4: 検証

Windows worktree（rustc 1.99.1）で実施: `mise run
lint-app-native` exit 0（`large_enum_variant` 消滅）、
`mise run check/lint/test synvelis-workspace` pass、
`mise run test synvelis-app` pass（47 test binaries 全て
0 failed、`library_browser.rs` の `import.submit` JSON
deserialize 経路を含む）、`mise run fmt-check` pass。
`mise.ci.toml` pin 1.99.0・ubuntu-latest 条件の AC1/AC2 は
merge 後の対象 SHA の CI 証拠に委譲する。

## 採用した判断

- **Box 化の粒度は field 単位**: variant 全体の Box 化は
  serde の internally-tagged newtype variant 挙動の追加検証を
  要し、diff も大きい。`target` が variant の支配的 payload
  であり field 単位の Box で閾値を十分下回るため最小の整合的
  変更を採用した。
- **wire form 優先**: `Option<Box<T>>` は serde に透過的で
  `{"action":"import.submit", ..., "target": {...}}` の JSON
  shape が不変。crate 外 consumer（`synvelis-app` の
  `op_library_action`）は無変更で済むことを `test
  synvelis-app` で実測した。
- **lint 隠蔽の排除**: `#[allow]`/`#[expect]`・lints 緩和は
  Issue 受入条件で禁止のため不使用。`target` field の doc
  comment に Box 化の理由（feature unification 下での
  `Value` 肥大化）を記録し、将来の安易な戻しを防いだ。

## 結論

`serde_json/preserve_order` feature unification（gpui 経由）
による `serde_json::Value` 肥大化が `large_enum_variant` 発火の
根本原因と特定し、`ImportSubmit.target` の `Box` 化で構造修正
した。`lint-app-native` を含む変更範囲の検証は全て pass。
ubuntu-latest/toolchain 1.99.0 での AC1/AC2 は merge 後の
対象 SHA の CI 証拠で Coordinator が確認する残件である。
