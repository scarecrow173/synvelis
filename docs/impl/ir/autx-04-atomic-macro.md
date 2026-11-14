---
id: "IMPL-034XgQ1qM5AZJvhmAnHddj"
type: "impl"
status: "in-progress"
title: "AUTX-04 Atomic Macro v1 recipe compiler and ID resolution"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/263"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-automation/src/macros.rs"
      - type: "file"
        path: "crates/synvelis-automation/tests/macros.rs"
      - type: "file"
        path: "docs/impl/exp/autx-04-atomic-macro.md"
      - type: "file"
        path: "docs/test-specs/autx-04-atomic-macro.md"
      - type: "file"
        path: "evidence/sv263/autx-04-atomic-macro-ledger.md"
    modified:
      - type: "file"
        path: "crates/synvelis-automation/src/compiler.rs"
      - type: "file"
        path: "crates/synvelis-automation/src/definition.rs"
      - type: "file"
        path: "crates/synvelis-automation/src/error.rs"
      - type: "file"
        path: "crates/synvelis-automation/src/expr.rs"
      - type: "file"
        path: "crates/synvelis-automation/src/frontend.rs"
      - type: "file"
        path: "crates/synvelis-automation/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-automation/tests/compiler.rs"
      - type: "file"
        path: "crates/synvelis-gateway/src/prepare_engine.rs"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on:
    - "docs/automation/design-atomic-macro.md"
    - "docs/impl/ir/autx-05-workflow-dsl.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/exp/autx-04-atomic-macro.md"
    - "docs/test-specs/autx-04-atomic-macro.md"
    - "evidence/sv263/autx-04-atomic-macro-ledger.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/autx-04-atomic-macro.md"
  references:
    - "docs/automation/design-atomic-macro.md"
    - "crates/synvelis-gateway/src/prepare_engine.rs"
    - "crates/synvelis-gateway/src/prepare.rs"
    - "crates/synvelis-gateway/src/request.rs"
    - "crates/synvelis-gateway/src/registry.rs"
    - "crates/synvelis-gateway/src/schema.rs"
    - "crates/synvelis-gateway/src/snapshot.rs"
    - "crates/synvelis-project/src/ops.rs"
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted:
      - "docs/impl/exp/autx-04-atomic-macro.md"
    rejected: []
---

# AUTX-04 Atomic Macro v1 recipe compiler and ID resolution

## Summary

Issue #263（AUTX-04）の headless 実装スライスとして、
`synvelis-automation` に versioned
`AtomicMacroDefinition`（Recipe）モデルと
`MacroCompiler`/`MacroRegistry` を追加した。Recipe は
authority 非保持の静的定義であり、compile 時に runtime
input + pinned revision と共に `synvelis.operation.batch`
の `ops[]` へ lowering され、既存 GW-04 preparation /
GW-05 approval / GW-06 commit・undo 経路にそのまま載る。
第 2 の transaction/approval/commit/undo 系は導入しない。
create-then-reference は予約レンジ placeholder +
`bindings` 宣言として compile されるが、Gateway 側は
`Composite::validate` の逐次化 seam（`synvelis-project`、
別リース）が未着のため、非空 `bindings` を持つ batch を
prepare で typed reject する fallback を採用した
（`docs/automation/design-atomic-macro.md` §3）。

## Context

`docs/automation/design-atomic-macro.md` が compile 契約を
固定: Recipe = `{schema_version, macro_id, version,
parameters, steps: Vec<NamedAction>, output_bindings,
policy}`、capability は exact `(id, version)`、target は
明示、member ≤64、nested batch 禁止、`ProjectMutation`
capability のみ、async/Query/外部 I/O は Workflow 責務。
create-then-reference は named `OutputBinding` +
project/controller と同一の決定的 ID 採番列を使い、ID の
推測・ハードコードは禁止。preview は GW-04 scratch/derived
効果のみ、apply は既存 approval/commit gate を通る。

`ProjectOp::Composite::apply` は member 毎に evolving
scratch で逐次 validate+apply するが、`Composite::validate`
は全 member を pre-state に対して validate し、
`ProjectController::apply` が apply 前に呼ぶ
（`crates/synvelis-project/src/ops.rs`）。先行 member が
作る ID を後続 member が参照する場合、commit 時に
`NotFound` で必ず失敗する。この seam は `synvelis-project`
のリース（sv#290）にあり、本 slice のリース外のため
Coordinator 判断で fallback が確定した。

## Implementation

### Changes

