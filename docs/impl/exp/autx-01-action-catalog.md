---
id: "EXP-AUTX01ActionCatalog"
type: "experiment"
status: "completed"
title: "AUTX-01 Action Catalog 実験ログ（synvelis#260 headless slice）"
created: "2026-11-14"
updated: "2026-11-14"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/260"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-workspace/src/action_catalog.rs"
      - type: "file"
        path: "crates/synvelis-workspace/src/action_catalog/fixture.rs"
      - type: "file"
        path: "crates/synvelis-workspace/tests/autx01_action_catalog.rs"
      - type: "file"
        path: "docs/impl/ir/autx-01-action-catalog.md"
      - type: "file"
        path: "docs/impl/exp/autx-01-action-catalog.md"
      - type: "file"
        path: "docs/test-specs/autx-01-action-catalog.md"
      - type: "file"
        path: "evidence/sv260/README.md"
    modified:
      - type: "file"
        path: "crates/synvelis-workspace/src/lib.rs"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/impl/ir/autx-01-action-catalog.md"
  implemented-by: []
  depends-on:
    - "docs/automation/design-capability-action.md"
    - "docs/automation/design-trust-boundary.md"
    - "docs/specs/command-gateway.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/autx-01-action-catalog.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/impl/ir/autx-01-action-catalog.md"
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers:
    - "CapabilityRegistry::has_executor / Gateway::schemas read accessor（Coordinator 判断で否 — sv#263 lease 衝突。ExecutorState::Unknown / SchemaConsistency::Unknown の typed Unknown で代替）"
    - "実 ScriptApiModel（synvelis#271）照合・Script Action（synvelis#272）・CLI surface（synvelis#262）・実 GPUI/OS lane（#114/#267）"
  deferred-by: []
metadata:
  experiments:
    adopted:
      - "ActionCatalog::project(descriptor slice, Option<&SchemaRegistry>, presentations) + project_gateway(&mut Gateway) の 2 入口"
      - "ActionStatus（lifecycle）/ SchemaConsistency / ExecutorState の verdict 分離"
      - "orphan presentation を Unknown/UnsupportedVersion entry として surface"
      - "ActionFilter{query,category,include_hidden,include_unpresented} — 既定 hidden 除外・unpresented 含有"
    rejected:
      - "crates/synvelis-gateway への has_executor/schemas accessor 追加（Coordinator ask → lease 衝突で否）"
      - "Action 側への authority field 複写（required_permissions/target/availability/version）"
      - "catalog への register/ mutate API（第二 Registry 防止）"
      - "GUI 側 ActionRow/ActionCatalogView の再利用・変更（#266 view 契約は consumer-neutral に維持）"
      - "parameter_hints の schema field 照合（hint は表示 metadata、authority ではない）"
---

# AUTX-01 Action Catalog experiment log

Issue: https://github.com/scarecrow173/synvelis/issues/260

## 採用した設計判断

### 1. projection は descriptor slice 入力、Gateway 変更ゼロ

`ActionCatalog::project` は `&[CapabilityDescriptor]` +
`Option<&SchemaRegistry>` + `&[ActionPresentation]` を取り、join 結果を
所有 `ActionEntry` に snapshot する。`project_gateway(&mut Gateway)` は
`registry().list()`+`describe()` で descriptor を収集してから
`schemas_mut()` を borrow する sequential 構成で、`&Gateway`/`&mut
Gateway` どちらの caller も追加の公開 API なしに使える。

- 理由: `CapabilityRegistry::list`/`describe`/`versions` と
  `SchemaRegistry::resolve` は既に公開済み。catalog が必要とする
  全情報は公開面で揃っており、gateway 変更は不要だった。
- 副次効果: `project` は Gateway 非依存の純粋投影として単体試験
  可能で、`registry_mut().register` による incoherent 登録
  （`register_capability` の schema 整合 check を bypass）も
  descriptor slice 経由で `SchemaConsistency::Mismatch` として
  検出できる。

