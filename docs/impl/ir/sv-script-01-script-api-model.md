---
id: "IMPL-034XhXMfp77Z0MfGZwZQIF"
type: "impl"
status: "completed"
title: "SV-SCRIPT-01 ScriptApiModel・Runtime Binding/Luau 型定義 Generator"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/271"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-script/Cargo.toml"
      - type: "file"
        path: "crates/synvelis-script/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-script/src/analyzer.rs"
      - type: "file"
        path: "crates/synvelis-script/src/binding.rs"
      - type: "file"
        path: "crates/synvelis-script/src/canon.rs"
      - type: "file"
        path: "crates/synvelis-script/src/error.rs"
      - type: "file"
        path: "crates/synvelis-script/src/luau.rs"
      - type: "file"
        path: "crates/synvelis-script/src/model.rs"
      - type: "file"
        path: "crates/synvelis-script/src/schema.rs"
      - type: "file"
        path: "crates/synvelis-script/src/snapshot.rs"
      - type: "file"
        path: "crates/synvelis-script/src/surface.rs"
      - type: "file"
        path: "crates/synvelis-script/tests/analyzer.rs"
      - type: "file"
        path: "crates/synvelis-script/tests/binding.rs"
      - type: "file"
        path: "crates/synvelis-script/tests/common/mod.rs"
      - type: "file"
        path: "crates/synvelis-script/tests/fixtures/registry_snapshot.json"
      - type: "file"
        path: "crates/synvelis-script/tests/golden.rs"
      - type: "file"
        path: "crates/synvelis-script/tests/luau.rs"
      - type: "file"
        path: "crates/synvelis-script/tests/model.rs"
      - type: "file"
        path: "docs/impl/exp/sv-script-01-script-api-model.md"
      - type: "file"
        path: "docs/test-specs/sv-script-01-script-api-model.md"
      - type: "file"
        path: "evidence/sv271/sv-script-01-script-api-model-ledger.md"
    modified:
      - type: "file"
        path: "Cargo.toml"
    deleted: []
    renamed: []
    moved: []
    generated:
      - type: "file"
        path: "Cargo.lock"
      - type: "file"
        path: "docs/impl/ir/README.md"
      - type: "file"
        path: "docs/impl/exp/README.md"
      - type: "file"
        path: "docs/test-specs/README.md"
  implements: []
  implemented-by: []
  depends-on:
    - "docs/automation/design-script.md"
    - "docs/automation/design-capability-action.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/exp/sv-script-01-script-api-model.md"
    - "docs/test-specs/sv-script-01-script-api-model.md"
    - "evidence/sv271/sv-script-01-script-api-model-ledger.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/sv-script-01-script-api-model.md"
  references:
    - "docs/automation/design-script.md"
    - "crates/synvelis-gateway/src/capability.rs"
    - "crates/synvelis-gateway/src/schema.rs"
    - "crates/synvelis-gateway/src/registry.rs"
    - "crates/synvelis-gateway/src/request.rs"
    - "crates/synvelis-gateway/src/wire.rs"
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted:
      - "docs/impl/exp/sv-script-01-script-api-model.md"
    rejected: []
---

# SV-SCRIPT-01 ScriptApiModel・Runtime Binding/Luau 型定義 Generator

## Summary

新規 headless crate `crates/synvelis-script` を追加し、
Gateway `CapabilityRegistry`/`SchemaRegistry` を read-only
正本として versioned `RegistrySnapshot` を capture、
script 名→exact `CapabilityRef` の `ScriptSurfaceSpec` と
合わせて決定的 `ScriptApiModel` を build する。同一 model
digest から Runtime `BindingSet`（wire-shaped
`InvocationDraft` への typed lower）と `LuauStub`
（`.d.luau` 形型定義）を双生成し、digest pin・
`StaleArtifact`/`BindingStubMismatch` による drift 検出、
fail-closed `ScriptVerdict`/`InvocableBlocker`、Analyzer
port（definition-injection consumer adapter）を実装した。
invoke/bind は呼ばず、`InvocationDraft` は
`WireRequest::parse` を通る precursor に留まる。

