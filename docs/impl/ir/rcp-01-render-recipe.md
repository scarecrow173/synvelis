---
id: "IMPL-rcp01-render-recipe"
type: "impl"
status: "completed"
title: "RCP-01 Canonical Render Recipe・Typed Dependency Closure・Temporal Invalidation — Headless スライス"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/290"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-project/src/render_input.rs"
      - type: "file"
        path: "crates/synvelis-render/src/recipe.rs"
      - type: "file"
        path: "crates/synvelis-render/src/dependency.rs"
      - type: "file"
        path: "crates/synvelis-render/src/invalidation.rs"
      - type: "file"
        path: "crates/synvelis-render/tests/rcp01_recipe.rs"
      - type: "file"
        path: "docs/impl/ir/rcp-01-render-recipe.md"
      - type: "file"
        path: "docs/impl/exp/rcp-01-render-recipe.md"
      - type: "file"
        path: "docs/test-specs/rcp-01-render-recipe.md"
      - type: "file"
        path: "evidence/sv290/acceptance-ledger.md"
      - type: "file"
        path: "evidence/sv290/verification.md"
    modified:
      - type: "file"
        path: "crates/synvelis-project/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-project/src/routing.rs"
      - type: "file"
        path: "crates/synvelis-project/src/variant/mod.rs"
      - type: "file"
        path: "crates/synvelis-render/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-render/Cargo.toml"
      - type: "file"
        path: "Cargo.lock"
    deleted: []
    renamed: []
    moved: []
    generated:
      - type: "file"
        path: "docs/impl/README.md"
      - type: "file"
        path: "docs/impl/ir/README.md"
      - type: "file"
        path: "docs/impl/exp/README.md"
      - type: "file"
        path: "docs/test-specs/README.md"
  implements: []
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/rcp-01-render-recipe.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/render-checkpoint.md"
    - "docs/designs/rcp-render-recipe-design.md"
    - "docs/adr/rcp-00-render-recipe.md"
  derived-by: []
  verifies:
    - "docs/specs/render-checkpoint.md"
metadata:
  experiments:
    adopted:
      - "docs/impl/exp/rcp-01-render-recipe.md"
    rejected: []
---

# Implementation Record: RCP-01 Canonical Render Recipe Headless スライス（synvelis#290）

## Summary

Issue #290 の RCP-01 として、headless に完結する canonical
`RenderRecipe` モデル + typed dependency closure + temporal
invalidation を `synvelis-project`（Semantic ID/snapshot
read adapter）と `synvelis-render`（Recipe/Dependency
Resolver）に実装した。`RecipeResolution =
Resolved|Unknown|Unsupported`・`Validity = Match|Stale|
Unknown` の fail-closed 契約と、RCP-REC-01..06 を網羅する
`tests/rcp01_recipe.rs`（11 fixture）を追加した。

## Context

- 対象 Issue: https://github.com/scarecrow173/synvelis/issues/290
  （親 #289、RCP-00 契約正本は PR #421、CB-01 VariantSet は
  PR #425 で統合済み）。
- 本スライスは **headless 実装**に限定。Artifact
  Store/GC/remote upload（RCP-02）・Render Job/Policy
  （RCP-03）・実行経路選択（RCP-04）・workspace/Gateway
  projection・実 Plugin/実 OS Gate は後続 Issue の範囲。
- 既存契約（single-writer CAS・typed stable ID・VCS
  ChangeSet・Routing/Control/Offline 能力・CB-01
  `variant/`・schema v14）を**読み取る側**に留め、Project /
  Branch の authority は変更しない。
- `synvelis-render` の `spec.rs` `PluginStateRecipe` は
  plugin state 品質ゲートの別 recipe であり、本実装の
  canonical `RenderRecipe` とは別物（用語衝突を避けるため
  型名・module 名で区別）。

## Implementation

### Changes

- `synvelis-project/src/render_input.rs` 新設:
  - `resolve_render_project` — live project を検証後に
    render 入力へ写像。`RenderBranchTarget::Captured` は
    payload digest・scope digest・schema version・参照を
    検証し、`variant::engine::plan_adopt_writes` +
    `entity::write_all` で live shared context 上に合成
    （merge の独自実装を持たない）。`Live` は live project
    を直接解決。
  - `BranchRenderPin` — variant set/branch/branch content
    revision/payload digest/canonical scope digest/canonical
    shared-context digest を記録（authority provenance）。
  - canonical 抽出: `canonical_entity_value`/`_bytes`/
    `_digest`/`canonical_fingerprint`/`_except`。display
    `name` を再帰除去、非有限値 reject、map/set は
    ソート済み直列化、revision/artifact location/作成時刻/
    無関係 subgraph を除外。
  - `shared_context_digest` は live semantic objects の
    scope 外補集合を fingerprint（shared edit が全 branch
    を失効させるため）。
- `synvelis-render/src/recipe.rs` 新設:
  - `RECIPE_SCHEMA_VERSION`/`CANONICALIZATION_VERSION`/
    `AUDIO_SEMANTICS_VERSION` = 1、
    `RECIPE_HASH_DOMAIN`/`RECIPE_DOMAIN_TAG`/
    `RECIPE_HASH_ALGORITHM` 定数。
  - `RenderRecipeV1`（temporal domain・render boundary・
    output contract・target・branch pin・closure を含む
    canonical recipe）と `RecipeKey`（SHA-256 over domain
    separator + versioned canonical bytes）。
  - `RecipeResolution`（`Resolved`/`Unknown`/`Unsupported`）
    と `ResolutionProvenance`（非 hash の authority pins:
    snapshot/live revision/variant set/branch/branch content
    revision/payload digest）。
  - `RecipeRecord::verify`（schema version + canonical
    bytes からの key 再計算）と `evaluate_validity`
    （`Match`/`Stale`/`Unknown`）。
