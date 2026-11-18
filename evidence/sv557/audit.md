# sv557 — VariantIntent::Adopt declared_facts 伝播監査 evidence

Issue: `scarecrow173/synvelis#557` / branch
`feature/557-adopt-facts-audit`（base: `origin/main`
`bc90f0e0`、前提 PR `synvelis#555`/`synvelis#556`/
`synvelis#558` merge 済み）
OS: Windows（mise managed toolchain）。cargo 直接実行は
補助証拠、canonical = mise。

## 監査対象

`synvelis#553`（PR `synvelis#556`）で修正した
`declared_facts` Edit arm の inner `OpFacts` 伝播と
同型の gap が `VariantIntent::Adopt` 経路に残っていないか。

## 確認したコード箇所と結論

### 1. `AdoptIntent` に inner op は存在しない（gap の構造的不成立）

- `crates/synvelis-project/src/variant/types.rs:411-426`:
  `AdoptIntent` は `variant_set`/`branch`/
  `expected_project_revision`/`expected_branch_revision`/
  `expected_scope_digest`/`approval` のみ。inner
  `ProjectOp` 相当のフィールドは無い。
- `crates/synvelis-gateway/src/variant.rs` の
  `declared_facts` Adopt arm（`:718-723` 付近）は
  `cover_set` のみ呼ぶ。Edit arm（`:700-716`）が
  `crate::prepare::op_facts` の inner `reads`/
  `preconditions`/`unsupported`/条件付き `impacted` を
  伝播するのは `BranchEditOp::Project(op)` が inner op
  を持つから — Adopt には対応物がなく、伝播すべき
  inner facts は型レベルで存在しない。
- wire 側も同じ: `lower_adopt`（variant.rs:808-815）は
  CAS 4 項目のみ受理し op payload を取らない。

### 2. adopt の write 認可は post-simulation gate が実 diff で行う

`PreparedMutation::Variant` の prepare
（`crates/synvelis-gateway/src/prepare_engine.rs`）は
`declared_facts` → provisional gate →
`variant::simulate` → `resource_map` before/after →
`derive_effects` → `authorize_effects` の順で実行。

adopt の domain 拒絶は全て `simulate` 内で起き、
`PreparationRejected`（"variant simulation failed:
variant-*"）として provisional gate に届く:

- `adopt_guard`（CAS 3 項目 + approval ticket）
- `verified_candidate`（payload decode + scope digest
  再検証 — 改竄・stale payload はここで拒絶）
- adapter `preflight_adopt`
- `plan_adopt_writes`
  （`crates/synvelis-project/src/variant/engine.rs:817-909`
  — scope 外移動・removal 非対応 kind は typed
  `VariantError::Unsupported`、lineage 不明は
  `AmbiguousLineage` で fail-closed）
- asset/plugin/port preflight → `controller.apply`

provisional gate の `impacted` が adopt では
`{VariantSet, Branch}` のみでも、post-simulation の
`derive_effects` + `authorize_effects`（GW-02）が実
diff を認可するため scope root への write はカバー
される — facts-drop 型の穴ではない。

### 3. 観測不能削除の wire 到達性 — 現行 wire では生成不能

`ResourceKind`（`crates/synvelis-gateway/src/
request.rs:31-65`）と `snapshot::ENUMERATED_KINDS` は
`ModulationSource`/`ModulationAssignment`/
`RoutingConnection`/`SemanticPort` を含まない —
これらの削除は `derive_effects` で観測不能
（SEC-14 境界）。

candidate payload に不可視削除を混入させうる wire op
は **send.remove のみ**で、send-owned modulation
entities 存在時は synvelis#553 以降 edit prepare で
"unverifiable effects" deny（pin:
`cb06_strip_553_variant_edit_send_remove_
unobservable_cascade_denied`、
cb06_strip_gateway.rs:1575-1597）。`RemoveConnection`/
`AddConnection`/`ConfigureBus`/modulation 系 op は
wire capability 非存在。**現行 wire で不可視削除入り
candidate を新規生成する経路は存在しない**。

### 4. 唯一の残存経路 — pre-553 stored candidate payload

