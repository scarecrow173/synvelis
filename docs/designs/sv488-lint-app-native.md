---
id: "DESIGN-034YnFdVxCjVKSDV4JZaWa"
type: "design"
status: "approved"
title: "Issue #488 lint-app-native clippy::large_enum_variant 是正 — 設計"
created: "2026-11-16"
updated: "2026-11-16"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/488"
  changes:
    added: []
    modified:
      - type: "file"
        path: "crates/synvelis-workspace/src/library_ui.rs"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/sv488-lint-app-native.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/discovery/sv488-lint-app-native.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/sv488-lint-app-native.md"
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

# Issue #488 lint-app-native clippy::large_enum_variant 是正 — 設計

## Context

main `f05bdbf`（PR #487 merge）の Full CI（`ci.yml`、run
43784707716、2026-11-16T01:10Z）が `verify (ubuntu-latest)` の
`Lint synvelis-app native feature (mise run lint-app-native)` で
失敗した。`cargo clippy -p synvelis-app --all-targets --features
native -- -D warnings` が workspace member の path 依存
`synvelis-workspace` に対しても lint を適用し、
`crates/synvelis-workspace/src/library_ui.rs:454` の
`LibraryAction` enum で `clippy::large_enum_variant`（error、
`-D warnings` 昇格）を報告した。最大 variant `ImportSubmit`
（≥305B）と第二位 `ImportRebase`（≥80B）のサイズ差が閾値を
超え、`evidence`・`ci-summary` が fail-closed で連鎖失敗した。

失敗が `lint-app-native` lane 限定で `lint synvelis-workspace`
単独では再現しない理由は feature unification にある。
`synvelis-app` の `native` feature が引く `ruviform-ui-gpui` →
`gpui` → `gpui_perf` が `serde_json/preserve_order` を有効化し、
`serde_json::Map` が `BTreeMap` から `IndexMap` に切り替わる。
これにより `serde_json::Value`（およびそれを 3 個含む
`ImportSubmit`/`ImportTargetWire`）が肥大化し、variant サイズ差が
`clippy::large_enum_variant` の閾値（既定 200B）を超える。
`cargo tree -p synvelis-app --features native -e features -i
serde_json` で `gpui_perf → serde_json/preserve_order` の経路を
確認済み。

## Scope

### In Scope

- `LibraryAction::ImportSubmit` variant の構造修正
  （`target: Option<ImportTargetWire>` →
  `Option<Box<ImportTargetWire>>`）と、それに伴う同 crate 内の
  構築・pattern match・test の整合
- `#[serde(tag = "action", deny_unknown_fields)]` の wire form
  （`"action": "import.submit"` 等の JSON shape）を変えないこと
- 本タスクの doc-chain・Implementation Record・Experiment Log・
  evidence の作成

### Out of Scope

- `#[allow]`/`#[expect]` による lint 隠蔽、`workspace.lints` /
  `[lints]` / `mise.toml` / `mise.ci.toml` / `Cargo.toml` /
  `Cargo.lock` の変更
- `crates/synvelis-workspace/` 以外の crate の変更
  （`synvelis-app` の `op_library_action` は serde deserialize
  のみで wire 互換が保たれるため変更不要）
- 新規依存・feature flag の追加
- `docs/**/README.md` 管理 index の再生成（Coordinator が統合時に
  実施）
- gpui/serde_json feature unification 自体の見直し（ruviform 側の
  依存特性であり本タスクの対象外）

## Components and Boundaries

- `crates/synvelis-workspace/src/library_ui.rs`:
  `LibraryAction` enum が唯一の変更点。`ImportSubmit` の
  `target` field を `Option<Box<ImportTargetWire>>` に変更する。
  `Box` は serde に対して透過的（`Option<Box<T>>` と
  `Option<T>` は同一の JSON shape に serialize/deserialize
  される）ため wire 契約は不変。
- `apply()` の match arm: `target.as_ref()` →
  `target.as_deref()` で `Option<&ImportTargetWire>` を維持し、
  `import_submit`/`decode_target` のシグネチャは不変。
- `crates/synvelis-app/src/ui_ops.rs` `op_library_action`:
  `serde_json::from_value` で `LibraryAction` を decode する
  唯一の crate 外 consumer。wire form 不変のため無変更。

## Data and Control Flow

1. CI/ローカルの `cargo clippy -p synvelis-app --features native`
   が feature unification で `serde_json/preserve_order` を有効化
   し、`serde_json::Value` を含む `LibraryAction::ImportSubmit`
   が肥大化する。
2. `clippy::large_enum_variant` が最大 variant と第二位のサイズ差
   （≥225B）を検出し、`-D warnings` で error 化する。
3. 修正後は `target` が `Box`（8B）になり、`ImportSubmit` の実サイズが
   `import_id: Value` + `locator: String` + `Option<u64>` + `bool`
   程度まで縮小し、第二位 `ImportRebase` とのサイズ差が閾値未満に
   収まる。

## Non-Functional Constraints

- wire/serde 互換: `#[serde(tag = "action", deny_unknown_fields)]`、
  各 variant の `#[serde(rename = "...")]`、field の
  `#[serde(default)]` をすべて維持する。`Box` 化は JSON shape に
  影響しない。
- derive（`Clone, Debug, PartialEq, Serialize, Deserialize`）と
  `#[non_exhaustive]` を維持する。
- `import_submit`/`decode_target` の内部ロジック・公開 API
  シグネチャを変えない。
- lint を `#[allow]` で隠さない（Issue 受入条件）。

## Risks and Trade-offs

- `target` 以外の field 肥大化で再発するリスク → 現行の第二位
  `ImportRebase`（`Value` + `u64`）は `ImportSubmit` 修正後も
  最大でも ~100B 規模であり、200B 差閾値に対し余裕がある。
  再発時は同じく large payload の `Box` 化で対処する。
- variant 全体を `Box` 化する案（`ImportSubmit(Box<Payload>)`）は
  より確実だが、serde の internally-tagged enum における newtype
  variant + `deny_unknown_fields` の相互作用検証が必要で diff も
  大きい。`target` の `Box` 化で閾値を十分下回ることが実測で
  確認できたため、最小の整合的変更を採用した。
- `ImportTargetWire` の `placement`/`playback` が `serde_json::Value`
  であること自体は wire 契約であり、本タスクでは型を変えない。

## Deferred Design Concerns

- `serde_json/preserve_order` が workspace 全体の `serde_json::Value`
  サイズを変える feature unification の影響範囲の棚卸し | gpui
  依存グラフ由来で synvelis 側の制御外 | ruviform/依存構成の見直し
  Issue で扱うとき | 他の `Value` 保持構造も同様に肥大化しうる |

## Verification Notes

- [x] `mise run lint-app-native` 修正前の失敗再現（`large_enum_variant`
  で exit 101 — `evidence/sv488/` に記録）
- [x] `mise run check synvelis-workspace` / `mise run lint
  synvelis-workspace` / `mise run test synvelis-workspace` /
  `mise run test synvelis-app` / `mise run fmt-check`
  （実行結果は `evidence/sv488/` と PR 本文に記録）
- [ ] `mise.ci.toml` pin の 1.99.0・ubuntu-latest 相当での確認は
  merge 後の対象 SHA の CI 証拠に委譲（AC1）
