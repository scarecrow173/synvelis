---
id: "TSPEC-034XiQux1sWs73DvJunwUQ"
type: "test-spec"
status: "approved"
title: "SV-SCRIPT-01 ScriptApiModel・Luau stub/binding 整合 invariants"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/271"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on:
    - "docs/automation/design-script.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/ir/sv-script-01-script-api-model.md"
    - "docs/impl/exp/sv-script-01-script-api-model.md"
    - "evidence/sv271/sv-script-01-script-api-model-ledger.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies:
    - "docs/specs/command-gateway.md"
    - "docs/designs/command-gateway-design.md"
  verified-by: []
  references:
    - "docs/automation/spec.md"
    - "docs/automation/design-script.md"
    - "evidence/sv271/sv-script-01-script-api-model-ledger.md"
    - "docs/impl/ir/sv-script-01-script-api-model.md"
    - "crates/synvelis-script/src/model.rs"
    - "crates/synvelis-script/src/binding.rs"
    - "crates/synvelis-script/src/luau.rs"
    - "crates/synvelis-script/src/analyzer.rs"
  defers: []
  deferred-by: []
---

# SV-SCRIPT-01 ScriptApiModel・Luau stub/binding 整合 invariants

## Purpose

Issue #271（SV-SCRIPT-01）の headless slice が保証すべき不変条件を
固定する。対象は新規 crate `synvelis-script` の
`RegistrySnapshot` → `ScriptSurfaceSpec` → `ScriptApiModel` →
`BindingSet`/`LuauStub` の決定的生成、digest pin、fail-closed
verdict、wire-shaped `InvocationDraft`、および Analyzer port の
consumer 側契約である。実 Luau runtime 実行・実 Luau.Analysis
native FFI・consumer 結合（#272/#273/#274）・Rhai/Lua backend は
対象外であり、fake analyzer の Pass を実 Analyzer の Pass に
昇格しない。

## Feature

Gateway `CapabilityRegistry`/`SchemaRegistry` を read-only 正本と
する versioned Script API surface と、同一 `ScriptApiModel`
digest から生成される Runtime Binding（typed 呼出 lower 形）と
Luau 型定義（`.d.luau` 形テキスト）の対。

## Rules

- Rule: digest 連鎖の決定性 — 同一 Registry/Schema Snapshot と
  surface spec からは、同一 `ScriptApiModel`・`BindingSet`・
  `LuauStub`・全 digest が再現される。生成物は model digest を
  pin し、model との不一致は `StaleArtifact`、binding↔stub の
  面不一致は `BindingStubMismatch` で fail closed する。
- Rule: fail-closed verdict — `UnknownCapability`・
  `UnsupportedVersion`・`Disabled`・`Planned`・`Retired`・
  `UnresolvedSchema`・`InvalidName`・visibility `Hidden` は
  いずれも invocable 型・binding・stub・completion 候補を
  生成しない。discovery 表示 metadata と実行可否を分離する。
- Rule: wire 形状の lower — `InvocationDraft` は project・
  target・consistency・args・request id・opaque
  task/grant/idempotency field を持つ wire-shaped precursor
  であり、`WireRequest::parse` を通る。`CallerIdentity`・
  `AdapterSeal`・Grant/Approval receipt を生成・名乗らない。
- Rule: 型投影の total 性 — `ValueSchema` の全 variant
  （Bool/Number/Integer/Enum/String/Array/Object/Optional/
  Id/TimeRange）が Luau 型へ写像される。f64 で表現不能な i64
  は `SynvelisI64` 10 進文字列で運び、invoke 時に schema が
  `Integer` の位置でのみ厳密 decode する。暗黙丸めをしない。
- Rule: 値検証の typed reject — target contract shape・
  resource kind・tagged id・consistency×domain 適合・
  schema 適合・i64 decode・time-range domain は
  `GatewayError`/typed `ScriptApiError` で拒否し、Gateway の
  schema 再検証が最終権威であり続ける。
