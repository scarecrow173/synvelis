---
id: "DISC-557AdoptFacts01"
type: "discovery"
status: "resolved"
title: "Issue #557 discovery — VariantIntent::Adopt 経路の declared_facts 伝播監査"
created: "2026-11-18"
updated: "2026-11-18"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/557"
    - "https://github.com/scarecrow173/synvelis/issues/553"
    - "https://github.com/scarecrow173/synvelis/issues/550"
    - "https://github.com/scarecrow173/synvelis/issues/283"
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
    - "docs/impl/ir/issue-553-intent-facts.md"
    - "docs/impl/ir/cb-06-track-bus-adapter.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/issue-557-adopt-facts.md"
    - "docs/test-specs/cb-06-track-bus-adapter.md"
---

# Issue #557 discovery — VariantIntent::Adopt 経路の declared_facts 伝播監査

## 背景

synvelis#553（PR `synvelis#556`）で `declared_facts` の
`VariantIntent::Edit` arm が inner `ProjectOp` の
`OpFacts`（`preconditions`/`unsupported`）を破棄していた
gap を修正した。同 Issue の review で「`VariantIntent::Adopt`
の同等 gap は実在の可能性があるが Issue/lease 範囲外」と
記録され（EXP E4・PR「未対応の作業」Non-blocking）、
本 Issue `synvelis#557` がその残件監査として起票された。

監査対象は `crates/synvelis-gateway/src/variant.rs` の
`declared_facts` Adopt arm（cover_set のみ呼ぶ分岐）と、
adopt が実際に書き込む scope 内容を導出する
`plan_adopt_writes`（synvelis-project 側・読み取りのみ）。

## 調査結果（main `bc90f0e0` で再確認）

### 1. sv#553 同型 gap は構造上存在しない

- `AdoptIntent`（`crates/synvelis-project/src/variant/
  types.rs:411-426`）は `variant_set`/`branch`/
  `expected_project_revision`/`expected_branch_revision`/
  `expected_scope_digest`/`approval` のみを持ち、
  **inner op を持たない**。`declared_facts` Edit arm が
  `crate::prepare::op_facts` を呼ぶのは
  `BranchEditOp::Project(op)` という inner op が存在する
  からであり、Adopt には伝播すべき inner
  `preconditions`/`unsupported` が構造上存在しない。
- wire 側も同じ: `lower_adopt`（variant.rs:808-815）は
  CAS 4 項目のみ受理し op payload を取らない。

### 2. adopt の fail-closed は domain 層 + post-simulation gate が担う

`PreparedMutation::Variant` の prepare は
`declared_facts` → provisional gate → `variant::simulate`
→ `resource_map` before/after → `derive_effects` →
`authorize_effects` の順で実行される
（`prepare_engine.rs`）。adopt の domain 側拒絶は全て
`simulate` 内で起き、`PreparationRejected`
（"variant simulation failed: variant-*"）として
provisional gate に届く:

- `adopt_guard`（CAS 3 項目 + approval ticket）
- `verified_candidate`（payload decode + scope digest
  再検証 — 改竄・stale payload はここで拒絶）
- adapter `preflight_adopt`
- `plan_adopt_writes`（scope 外移動・removal 非対応
  kind は typed `VariantError::Unsupported`、lineage
  不明は `AmbiguousLineage` で fail-closed）
- asset/plugin/port preflight → `controller.apply`

provisional gate の `impacted` が adopt では
`{VariantSet, Branch}` のみでも、post-simulation の
`derive_effects` + `authorize_effects`（GW-02）が実 diff
を認可するため scope root への write はカバーされる
— これは設計どおりであり facts-drop 型の穴ではない。

### 3. 観測不能削除の wire 到達性