## Context

Issue #271（SV-SCRIPT-01）の headless slice。
`docs/automation/design-script.md`（AUTX-00 契約）の
SV-SCRIPT-01〜04 canonical 境界に従い、script が第二の
Capability Registry・独自認可・Job 正本を持たず、Gateway の
discovery metadata のみを消費する read-only consumer として
配置する。Issue 本文は `synvelis-automation` 内新設を想定
していたが、sv#265 の並行 lease のため Coordinator 決定で
新規 crate `synvelis-script` に配置した（責務・契約は不変）。

## Implementation

### Changes

- `crates/synvelis-script/`: 新規 crate（workspace member
  登録、edition/rust-version/lints は workspace 継承、
  `publish = false`、依存は `synvelis-gateway` read API +
  serde/serde_json/sha2 のみ）。
  - `snapshot.rs`: `RegistrySnapshot::capture` —
    `CapabilityRegistry::list`/`describe`/`versions` と
    `SchemaRegistry::resolve`/`versions` から descriptor +
    schema mirror を決定的 sort で凍結。canonical JSON +
    SHA-256 digest。duplicate・未対応 schema version を
    typed error で拒否。dangling schema 参照は保持して
    model 側 verdict に委ねる。
  - `surface.rs`: `ScriptSurfaceSpec`/`ScriptSurfaceEntry`
    — script 名の Luau identifier 検査（`luau_name_status`）、
    exact `CapabilityRef` pin、`Public`/`Hidden` visibility。
  - `model.rs`: `ScriptApiModel::build` — snapshot+surface
    検証、duplicate 名の fatal reject、name sort、
    `Availability`→`ScriptVerdict` 写像（`Invocable`/
    `Disabled`/`Planned`/`Retired`/`UnsupportedVersion`/
    `UnknownCapability`/`UnresolvedSchema`/`InvalidName`）、
    provenance（snapshot/surface/capability digest）付与。
    `is_invocable` は verdict と visibility の双方を要求。
  - `schema.rs`: `ScriptSchema`/`ScriptField` — `ValueSchema`
    の serializable mirror と resolve 結果。
  - `canon.rs`: canonical JSON（object key 再帰 sort）+
    SHA-256 lowercase hex digest。
  - `binding.rs`: `BindingSet::generate`（public invocable
    のみ・exact capability/schema pin・target/domain/
    execution metadata）、`InvocationCall`/
    `InvocationDraft`/`ScriptTarget`/`ScriptConsistency`、
    `assemble_call` の typed 検証（名前存在・invocable・
    target contract・resource kind・tagged id・consistency
    ×domain・schema・strict i64 decode・time-range domain）、
    `verify_wire()`（`WireRequest::parse` 経由）、
    `verify_against()`（model digest pin）。
  - `luau.rs`: `ValueSchema`→Luau 型の total 写像
    （`SynvelisI64`/`Synvelis<Kind>Id`/`SynvelisConsistency`/
    `SynvelisProjectId` helper alias）、`LuauStub::generate`
    （provenance header・`-- DO NOT EDIT`・権限非付与
    disclaimer・public invocable のみ）、stub digest 検証。
  - `analyzer.rs`: `ScriptAnalyzer` trait・
    `AnalyzeRequest`/`AnalyzeResult`/`TypeDefinition`/
    `Diagnostic`/`CompletionItem`/`AnalyzerDescriptor`/
    `DefinitionInjection`・`verify_result_revision`（stale
    reject）。ruviform/Luau.Analysis 依存なし。
  - `error.rs`: `ScriptApiError` typed error 集合
    （`Internal` は serialized model の invocable 不変条件
    違反のみ）。
  - `tests/`: `common` fixture（全 `ValueSchema` variant・
    target contract・domain・availability を網羅する
    in-memory registry + surface + serialized snapshot
    fixture）と analyzer/binding/golden/luau/model の
    5 test binary・56 test。
