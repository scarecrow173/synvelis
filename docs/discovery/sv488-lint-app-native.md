---
id: "DISC-034YnF74eHyeIdOhozJWeH"
type: "discovery"
status: "resolved"
title: "Issue #488 lint-app-native clippy::large_enum_variant — LibraryAction ImportSubmit variant の肥大化原因調査"
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
    - "docs/specs/sv488-lint-app-native.md"
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/sv488-lint-app-native.md"
    - "evidence/sv488/"
  refines: []
  refined-by: []
  derives-from: []
  derived-by:
    - "docs/specs/sv488-lint-app-native.md"
  verifies: []
  verified-by: []
  references:
    - "crates/synvelis-workspace/src/library_ui.rs"
    - "crates/synvelis-app/src/ui_ops.rs"
    - "crates/synvelis-app/Cargo.toml"
    - "mise.toml"
    - ".github/workflows/ci.yml"
  defers: []
  deferred-by: []
---

# Issue #488 lint-app-native clippy::large_enum_variant — LibraryAction ImportSubmit variant の肥大化原因調査

## Summary

CI run 43784707716（main `f05bdbf`）で `verify (ubuntu-latest)`
の `mise run lint-app-native` が `clippy::large_enum_variant`
（`library_ui.rs:454` の `LibraryAction`、ImportSubmit ≥305B vs
ImportRebase ≥80B）で失敗した。原因は `synvelis-app` の `native`
feature が `gpui → gpui_perf` 経由で `serde_json/preserve_order`
を feature unification し、`serde_json::Value` が肥大化して
variant サイズ差が lint 閾値を超えたこと（confirmed）。
`target: Option<ImportTargetWire>` の `Box` 化で構造修正し、
wire form を維持したまま解消する方針で spec へ昇格した。

## Exploration Goal

Issue #488: Full CI の `lint-app-native` 失敗の原因特定と修正
方針の決定。`evidence`/`ci-summary` の失敗が独立した defect か
fail-closed 連鎖かを切り分け、synvelis 側・ruviform 依存・
CI 環境のどこに root cause があるかを特定する。

## Key Issues and Assumptions

- `cargo clippy -p synvelis-app --all-targets --features native`
  は workspace member の path 依存 `synvelis-workspace` にも
  lint を適用する（confirmed: ローカル Windows で同一 lint・
  同一サイズ値 ≥305B/≥80B を再現）。
- `synvelis-app` の `native` feature → `dep:ruviform-ui-gpui` +
  `ruviform-ui-gpui/gpui`（confirmed: `crates/synvelis-app/
  Cargo.toml`）。
- `cargo tree -p synvelis-app --features native -e features -i
  serde_json` で `serde_json feature "preserve_order"` ←
  `gpui_perf` ← `gpui_util_macros` ← `gpui` ←
  `ruviform-ui-gpui` ← `synvelis-app/native` の経路を確認
  （confirmed）。`preserve_order` で `serde_json::Map` が
  `IndexMap` 化し `Value` が肥大化する。
- `mise run lint synvelis-workspace` 単独では lint が発火しない
  （confirmed: exit 0）。`preserve_order` を含まない feature
  unification では `Value` が小さく variant サイズ差が閾値未満。
- `LibraryAction` の crate 外 consumer は `synvelis-app` の
  `op_library_action`（`serde_json::from_value` による
  deserialize のみ）で、wire form が不変なら変更不要
  （confirmed: `ui_ops.rs:5033`）。
- `ImportSubmit`/`ImportTargetWire` を構築するコードは
  `library_ui.rs` 内の `apply()` match arm と test constructor
  （`target: None` のみ）に限定される（confirmed: crate 全体の
  grep）。

## Alternatives and Comparison

| Option | Pros | Cons | Lean |
| --- | --- | --- | --- |
| `target` を `Option<Box<ImportTargetWire>>` に Box 化 | 最小 diff、serde 透過で wire 不変、variant が ~200B 縮小 | `Value` を他に持つ variant 追加時は再発しうる | yes |
| variant 全体を `Box` 化（`ImportSubmit(Box<Payload>)`） | variant サイズが構造的に 8B で将来も安全 | internally-tagged enum の newtype variant + `deny_unknown_fields` の相互作用検証が必要、diff 大 | no |
| `#[allow(clippy::large_enum_variant)]` | 1 行 | Issue 受入条件で禁止（構造修正必須） | no |
| `workspace.lints`/`[lints]` 緩和 | 広く効く | 同上・検証弱体化禁止 | no |
| gpui/serde_json feature unification の見直し | 根本原因に近い | ruviform 側依存・synvelis の lease 外 | no |

## Tentative Conclusions and Hypotheses

- confirmed: `preserve_order` による `serde_json::Value` 肥大化が
  `large_enum_variant` 発火の直接原因。`ImportSubmit` は
  `import_id: Value` + `target`（`ImportTargetWire` 内に
  `placement`/`playback` の 2 `Value`）で計 3 `Value` を保持する。
- confirmed: `target` の `Box` 化で variant の支配的 payload
  （`ImportTargetWire` ≈ 2 String + 2 Value ≈ 190B+）を
  ヒープへ逃がせる。第二位 `ImportRebase`（`Value` + `u64`、
  ~80B）とのサイズ差は閾値 200B を十分下回る見込み。
- hypothesis: `preserve_order` 非有効時（`lint
  synvelis-workspace` 単独）でも `Box` 化は serde 透過で
  wire form・挙動ともに不変（`test synvelis-workspace` で実測）。

## Open Questions

- `serde_json/preserve_order` の feature unification が他の
  `serde_json::Value` 保持構造（workspace 全体）に与える影響は
  未棚卸し — 別 Issue 候補として spec の Deferred Design
  Concerns に記録。

## Promotion Candidates

- [x] spec-doc needed: `docs/specs/sv488-lint-app-native.md`
- [ ] adr-doc needed: なし（lint 是正の局所的構造修正であり
  アーキテクチャ判断を伴わない）

## Sources and Relations

- https://github.com/scarecrow173/synvelis/issues/488
- https://github.com/scarecrow173/synvelis/actions/runs/43784707716
- 混入元: PR scarecrow173/synvelis#485（ALIB-05 Asset Library
  Browser/Import UX、`LibraryAction` 追加）
- `cargo tree -p synvelis-app --features native -e features -i
  serde_json` の feature 経路出力

## Evidence

- 2026-11-16: CI run 43784707716 の `verify (ubuntu-latest)` で
  `Lint synvelis-app native feature (mise run lint-app-native)`
  が `large size difference between variants`（`library_ui.rs:
  454`、ImportSubmit ≥305B / ImportRebase ≥80B / enum ≥312B）
  を報告し exit 101。`evidence`・`ci-summary` が連鎖失敗。
- 2026-11-16: ローカル Windows worktree（rustc 1.99.1）で
  `mise run lint-app-native` を実行し、同一 lint・同一サイズ値で
  再現（exit 101）。`mise run lint synvelis-workspace` 単独は
  exit 0 で非再現 — feature unification 依存を確認。
- 2026-11-16: `cargo tree -p synvelis-app --features native -e
  features -i serde_json` で `gpui_perf → serde_json feature
  "preserve_order"` の経路を確認。
