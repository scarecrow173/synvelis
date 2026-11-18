---
id: "IR-557AdoptFacts07"
type: "impl"
status: "completed"
title: "Issue #557 impl IR — adopt declared_facts 監査 実装記録"
created: "2026-11-18"
updated: "2026-11-18"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/557"
  changes:
    added:
      - type: "file"
        path: "docs/discovery/issue-557-adopt-facts.md"
      - type: "file"
        path: "docs/specs/issue-557-adopt-facts.md"
      - type: "file"
        path: "docs/designs/issue-557-adopt-facts.md"
      - type: "file"
        path: "docs/plans/issue-557-adopt-facts.md"
      - type: "file"
        path: "docs/tasks/issue-557-adopt-facts.md"
      - type: "file"
        path: "docs/test-specs/issue-557-adopt-facts.md"
      - type: "file"
        path: "docs/impl/exp/issue-557-adopt-facts.md"
      - type: "file"
        path: "docs/impl/exp/issue-557-adopt-facts-exp.jsonl"
      - type: "dir"
        path: "evidence/sv557/"
    modified:
      - type: "file"
        path: "crates/synvelis-gateway/tests/cb06_strip_gateway.rs"
      - type: "file"
        path: "docs/test-specs/cb-06-track-bus-adapter.md"
      - type: "file"
        path: "docs/impl/ir/cb-06-track-bus-adapter.md"
      - type: "file"
        path: "docs/impl/exp/cb-06-track-bus-adapter.md"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/issue-557-adopt-facts.md"
  implemented-by: []
  depends-on:
    - "docs/plans/issue-557-adopt-facts.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/ir/issue-553-intent-facts.md"
    - "docs/impl/ir/cb-06-track-bus-adapter.md"
    - "docs/impl/exp/issue-557-adopt-facts.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/plans/issue-557-adopt-facts.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/issue-557-adopt-facts.md"
  references: []
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted:
      - "docs/impl/exp/issue-557-adopt-facts.md"
    rejected: []
---

# Issue #557 impl IR — adopt declared_facts 監査 実装記録

## 実装サマリ

`VariantIntent::Adopt` 経路の `declared_facts` 伝播を監査
し、**synvelis#553 同型の gap は構造上存在しない**ことを
確認した。production code（`variant.rs` を含む）は
変更せず、監査根拠の記録・adopt 受理 pin test・残存経路の
hardening Issue 起票で slice を完結した。

## 監査結果（AC1 — gap 無しの根拠）

### `declared_facts` Adopt arm（variant.rs:718-723 付近）

Adopt arm は `cover_set`（`VariantSet`/`Branch`/
`Project`/scope roots の reads + `VariantSet`/`Branch` の
impacted）のみを返す。Edit arm（同:700-716）が
`crate::prepare::op_facts` の inner `reads`/
`preconditions`/`unsupported`/条件付き `impacted` を
伝播するのは `BranchEditOp::Project(op)` が inner
`ProjectOp` を持つからであり、**`AdoptIntent`
（`synvelis-project/src/variant/types.rs:411-426`）は
set/branch/CAS/approval フィールドのみで inner op を
持たない** — 伝播すべき inner facts は型レベルで存在
しない。wire 側も `lower_adopt`（variant.rs:808-815）が
CAS 4 項目のみ受理し op payload を取らない。

### adopt の write 認可は post-simulation gate が実 diff で行う

`PreparedMutation::Variant` の prepare
（`prepare_engine.rs`）は `declared_facts` → provisional
gate → `variant::simulate` → `resource_map`
before/after → `derive_effects` → `authorize_effects`
の順。adopt の domain 拒絶（`adopt_guard` CAS/
`verified_candidate` payload+digest 再検証/
`preflight_adopt`/`plan_adopt_writes` の typed
`Unsupported`・`AmbiguousLineage`/asset-plugin-port
preflight/`controller.apply`）は全て `simulate` 内で
起き `PreparationRejected`（"variant simulation
failed: variant-*"）として届く。provisional
`impacted` に scope roots が無くても、post-simulation
の `derive_effects` + `authorize_effects`（GW-02）が
実 diff を認可するため scope write はカバーされる
— facts-drop 型の穴ではない。

### 観測不能削除の wire 到達性

`ResourceKind`（`request.rs:31-65`）と
`snapshot::ENUMERATED_KINDS` は `ModulationSource`/
`ModulationAssignment`/`RoutingConnection`/
`SemanticPort` を含まない（SEC-14 境界）。candidate
payload に不可視削除を混入させうる wire op は
send.remove のみで、send-owned modulation entities
存在時は synvelis#553 以降 edit prepare で deny
（pin: `cb06_strip_553_variant_edit_send_remove_
unobservable_cascade_denied`）。`RemoveConnection`/
`AddConnection`/`ConfigureBus`/modulation 系 op は
wire capability 非存在 → **現行 wire で不可視削除入り
candidate を新規生成不能**。

