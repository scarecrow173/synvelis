---
id: "IMPL-034Xe5swowhuVJIrpCp969"
type: "impl"
status: "in-progress"
title: "AUTX-05 Workflow DSL v1 — typed definition and plan compiler"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/264"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-automation/"
      - type: "file"
        path: "docs/impl/exp/autx-05-workflow-dsl.md"
      - type: "file"
        path: "docs/test-specs/autx-05-workflow-dsl.md"
      - type: "file"
        path: "evidence/sv264/autx-05-workflow-dsl-ledger.md"
    modified:
      - type: "file"
        path: "Cargo.toml"
      - type: "file"
        path: "Cargo.lock"
      - type: "file"
        path: "tools/check-headless-deps.mjs"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on:
    - "docs/designs/execution-plan-design.md"
    - "docs/specs/command-gateway.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/exp/autx-05-workflow-dsl.md"
    - "docs/test-specs/autx-05-workflow-dsl.md"
    - "evidence/sv264/autx-05-workflow-dsl-ledger.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/autx-05-workflow-dsl.md"
  references:
    - "docs/designs/execution-plan-design.md"
    - "docs/specs/command-gateway.md"
    - "crates/synvelis-gateway/src/registry.rs"
    - "crates/synvelis-gateway/src/schema.rs"
    - "crates/synvelis-gateway/src/snapshot.rs"
    - "crates/synvelis-gateway/src/request.rs"
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted:
      - "docs/impl/exp/autx-05-workflow-dsl.md"
    rejected: []
---

# AUTX-05 Workflow DSL v1 — typed definition and plan compiler

## Summary

Issue #264（AUTX-05）の headless 実装スライスとして、新規 crate
`synvelis-automation` を追加した。Versioned YAML Workflow DSL v1
を安全な YAML サブセット parser で読み、source 非依存の型付き
`WorkflowDefinition` へ lowering し、共通 `PlanCompiler` が
Capability Version/Availability・Schema・Target・Effect・
Data Access・静的 Budget を fail-closed で検証して決定的な
`ExecutablePlan`（digest 付き）を生成する。Authority
（principal/grant/approval/credential/shell/eval/script）は
Definition・Plan のどちらにも表現不能であり、実行時の
再検証は Gateway/Project/Job 側の責務として本 crate の外に
置いた。

## Context

`docs/designs/execution-plan-design.md` の AUTX lane が固定する
#264 → #265 start 関係と、Issue #264 本文の「定義」で固定され
た `WorkflowDefinition`/Typed Step/`StepOutputRef`/
`PlanCompiler`/`ExecutablePlan` のフィールド境界が契約。
`synvelis-gateway` の `registry.rs`/`schema.rs`/`snapshot.rs`/
`request.rs` は read-only の参照先であり、本 crate は Gateway
へ依存せず、serialized Registry Snapshot として型を mirror
する。Macro 定義/Registry/展開は #263、Luau/Rhai Builder は
#273、Run/Step FSM・Dry Run は #265、live Gateway adapter は
統合 slice のスコープであり、本 slice では AST 参照
（`AtomicMacro` step）と reject 経路の予約のみ行った。

## Implementation

### Changes