- Rule: analyzer 成功 ≠ 権限 — `ScriptAnalyzer` port は
  生成 definitions を consumer へ注入するだけで、解析成功は
  `Denied`/`ApprovalRequired` を変えず Grant を生まない。
  unsupported・unavailable・stale result・version mismatch は
  typed error で fail closed する。
- Rule: metadata disclosure — stub・diagnostic・completion
  には公開可能 metadata（名前・説明・型・provenance digest・
  公開 availability）のみ含め、hidden/未公開 capability の
  存在や `Disabled{reason}` の reason を漏らさない。

## Examples

- Example: golden determinism — 同一 fixture snapshot +
  surface から 2 回生成した model/binding/stub の canonical
  JSON と digest が一致する（`tests/golden.rs`）。
- Example: schema drift — surface/binding/stub のいずれかを
  改変すると `verify_against`/`check_consistency` が typed
  error で拒否する（`tests/golden.rs`・`tests/binding.rs`・
  `tests/luau.rs`）。
- Example: hidden 境界 — visibility `Hidden` の entry は
  model には残るが `list_public`・stub・binding・
  completion のいずれにも現れない（`tests/model.rs`・
  `tests/luau.rs`・`tests/analyzer.rs`）。
- Example: typed 呼出 — `addNote` の call が target
  `{kind: track, id}`・`expected-revision` consistency・
  i64 文字列 decode 済み args を持つ draft になり、
  `WireRequest::parse` を通過する（`tests/binding.rs`）。
- Example: 値 reject — target 欠落・kind 不一致・consistency
  ×domain 不適合・schema 違反・i64 非数字・time domain 不一致
  がそれぞれ typed error になる（`tests/binding.rs`）。
- Example: analyzer fixture — fake analyzer が未知 API 名と
  誤 arity を diagnostic として返し、公開 capability の
  completion 候補を返し、source revision ずれを stale として
  拒否する（`tests/analyzer.rs`）。

## Guarantees

- 同一 snapshot+surface からの生成物は byte/digest 単位で
  決定的であり、fixture と live registry capture の双方で
  同じ pipeline を通る。
- 公開 API 面（公開型シグネチャ）に VM/GPUI 内部型・
  authority 型（`CallerIdentity`/`Grant`/`Approval`/
  `Principal`）が存在しない。
- `BindingSet`/`LuauStub` は同一 model の invocable public
  集合だけを投影し、`GeneratedArtifacts::check_consistency`
  が両者の capability pin・schema ref・名前集合の一致を検証する。
- `assemble_call` は invoke/bind を呼ばず、生成されるのは
  wire-shaped `InvocationDraft` のみである。実行は常に
  `Gateway::bind`→`invoke` の認可・schema 再検証・approval
  を通る。

## Non-goals

- 実 Luau runtime での script 実行、実 Luau.Analysis native
  FFI/実測（ruviform#560 領域）、Rhai/Lua 5.4 backend。
- `Gateway::bind`/`invoke` の呼出、`OperationRequest` の
  trusted 型構築、project 直接 mutation、独自 Capability
  Registry/認可/Job 正本の検証。
- Action Script lowering（#272）・Script Builder（#273）・
  Workflow Script（#274）の consumer 受入。
- descriptor の effect/permission/data-access metadata の
  script 面への再投影（正本は Gateway のみ）。

## Risk

- 決定性・digest pin が崩れると schema drift を検出できず、
  stale binding/stub が analyzer・runtime に流通する。
- fail-closed verdict の抜けは、非 available/hidden
  capability の invocable 型生成や completion 漏洩を招く。
- i64 を f64 `number` に落とす投影は精度を静かに失うため、
  `SynvelisI64` 経路の typed decode が失われると値破壊になる。
- analyzer port が authority を匂わせると「解析成功＝実行可」
  の誤契約を生むため、Grant 非生成・stale reject の保証が
  必須である。