- `synvelis-render/src/dependency.rs` 新設:
  - `DependencyEdgeKind`（Audio/Event/Control/Parameter/
    Sidechain/ExternalMedia/Transport/TimeMapping）の
    typed edge 語彙。
  - `resolve_recipe` — render boundary port から routing を
    逆走査し、routing/strip/bus/processor/send/return/
    clip/note/source/MIDI/comping（lane/take group/comp/
    region/take）/parameter target/automation lane/modulation/
    plugin instance/plugin-state evidence/external asset/
    source verification/project time-mapping を typed
    closure として構築。
  - fail-closed: missing entity・unresolved processor・
    plugin-state evidence 欠落/degraded・unpinned/weak
    external asset・unverified source・unsupported
    automation scope・schema skew・invalid value・live
    clock・routing validation/cycle を `Unknown`/
    `Unsupported` に落とす。
  - `PluginStateProbe`/`ExternalAssetProbe` 注入境界
    （`NoPluginStates`/`StaticPluginStates`/
    `NoExternalAssets`/`StaticExternalAssets`）。
- `synvelis-render/src/invalidation.rs` 新設:
  - `ChangedObject`/`InvalidationReason`/`InvalidationPlan`/
    `classify_invalidation`。closure 外変更を無視し、既知
    dirty interval を lookahead+PDC（後方）・tail（前方）・
    loop coverage・seek boundary・temporal-domain/未知
    extent（全域）へ保守的に拡張。checked 時間演算のみ。
- `synvelis-project/src/routing.rs`: `RoutingModel::validate`
  を `pub` に widening（render crate が sanctioned read
  consumer として cycle/owner 不変条件を再検証するため。
  write 権限の付与ではない）。
- `synvelis-project/src/variant/mod.rs`: `scope` を
  `pub(crate)` に（render_input が scope object 列挙を
  再利用するため）。
- `synvelis-render/Cargo.toml`: `serde`/`serde_json` 追加
  （canonical bytes 直列化）。
- `synvelis-render/tests/rcp01_recipe.rs` 新設: 11 fixture
  （RCP-REC-01..06 を網羅）。

### Decisions During Implementation

- **branch identity は provenance のみ**（exp E1）:
  `RecipeBranchPin` を canonical scope/shared-context digest
  の 2 要素に限定し、variant set/branch/revision/payload
  locator を hashed identity から除外。同一音響 graph の
  別 branch は同一 key。
- **scope digest は canonical name-free fingerprint を別計算**
  （exp E1/E2）: CB-01 の `scope_digest` は display name を
  含むため render identity には使わず、payload integrity
  検査のみに残す。
- **shared_context_digest は補集合**: live semantic objects
  から scope objects を除いた fingerprint。shared edit が
  全 branch の key を動かし、scope 内 edit は当該 branch
  のみを動かす。
- **branch 合成は既存 machinery 再利用**: `plan_adopt_writes`
  + `entity::write_all` で captured payload を live context
  上に合成。merge 実装を重複させない。
- **non-finite は reject、正規化しない**（exp E2）:
  NaN/±inf を `0`/`null` に潰さず fail-closed。
- **temporal invalidation は over-approximate**（exp E4）:
  過少区間再利用を許さず、未知 extent・temporal-domain は
  全域失効。

## Related Experiments

- `docs/impl/exp/rcp-01-render-recipe.md` — branch authority
  識別子の分離・canonical 抽出・closure 走査・temporal
  拡張・record verify の検討記録（adopted）。

## Validation

- `mise run fmt-check` — Pass。
- `mise run check synvelis-project` — Pass。
- `mise run check synvelis-render` — Pass。
- `mise run lint synvelis-project` — Pass。
- `mise run lint synvelis-render` — Pass。
- `mise run test synvelis-project` — Pass。
- `mise run test synvelis-render` — Pass（`rcp01_recipe.rs`
  11 件を含む全 suite green）。
- 詳細は `evidence/sv290/verification.md`・
  `evidence/sv290/acceptance-ledger.md`。

## Risks

- `resolve_recipe` の closure は render boundary 起点の
  逆走査。boundary 外の subgraph は意図的に含まないため、
  「render に到達しない変更」が key を動かさないことは
  RCP-REC-03 の要件どおりだが、boundary 定義の誤りは
  closure の盲点になり得る（`RenderBoundary::master` の
  port 解決は `RenderBoundary` 側の責務）。
- `canonical_entity_value` は entity kind ごとの手書き写像。
  新 entity kind 追加時に写像漏れがあると canonical bytes
  から落ちて key が過小になる可能性がある（schema gate と
  `validate_references` で fail-closed 側に倒す設計だが、
  kind 追加時のレビュー観点として残す）。
- `RecipeResolution::Unknown`/`Unsupported` は Exact
  CacheHit に昇格しない契約。上位層（RCP-02..04）がこの
  tri-state を正しく扱うことが前提。

## Follow-ups

- Artifact Store/GC/remote upload（RCP-02、Issue #291 系）。
- Render Job/Policy（RCP-03）、実行経路選択（RCP-04）。
- workspace/Gateway projection（`synvelis-workspace` は別
  Worker の lease のため DEFER）。
- 実 Plugin/実 OS device/実 codec/実 render 実行での受入
  （`evidence/sv290/` の NotRun を後続 Gate で Pass/NotRun
  記録）。