- `crates/synvelis-automation/`（新規 headless crate、
  `publish = false`、workspace lints/edition/rust-version 継承）:
  - `yaml.rs` — 安全サブセット YAML parser。block mapping/
    sequence・quoted/plain scalar・制限付き implicit 型・comment
    のみ受理し、anchor/alias/tag/block scalar/merge key/
    flow collection/multi-document/directive/duplicate key/
    非 canonical 数値/過剰な深さ・サイズを fail-closed で
    reject。1 MiB・深さ 64・200k node・scalar 64 KiB の
    parser budget を強制。
  - `frontend.rs` — YAML → tagged expr JSON → canonical decode
    の lowering。`$input`/`$steps`/`$item` 参照 grammar を
    検証し、不正・未知参照を reject。
  - `definition.rs`/`expr.rs`/`refs.rs` — versioned
    `WorkflowDefinition`（`schema_version`/`workflow_id`/
    `inputs`/`outputs`/`limits`/`steps`）、型付き Step AST
    （Query/AtomicMacro/Invoke/AsyncSubmit/AwaitJob/If/
    ForEachBounded/EmitResult）、条件式 AST
    （Eq/Ne/Lt/Lte/Gt/Gte/Exists/And/Or/Not）、`Expr`
    （literal/input/step-output/item/array/object）、
    `StepOutputRef`、`VersionedRef`。全 decode で
    `deny_unknown_fields`。
  - `schema.rs` — Gateway `ValueSchema` mirror の `SchemaNode`、
    `StaticType`、`ResourceKind`/tagged ID 検証、
    `validate_value`（concrete input）、`type_compatible`
    （static 式照合、Optional semantics 含む）。
  - `registry.rs` — serialized Registry Snapshot mirror
    （capabilities + schemas）。schema_version 検証、
    duplicate 登録 reject、`(id, version)` 完全一致解決のみ
    （latest 解決なし、unknown と unsupported version を区別）。
  - `compiler.rs` — `PlanCompiler::compile(definition, inputs,
    registry)`。definition/registry/input 検証 → 静的 ceiling
    → step 走査（scope/順序/budget 会計）→ output emit 検証 →
    effect/data-access ceiling → deterministic
    `ExecutablePlan` 生成。`Cost{steps,iters,depth}` で
    branch は worst-case max・bounded loop は iteration 数
    倍・sequence は和を取り、宣言 limit 未満の過小申告を
    `BudgetUnderestimated` で reject。`step_order` は
    pre-order で決定的、capability ref は `id@major.minor`、
    effect/data-access bound は sort 済み union。
  - `canon.rs` — canonical JSON（key sort・compact）と
    SHA-256 hex digest。`definition_digest` は canonical
    definition、`ExecutablePlan::digest()` は canonical plan。
  - `error.rs` — `AutomationError`/`CompileError`。全 variant
    が Display message を持ち、decode/compile の失敗を
    fail-closed に分類。
- workspace 配線: root `Cargo.toml` の `members` に
  `crates/synvelis-automation` を追加、`Cargo.lock` を cargo
  で再生成、`tools/check-headless-deps.mjs` の
  `HEADLESS_CRATES` に追加（GUI 依存なしを機械検証）。
- tests: `tests/common/mod.rs`（fixture registry + helper）、
  `yaml_subset.rs`(15)、`decode.rs`(1)、`compiler.rs`(43)、
  `determinism.rs`(12) — 計 82 test。

### Decisions During Implementation

- **YAML parser を自前実装**: serde_yaml 系の一般 YAML
  dependency は workspace 外部依存追加の許可が必要であり、
  かつ tag/alias/implicit 型/anchor を構造的に排除するには
  制限付き parser が契約に合致するため、安全サブセットを
  fail-closed で実装した（採用: exp E1）。
- **Gateway 非依存の snapshot mirror**: registry/schema/
  capability/target/effect/data-access の契約型を本 crate 内に
  serialize 可能な mirror として定義し、live Gateway への
  依存・adapter を持たない（headless 境界の維持）。fixture は
  tests 内の serialized snapshot literal。
- **source 非依存 compiler**: YAML frontend は
  `WorkflowDefinition` への lowering のみを行い、Plan
  検証・compile は typed definition のみに依存する。
  `plan_compiler_is_source_agnostic` test で JSON decode 経路
  から同一 compiler へ到達することを固定（Luau/Rhai Builder
  同一性の本検証は #273 で NotRun/Deferred）。
- **静的 budget 会計**: `Cost{steps,iters,depth}` を
  `seq`(和/max depth)/`branch`(max)/`loop`(乗算) で合成し、
  宣言 `WorkflowLimits` 未満の worst-case を
  `BudgetUnderestimated`、compiler ceiling 超過を
  `BudgetExceeded` で reject。runtime/output/memory は
  plan budget として carry forward。
- **emit-result 規則**: declared output は top-level で
  exactly-once emit を要求し、undeclared/重複/nested emit・
  missing emit を reject。control step 自体は output を
  持たず、`$steps.<id>` 参照は leaf step の output schema
  のみを指す。