- `crates/synvelis-automation/src/expr.rs` — `Expr` に
  `Param { path }` / `Output { name }` variant を追加。
  serde は `{"expr":"param","path":[...]}` /
  `{"expr":"output","name":"..."}`。workflow 側は
  `MacroOnlyExpr` で fail-closed reject。
- `crates/synvelis-automation/src/macros.rs`（新規）—
  `AtomicMacroDefinition`/`NamedAction`/`OutputBinding`/
  `MacroPolicy` の型と decode-time validation、canonical
  JSON + SHA-256 の `definition_digest`、`MacroRegistry`
  （exact `(macro_id, version)` 解決、unknown/unsupported
  version を区別、latest 解決なし）、`MacroCompiler`。
  compile は static check（capability exact version・
  domain=project-mutation・execution≠async・target 契約・
  引数 schema 適合・macro-only expr の workflow 混入拒否）
  → runtime parameter binding（missing/unexpected/schema/
  予約レンジ literal 拒否）→ output binding 検証 →
  flat member `ops[]` 生成。`$output.<name>` は
  `OUTPUT_BINDING_ID_BASE` レンジの決定的 placeholder ID
  （`kind-<32hex>` 形式維持）を emit し、実 ID は一切
  推測しない。`CompiledMacro::batch_args()` が Gateway
  batch request の `args` 形状（`ops` + 非空時 `bindings`）
  を返す。
- `crates/synvelis-automation/src/compiler.rs` —
  `Step::AtomicMacro` を `compile_with_macros` で展開。
  registry なしは `UndefinedMacro`、registry ありで ID
  不在は `UnknownMacro`、version 非対応は
  `UnsupportedMacroVersion`。call-site 引数は macro
  signature で schema 照合し、member の step cost・effect・
  data-access・capability ref を plan bounds に会計。
  macro-level target は reject（macro は project mutation
  列を内包し call-level target を取らない）。
- `crates/synvelis-automation/src/frontend.rs` — YAML/JSON
  macro definition の lowering。`$param.<name>` /
  `$output.<name>` 参照 grammar、macro step の
  `{id, capability, args?, target?}` 形状、未知・不正な
  `$` 参照の reject。
- `crates/synvelis-automation/src/{definition,error,lib}.rs`
  — macro 関連型の export と `CompileError` 追加
  （`UndefinedMacro`/`UnknownMacro`/`UnsupportedMacroVersion`/
  `MacroOnlyExpr`/`ReservedIdLiteral` 等）。
- `crates/synvelis-gateway/src/prepare_engine.rs` —
  `synvelis.operation.batch` input schema に optional
  `bindings`（array, max 64, any_obj items）を追加し、
  `lower_named` の batch arm 先頭で非空 `bindings` を
  `GatewayError::PreparationRejected`（reason: intra-
  composite id resolution awaits GW-04 scratch-id seam、
  Workflow 分解を推奨）で typed reject。`bindings` 欠落・
  空配列は従来通り member lowering へ進む。
- `crates/synvelis-automation/tests/macros.rs`（新規）—
  17 integration tests。decode/encode round-trip・
  schema/version/shape reject・policy ceiling・決定的
  compile・flat ops・placeholder/binding 形状・
  `batch_args()` wire 形状・capability skew・非 mutation
  domain・target 契約・param binding・予約 ID・output
  binding・macro-only expr・registry exact version・
  plan 展開・registry なし fail-closed。
- `crates/synvelis-gateway/src/prepare_engine.rs` tests —
  `batch_bindings_shape_is_schema_enforced` /
  `batch_output_bindings_reject_typed_unsupported` /
  `batch_without_bindings_still_lowers` の 3 件。

### Verification

- `cargo test -p synvelis-automation`: 99 tests PASS
  （unit 1・compiler 43・decode 11・determinism 12・
  macros 17・yaml-subset 15）。
- `cargo test -p synvelis-gateway`: 全 26 test binaries
  PASS、0 failure（新規 3 件含む、batch schema 拡張の
  非破壊性を確認）。

## Deferred

- `Composite::validate` の逐次 dry-run 化（または同等の
  intra-composite ID 解決 seam）— `synvelis-project` の
  リース（sv#290）完了後の後続 slice。完了まで非空
  `bindings` batch は prepare で typed reject。
- GW-04 scratch rewrite（placeholder → 実 ID）と
  preview/commit ID の CAS 整合 — 上記 seam 依存。
- Macro GUI 統合（#266）、OS 実験（#267）、Action Script
  lowering（#272）、Workflow Script owner（#273）、
  coordinator/runtime owner（#265）。