`ResourceKind`（`request.rs:31-65`）と
`snapshot::ENUMERATED_KINDS` は `ModulationSource`/
`ModulationAssignment`/`RoutingConnection`/`SemanticPort`
を含まない — これらの削除は `derive_effects` で
観測不能（SEC-14 境界）。candidate payload に不可視
削除を混入させうる wire op は **send.remove のみ**で、
send-owned modulation entities 存在時は synvelis#553
以降 edit prepare で "unverifiable effects" deny
（pin: `cb06_strip_553_variant_edit_send_remove_
unobservable_cascade_denied`）。`RemoveConnection`/
`AddConnection`/`ConfigureBus`/modulation 系 op は wire
capability 非存在。**現行 wire で不可視削除入り
candidate を新規生成する経路は存在しない**。

### 4. 唯一の残存経路 — pre-553 stored candidate payload

`DirectoryBranchContentStore` は durable であり、
synvelis#550（send op 着地）から synvelis#553
（facts 伝播修正）までの間に永続化された candidate
payload は、send-owned modulation entities を持つ send
への `send.remove` 効果を含んだまま残り得る。その
payload を今 adopt すると domain guard は全て通過し
（payload は自己整合的で scope digest も一致）、
`plan_adopt_writes` が `removal_supported_kind` により
観測不能 kind の `Removed` write を計画・適用し、
post-simulation `derive_effects` にも現れないため
`authorize_effects` を素通りして live に物質化する。

→ hardening は project/gateway 双方の境界設計を要する
ため本 slice の範囲外とし、`synvelis#559` として
別 Issue 化した（payload provenance/schema 世代管理・
adopt 時 observability fail-closed 等の方針候補を記録）。

## 結論と判断

- **AC1**: gap が無いことの根拠を本 discovery・IR・
  `evidence/sv557/` に記録（上記 1–4）。
- **AC2**: gap が無いため deny pin は条件不成立。
  代わりに adopt の受理側を pin する test
  `cb06_strip_557_adopt_send_remove_candidate_
  materializes_teardown` を追加 — fork → candidate
  `send.remove` edit → adopt が teardown
  （SendConfig/RoutingConnection edge/SendLevel
  ParameterTarget の `Removed` write、provenance
  `objects == 3`）を実 diff 経由で物質化することを
  headless で実測し、post-simulation gate が scope
  write をカバーする設計を固定する。
- **AC3**: Edit 経路は非接触。既存 pin
  `cb06_strip_553_*`/`cb06_strip_550_*` の再実測で
  回帰なしを確認。
- cb-06 doc chain の adopt 記述に stale は無く、
  監査結果の追補のみ行う。

## 非目標

- `crates/synvelis-project/**` の変更（Issue 指定どおり
  読み取りのみ — `AdoptIntent`/`plan_adopt_writes`/
  `preflight_adopt`/`removal_supported_kind`）。
- `declared_facts` Adopt arm のコード変更（gap が
  実証されなかったため — AC1 の「根拠記録」側で充足）。
- stored candidate payload の hardening 実装
  （`synvelis#559` に委譲）。
- Edit 経路の再修正（synvelis#553 で完了済み）。
- 個別 op の cascade ロジック・`lower_for`/`op_table`
  の見直し。
- docs index（`docs/**/README.md`）の編集・再生成
  （Coordinator が統合時に実施）。

## 関連リンク

- `synvelis#557`（本 Issue）/ `synvelis#553` /
  `synvelis#550` / `synvelis#283`（CB-06 tracker）
- `synvelis#559`（stored candidate hardening — 本
  discovery の残件から起票）
- `docs/specs/issue-557-adopt-facts.md`
- `docs/impl/ir/issue-557-adopt-facts.md`
- `evidence/sv557/`
- `docs/impl/ir/issue-553-intent-facts.md`
- `docs/impl/ir/cb-06-track-bus-adapter.md`
- `crates/synvelis-gateway/src/variant.rs`
- `crates/synvelis-gateway/src/prepare_engine.rs`
- `crates/synvelis-project/src/variant/engine.rs`
- `crates/synvelis-project/src/apply/entity.rs`
- `crates/synvelis-gateway/tests/cb06_strip_gateway.rs`
- `POLICIES.md`
- `AGENTS.md`