- **`OutputTypeMismatch` wrap の限定**: schema 系 error のみ
  `OutputTypeMismatch` に wrap し、`UnknownStepId`/
  `ForwardStepReference`/`ItemOutOfScope` 等の参照系 error を
  温存する（最初の wrap 実装が参照 error を呑み込む bug を
  test で検出・修正）。
- **multi-document `---` の reject**: doc 開始後の `---` を
  silent skip していた parser bug を、2 個目の document
  marker として reject に修正。
- **review-protocol で検出・修正した YAML parser 欠陥**
  （`yaml.rs`、adversarial pass）:
  - single-quoted scalar が byte 単位で `char` 化され、
    非 ASCII（`café`/日本語等）が mojibake 化 → UTF-8
    scalar 単位の copy に修正（double-quoted と同じ方式）。
  - inline comment の strip が map value にのみ適用され、
    sequence item（`- foo # bar`）で comment が値へ混入・
    quoted item は誤 reject → line scan 時に quote-aware
    `strip_comment` を一括適用する形へ集約。
  - tab indentation 検査が dead code（`leading` が常に
    空白のみで `contains('\t')` が不成立）→ leading
    whitespace を space/tab 両方で走査して tab を reject。
  - `...` end-of-document の後続行判定が行 index を byte
    offset として使用 → `split('\n').skip(idx+1)` へ修正し、
    `...` を正しく受理（後続に内容があれば multi-doc reject）。
  いずれも regression test を `yaml_subset.rs` に追加
  （`single_quoted_preserves_utf8`・`comments_strip_everywhere`・
  `rejects_tab_indentation`・`end_of_document_marker`）。

## Related Experiments

### Adopted

- `docs/impl/exp/autx-05-workflow-dsl.md` — YAML parser 方針・
  Gateway mirror・budget 会計・digest canonicalization の各
  試行と結果。全て採用。

### Rejected

- なし。

## Validation

正規検証（`mise` task、Windows 11・Rust 1.99.1・mbx managed
target）:

- `mise run fmt-check` — Pass（exit 0）
- `mise run check synvelis-automation` — Pass（exit 0）
- `mise run lint synvelis-automation` — Pass（exit 0、
  clippy warning 0。`too_many_lines` は repo 先例
  （`outcome.rs`/`schema.rs`/`prepare_engine.rs`）に倣い
  flat dispatch 関数のみ justification 付き `#[allow]`）
- `mise run test synvelis-automation` — Pass（82 test:
  compiler 43・yaml_subset 15・determinism 12・decode 1・
  lib 11 系含む全緑）
- `mise run check-headless-deps` — Pass（11 headless crates
  に GUI 依存なし）
- `mise run check-full` — Pass（workspace `members` 変更の
  整合として task 指定どおり実行、exit 0）

詳細な case 対応は
`evidence/sv264/autx-05-workflow-dsl-ledger.md` を参照。
`*-msrv` 系はユーザー指定どおり未実行（NotRun）。

## Risks

- `AtomicMacro` は参照のみで未定義 macro は常に reject —
  Macro 定義/展開（#263）統合時に semantics が要再検証。
- Registry Snapshot は serialized fixture との照合であり、
  live Gateway Descriptor Read Adapter・availability の実
  時点性は統合 slice で検証が必要。
- `AwaitJob` の job validity・output 実値は runtime-owned で
  あり、本 crate は型と schema のみ保証（opaque job result
  は `PlanWarning::OpaqueJobResult` で可視化）。
- YAML safe subset は意図的に狭い — flow collection 等を
  使う既存/将来 workflow 記述は subset 拡張の別判断が必要。

## Follow-ups

- #265（AUTX-06）: coordinator/runtime FSM・Dry Run —
  `ExecutablePlan` の consumer。
- #267: E2E 結合・live Gateway adapter。
- #273: Luau/Rhai Script Builder — 同一 `WorkflowDefinition`
  への lowering と同一 compiler 検証の本証明。
- #263: AtomicMacro 定義・Registry・展開 — 本 crate の
  `AtomicMacro` step 参照が解決可能になる。
