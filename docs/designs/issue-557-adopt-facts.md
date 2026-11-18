---
id: "DESIGN-557AdoptFacts03"
type: "design"
status: "approved"
title: "Issue #557 design — Adopt 経路 declared_facts 監査と受理 pin"
created: "2026-11-18"
updated: "2026-11-18"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/557"
    - "docs/specs/issue-557-adopt-facts.md"
    - "docs/discovery/issue-557-adopt-facts.md"
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
    - "docs/specs/issue-557-adopt-facts.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/plans/issue-557-adopt-facts.md"
    - "crates/synvelis-gateway/src/variant.rs"
    - "crates/synvelis-gateway/src/prepare_engine.rs"
    - "crates/synvelis-project/src/variant/engine.rs"
    - "crates/synvelis-project/src/apply/entity.rs"
---

# Issue #557 design — Adopt 経路 declared_facts 監査と受理 pin

## 全体構造

本 slice は監査 + 記録 + pin test の slice であり、
production code の変更は行わない（監査の結論が
「gap 無し」の場合）。変更面は:

- `crates/synvelis-gateway/tests/cb06_strip_gateway.rs`
  — adopt 受理 pin test を 1 件追加（test-local、
  共有 fixture `tests/rig/mod.rs` は非接触）
- `docs/test-specs/cb-06-track-bus-adapter.md`・
  `docs/impl/ir/cb-06-track-bus-adapter.md`・
  `docs/impl/exp/cb-06-track-bus-adapter.md` —
  監査結果の追補
- issue-557 doc chain 9 件 + `evidence/sv557/`
- `crates/synvelis-gateway/src/variant.rs` —
  **変更しない**（gap が実証されなければ Adopt arm の
  `cover_set` のみの現行実装が正しい）

## 監査設計 — なぜ Adopt arm に inner facts が無いか

`declared_facts` の責務は「この intent が読み・書きし得る
resource の宣言的近似」を `OpFacts` として返すこと。
Edit arm が inner `op_facts` を伝播するのは、
`BranchEditOp::Project(op)` が inner `ProjectOp` を持ち、
その op 固有の `unsupported`/`preconditions` が
provisional gate でしか観測できない verdict を持つ
から（synvelis#553 で修正）。

Adopt arm に同じ伝播が必要になるのは、`AdoptIntent` が
inner op を持ち、かつその op に `op_facts` が存在する
場合のみ。実際の `AdoptIntent` は CAS + approval のみで
inner op を持たず、`lower_adopt` も op payload を
受理しない — 伝播すべき inner facts は型レベルで
存在しない。

## adopt の write 認可経路 — post-simulation gate が実 diff を覆う

adopt が live に書き込むのは candidate payload の scope
内容であり、`plan_adopt_writes` が origin scope object
と candidate scope object の差分から `Upserted`/
`Removed` write を計画する。この write 集合は
`declared_facts` ではなく **simulation 後の実 diff**
から導出される:

1. `declared_facts`（provisional）: cover_set のみ —
   `VariantSet`/`Branch`/`Project`/scope roots の reads、
   `VariantSet`/`Branch` の impacted。
2. provisional gate: declared reads/impacted/preconditions/
   unsupported を authorization scope と照合。
3. `variant::simulate`: domain の adopt 全経路
   （`adopt_guard` → `verified_candidate` →
   `preflight_adopt` → `plan_adopt_writes` → preflight →
   `controller.apply`）を scratch controller で実行。
   拒絶は `PreparationRejected`。
4. `resource_map` before/after → `derive_effects` →
   `authorize_effects`: **実際に変化した resource** の
   effect 集合を認可 — scope root 配下の entity write
   （send config 削除・edge 削除・level target 削除を
   含む）はここで観測・認可される。

したがって provisional `impacted` に scope roots が
無いことは穴ではない — provisional gate は「この intent
が触り得る範囲の宣言」を見る層であり、実 write の認可は
post-simulation gate が実 diff で行う二層構造。
Edit arm の inner facts 伝播が必要だったのは、inner op
の `unsupported` verdict が **simulation 前**にしか
観測できない fail-closed（unverifiable cascade）だから
であり、adopt にはその種の事前 verdict が存在しない
— domain の全拒絶は `simulate` 内の typed error として
現れる。

## 観測不能削除の扱い — SEC-14 境界の確認

`ResourceKind`/`ENUMERATED_KINDS` が観測しない kind
（`ModulationSource`/`ModulationAssignment`/
`RoutingConnection`/`SemanticPort`）の削除は
`derive_effects` に現れない。adopt がこれらを物質化
し得るかは「candidate payload にそれらの削除が入るか」
に帰着する:

- 現行 wire: `lower_for`/`op_table` で entity を削除
  し得る op は send.remove のみが観測不能 kind の
  cascade を持ち、send-owned modulation entities 存在時
  は synvelis#553 以降 edit prepare で deny。
  → 現行 wire で不可視削除入り candidate は生成不能。
- stored payload: pre-553 に永続化された payload は
  生成可能だった。`plan_adopt_writes` は
  `removal_supported_kind`（sv#550 の cascade 物質化
  設計 — `Removed` write は宣言 entity のみ削除し scope
  policy を再導出しない）により観測不能 kind も
  `Removed` write 化し、post-simulation gate を
  素通りする。
- → 残存経路は `synvelis#559` として別 Issue 化
  （本 slice では実装しない — project/gateway の
  境界設計を要する hardening）。

## pin test 設計

`cb06_strip_557_adopt_send_remove_candidate_
materializes_teardown`（cb06_strip_gateway.rs 末尾）:

- fixture: `boot_strip_send`（modulation entities を
  持たない send — post-553 でも candidate `send.remove`
  edit が合法な唯一の wire 経路）
- 手順: fork(`strip_scope()`) → candidate に
  `send.remove` edit commit → human approval +
  agent apply で adopt（cb10 の approval 規約に従う
  `issue_agent`/`complete`/`approve_req`/`apply_agent`
  経路）→ commit → provenance 検証
- 断言:
  - `GatewayOutcome::Accepted` + commit 成功
  - `new_adopted == candidate`・旧 adopted branch は
    freeze（`frozen != candidate`）
  - live から send config・`Send` kind の
    `RoutingConnection` edge・send-owned `SendLevel`
    `ParameterTarget` が除去（`gate.inspect` で 3 系統を
    assert）
  - provenance `branch == candidate`・`outcome ==
    "applied"`・`objects == 3`（SendConfig/
    RoutingConnection/ParameterTarget の `Removed`
    write 数）
- 意味: adopt が「candidate の scope diff を実 diff
  経由で物質化し post-simulation 認可を通る」契約を
  固定。`objects == 3` は `plan_adopt_writes` の
  `Removed` write 計画と `derive_effects` の観測範囲
  （RoutingConnection は SEC-14 境界で非観測だが
  provenance objects には計上）の両方を pin する。

## リスクと対策

- **監査の見落とし**: `AdoptIntent` の inner op 不在は
  型定義で確認済み。wire 側も `lower_adopt` が CAS のみ
  受理することを確認。→ discovery §1・§3 に根拠記録。
- **pin test が deny 経路を pin していない**: AC2 は
  「gap がある場合」の条件付きであり、本監査は gap 無し
  を結論。受理 pin は「post-simulation gate が scope
  write をカバーする」設計の固定であり、deny 側の
  残存経路は `synvelis#559` に委譲 — PR 本文に明記。
- **pre-553 stored payload の再現 test を書かない
  判断**: 現行 wire で当該 payload を生成不能なため、
  test で再現するには `BranchContentStore` への直接
  seed（test-local な payload 捏造）が必要で、domain
  の `verified_candidate` を通る正当な payload の
  捏造は test の信頼性を損なう。hardening 実装と
  セットで `synvelis#559` 側で扱うのが適切。

## 検証マトリクス

| 層 | 検証 |
| --- | --- |
| gateway check/lint/test | `mise run check/lint/test synvelis-gateway` — 新規 pin + 既存 sv#553/sv#550 pin の非回帰 |
| project check/lint/test | `mise run check/lint/test synvelis-project` — 非接触だが監査対象 domain 層の下流整合 |
| fmt/hooks | `mise run fmt-check`・`mise run check-headless-deps`・`mise exec -- prek run --all-files` |
| NotRun | GUI-L2・EXT-L3（実 PCM/plugin/RT）・MSRV・3OS 横断・full lane — POLICIES.md §5 により日常必須外 |

## 関連リンク

- `synvelis#557`（本 Issue）/ `synvelis#553` /
  `synvelis#550` / `synvelis#559` / `synvelis#283`
- `docs/discovery/issue-557-adopt-facts.md`
- `docs/specs/issue-557-adopt-facts.md`
- `docs/plans/issue-557-adopt-facts.md`
- `docs/test-specs/issue-557-adopt-facts.md`
- `docs/impl/ir/issue-553-intent-facts.md`
- `docs/impl/ir/cb-06-track-bus-adapter.md`
- `docs/designs/issue-553-intent-facts.md`
- `POLICIES.md`
- `AGENTS.md`