- `Cargo.toml`: workspace `members` に
  `"crates/synvelis-script"` を追加。
- `Cargo.lock`: cargo 再解決による再生成のみ。
- `docs/impl/exp/`・`docs/test-specs/`・`evidence/sv271/`:
  実験記録・test spec・case ledger。

### Decisions During Implementation

- **配置**: Issue は `synvelis-automation` 内新設を想定したが
  sv#265 lease のため新規 crate `synvelis-script` に配置
  （Coordinator 決定・責務不変）。
- **snapshot 入力**: `CapabilityRegistry::resolve` は
  `pub(crate)` のため、discovery metadata
  （`list`/`describe`/`versions` + `SchemaRegistry::resolve`）
  のみで凍結。dangling schema は model 側
  `UnresolvedSchema` verdict に写像。
- **i64 投影**: f64 で表現不能な `Integer` は `SynvelisI64`
  10 進文字列。`assemble_call` は schema `Integer` 位置と
  `TimeRange.start`/`len` のみ strict decode。暗黙丸めなし。
- **invocable 不変条件**: `ScriptApiModel` は deserialize
  可能なため、`BindingSet::generate`/`LuauStub::generate` は
  `.expect()` ではなく `ScriptApiError::Internal` で
  手編集 model の不変条件違反を fail closed にした。
- **wire 形状**: `InvocationDraft` は gateway wire 型を
  field に持ち `verify_wire()` で `WireRequest::parse` を
  通す。task/grant/idempotency は opaque `Value` で、
  trusted 型は型として存在しない。

## Related Experiments

### Adopted

- `docs/impl/exp/sv-script-01-script-api-model.md` —
  E1 snapshot 入力（discovery metadata 凍結）・E2 i64
  `SynvelisI64` 文字列・E3 `InvocationDraft` wire 互換形・
  E4 analyzer port trait の 4 試行をすべて採用。

### Rejected

- E1 代替: registry live 参照（versioned snapshot 契約を
  壊す）。E2 代替: 全 Integer 文字列化（stub 可読性を損なう）。
  E3 代替: `WireRequest` 直接返却（opaque 境界が曖昧）。
  E4 代替: ruviform 直接依存（本 slice 範囲外・#560 領域）。

## Validation

`evidence/sv271/sv-script-01-script-api-model-ledger.md` に
case 単位で記録。実行済み: `mise run fmt-check`・
`mise run check/lint/test synvelis-script`・
`mise run check/lint/test synvelis-gateway`（全 Pass・
exit 0）。`mise run gw10` は任意枠のため NotRun。
`*-msrv` はユーザー指定により未実行。

## Risks

- 実 Luau.Analysis/実 runtime 結合は ruviform#560・#272〜
  #274 の統合受入ゲートに残る。fake analyzer の Pass を
  実 Analyzer の Pass に昇格しない。
- `ScriptApiModel`/`BindingSet`/`LuauStub` の deserialize
  面は手編集で不変条件を破り得るため、生成側は
  `Internal` error で fail closed としたが、consumer 側で
  model を再構築する経路は `build` のみを正規入口とする
  規約が前提。
- Gateway 側の public API 面が sv#265 の merge で変化した
  場合、snapshot capture の適合確認が必要（本 slice は
  merged `origin/main` の公開 API に対して build）。

## Follow-ups

- #272 Action Script（`ScriptApiModel`/digest 契約の
  consumer）・#273 Script Builder・#274 Workflow Script。
- ruviform#560 の実 Analyzer 結合・実 Luau runtime 実行。
- `tools/check-headless-deps.mjs` の `HEADLESS_CRATES`
  登録は統合時に Coordinator が実施。