`DirectoryBranchContentStore` は durable であり、
synvelis#550（send op 着地）から synvelis#553
（facts 伝播修正）までの間に永続化された candidate
payload は、send-owned modulation entities を持つ send
への `send.remove` 効果を含んだまま残り得る。

その payload を今 adopt すると:

1. `adopt_guard`/`verified_candidate` は payload が
   自己整合的で scope digest も一致するため通過。
2. `plan_adopt_writes` が origin scope object の
   candidate 欠落を `removal_supported_kind`
   （`crates/synvelis-project/src/apply/entity.rs:
   111-125` — Clip/Note/ParameterTarget/
   AutomationLane/ModulationSource/ModulationAssignment/
   RoutingConnection/SendConfig）で `Removed` write 化。
   観測不能 kind も対象 — sv#550 の cascade 物質化
   設計（`Removed` write は宣言 entity のみ削除し
   scope policy を再導出しない）。
3. post-simulation `derive_effects` は観測不能 kind
   の削除を観測しないため `authorize_effects` を
   素通りし、live に物質化する。

→ hardening は project/gateway 双方の境界設計を要し
本 slice の範囲外。**`synvelis#559` として起票済み**
（payload provenance/schema 世代管理・adopt 時
observability fail-closed 等の方針候補を本文に記録。
着手条件なし・即着手可能）。

## 受理側 pin test（AC2 — gap 無しのため受理契約を固定）

`cb06_strip_557_adopt_send_remove_candidate_
materializes_teardown`（cb06_strip_gateway.rs 末尾）:

- `boot_strip_send` fixture — modulation entities を
  持たない send（post-553 で candidate `send.remove`
  edit が合法な唯一の wire 経路）
- fork → candidate `send.remove` edit commit → live
  は adopt まで send を保持
- human approval + agent apply（cb10 規約）で adopt
  → `GatewayOutcome::Accepted` + commit 成功
- provenance: `branch == candidate`・`outcome ==
  "applied"`・`objects == 3`
- live から send config・`Send` kind `RoutingConnection`
  edge・send-owned `SendLevel` `ParameterTarget` が
  除去、adopted branch が candidate に更新・旧
  branch freeze

実測: `mise run test synvelis-gateway` で Pass
（`verification.md` 参照）。

## Edit 経路非回帰（AC3）

`cb06_strip_553_variant_edit_send_remove_
unobservable_cascade_denied`・`cb06_strip_550_
send_remove_unobservable_cascade_denied` を含む
`cb06_strip_gateway` 全 9 件が `mise run test
synvelis-gateway` で Pass — Edit 経路は非接触で
回帰なし。

## 判断サマリ

- **AC1**: gap 無しの根拠を本ファイル・doc chain・
  IR に記録（上記 1–4）。
- **AC2**: gap が無いため deny pin は条件不成立。
  受理側 pin を追加（上記）。残存経路の deny は
  `synvelis#559` に委譲。
- **AC3**: Edit 経路非回帰を `mise run test
  synvelis-gateway` で実測。
- cb-06 doc chain 3 ファイルに監査結果の追補を実施
  （記述の stale は無し・監査結果の記録として追補）。

## 関連リンク

- `synvelis#557`（本 Issue）/ `synvelis#553` /
  `synvelis#550` / `synvelis#559` / `synvelis#283`
- `docs/discovery/issue-557-adopt-facts.md`
- `docs/impl/ir/issue-557-adopt-facts.md`
- `docs/impl/exp/issue-557-adopt-facts.md`
- `docs/impl/ir/issue-553-intent-facts.md`
- `docs/impl/ir/cb-06-track-bus-adapter.md`
- `evidence/sv557/verification.md`
- `crates/synvelis-gateway/src/variant.rs`
- `crates/synvelis-gateway/src/prepare_engine.rs`
- `crates/synvelis-gateway/src/request.rs`
- `crates/synvelis-project/src/variant/types.rs`
- `crates/synvelis-project/src/variant/engine.rs`
- `crates/synvelis-project/src/apply/entity.rs`
- `crates/synvelis-gateway/tests/cb06_strip_gateway.rs`
- `POLICIES.md`