### 残存経路 — pre-553 stored candidate payload

`DirectoryBranchContentStore` は durable。
synvelis#550（send op 着地）〜synvelis#553（facts
伝播修正）の間に永続化された candidate payload は、
send-owned modulation entities を持つ send への
`send.remove` 効果を含み得る。その payload を今
adopt すると domain guard は全通過し、
`plan_adopt_writes` が `removal_supported_kind`
（`apply/entity.rs:111-125` — sv#550 の cascade
物質化設計）で観測不能 kind を `Removed` write 化、
post-simulation gate を素通りして live に物質化する。

→ hardening は project/gateway 境界の設計を要し本
slice 範囲外のため **`synvelis#559` として起票**
（payload provenance/schema 世代管理・adopt 時
observability fail-closed 等の方針候補を本文に記録。
着手条件なし・即着手可能）。

## 変更内容

### test 追加（AC2 — 受理側 pin）

`crates/synvelis-gateway/tests/cb06_strip_gateway.rs`
末尾に `cb06_strip_557_adopt_send_remove_candidate_
materializes_teardown` を追加:

- `boot_strip_send` fixture（modulation entities を
  持たない send — post-553 で candidate `send.remove`
  edit が合法な唯一の wire 経路）
- fork → candidate `send.remove` edit commit →
  live は adopt まで send 保持を assert
- human approval + agent apply の cb10 規約で adopt
  → `GatewayOutcome::Accepted` + commit 成功
- provenance: `branch == candidate`・`outcome ==
  "applied"`・`objects == 3`
- live から send config・`Send` kind edge・
  send-owned `SendLevel` `ParameterTarget` が除去、
  adopted branch が candidate に更新・旧 branch freeze

deny pin は AC2 の条件（gap 有り）不成立のため不実施
— 残存経路は `synvelis#559` に委譲。

### doc chain + cb-06 追補 + evidence

- issue-557 doc chain 8 件 + exp.jsonl を新規作成
- cb-06 doc chain 3 ファイルに監査結果の追補
- `evidence/sv557/` に audit 記録 + verification 記録

### 非変更の明示

- `crates/synvelis-gateway/src/variant.rs` — gap が
  実証されなかったため Adopt arm は非接触
- `crates/synvelis-project/**`・`tests/rig/mod.rs`・
  他テストファイル・`docs/**/README.md` — 全て非接触

## 検証結果

`evidence/sv557/verification.md` に全記録。Issue 指定の
必須コマンド全て exit 0:

- `mise run check synvelis-gateway` / `lint` / `test`
- `mise run check synvelis-project` / `lint` / `test`
- `mise run fmt-check` / `check-headless-deps` /
  `prek run --all-files`

`cb06_strip_gateway` は 9 件全 Pass — 新規
`cb06_strip_557_*`・既存 `cb06_strip_553_*`/
`cb06_strip_550_*` pin を含む非回帰を実測。

## review 対応

`review-protocol` を 1 回実施。blocker 0・verdict
ACCEPT。確認した候補と判定は PR 本文「review-protocol
のレビュー結果」節に記録。

## 残件

- `synvelis#559` — stored candidate payload の
  観測不能 entity 削除 hardening（本監査の残存経路。
  project/gateway 境界の設計を要するため別 Issue）。
- docs index（`docs/**/README.md`）未同期 —
  Coordinator が統合時に再生成。
- `evidence/sv283/` CB-06 tracker ledger への監査結果
  追記 — lease 外のため未実施。PR 本文「未対応の
  作業」に記録。

## 関連リンク

- `synvelis#557`（本 Issue）/ `synvelis#553` /
  `synvelis#550` / `synvelis#559` / `synvelis#283`
- `docs/discovery/issue-557-adopt-facts.md`
- `docs/specs/issue-557-adopt-facts.md`
- `docs/designs/issue-557-adopt-facts.md`
- `docs/plans/issue-557-adopt-facts.md`
- `docs/tasks/issue-557-adopt-facts.md`
- `docs/test-specs/issue-557-adopt-facts.md`
- `docs/impl/exp/issue-557-adopt-facts.md`
- `docs/impl/ir/issue-553-intent-facts.md`
- `docs/impl/ir/cb-06-track-bus-adapter.md`
- `evidence/sv557/`
- `crates/synvelis-gateway/src/variant.rs`
- `crates/synvelis-gateway/src/prepare_engine.rs`
- `crates/synvelis-project/src/variant/engine.rs`
- `crates/synvelis-project/src/apply/entity.rs`
- `crates/synvelis-gateway/tests/cb06_strip_gateway.rs`
- `POLICIES.md`