### 2. 状態判定は 3 つの typed verdict に分離

`ActionStatus`（lifecycle/registration）・`SchemaConsistency`・
`ExecutorState` を別 field にした。`Available` は
`Availability::Available` の写しであって実行可能の意味ではなく、
design §3 の「表示 status ≠ 実行権限」を型で固定する。

- `ExecutorState` は当初「has_executor を ask する」案を持っていたが、
  Coordinator が `crates/synvelis-gateway/` の lease 拡張を否（sv#263
  AUTX-04 が占有中）。指示どおり typed Unknown で代替 —
  `ExecutorState::Unknown` 単一 variant は「観測不能」を偽装しない
  ためのもので、`has_executor` 導入時に `Installed`/`Absent` を
  追加しても wire 形が壊れない。
- `SchemaConsistency::Unknown` は schema store を渡せない caller
  向けの同型 fallback。`project_gateway` は常に `Some` を渡すため
  実運用では `Consistent`/`Mismatch` が出る。

### 3. orphan presentation は隠さず surface

presentation が指す `CapabilityRef` が registry に無い場合
（未知 id → `Unknown`、既知 id の未知 version →
`UnsupportedVersion{supported}`）、catalog は descriptor 無しの
entry を生成して `list` に残す。discovery が product metadata の
不整合を隠すと、欠陥が「表示されない」形で埋もれるため。

### 4. fixture は typed `WireRequest` で 4 domain を網羅

`fixture::invocations()` は Query（track/clip の 2 resource kind）・
ProjectMutation（`ExpectedRevision`）・RuntimeControl・
AsyncOperation（`Unconditional`）の `WireRequest` を返す。
`Gateway::bind`/`invoke` にそのまま食わせて全件成功を確認済み。
GUI 側は #266 の `automation_requests` が同じ `CapabilityRef` を
lower することを `catalog_feeds_autx07_view_projection_unchanged`
で固定し、CLI 側は同じ wire shape を `parse_wire` 経路で送る
（#262 の実装は別リース）。

## 試して棄却したもの

- **`CapabilityRegistry::has_executor` + `Gateway::schemas` 追加** —
  `orca orchestration ask` で範囲拡張を依頼したが、Coordinator が
  sv#263（AUTX-04）の gateway lease 衝突を理由に否。typed
  Unknown/Unsupported fallback で進行し、必要 API は evidence/PR
  の Deferred に記録（Coordinator が後続 slice を起票予定）。
- **catalog への `register`/`unregister` 的 API** — 第二 Registry
  化を型で防ぐため、構築は `project` のみ・entries は read-only。
- **`ActionRow`/`ActionCatalogView`（#266 view 型）の再利用** —
  view 契約は `automation_ui` が所有し、本 slice はその供給元に
  徹する。`descriptors()` で `&[CapabilityDescriptor]` を渡すだけで
  view 側変更ゼロを test で確認した。
- **`parameter_hints` と schema field の照合** — hint は product
  metadata であって schema 検証対象ではない。hint 名と schema の
  不一致は表示品質の問題であり、authority 契約違反ではないため
  実装しない（YAGNI）。

## 検証証跡

- `cargo test -p synvelis-workspace --test autx01_action_catalog`:
  15 passed（join/list/describe 状態行列・orphan・schema consistency・
  no-authority・display 名変更不変・fixture 4 domain bind/invoke・
  view projection 互換）。
- `mise run check/lint/test synvelis-workspace`・`mise run
  fmt-check`: Pass（OS: Windows、toolchain は mise 管理）。
- `mise run check synvelis-app`: 結果は `evidence/sv260/README.md`。
- 実 GPUI/実 OS/live adapter/CLI surface/実 ScriptApiModel 照合は
  NotRun/Deferred（`evidence/sv260` の Case ID 表参照）。
