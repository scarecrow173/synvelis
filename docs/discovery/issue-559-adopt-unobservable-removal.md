---
id: "DISC-559AdoptUnobservableRemoval01"
type: "discovery"
status: "resolved"
title: "Issue #559 discovery — stored candidate payload の観測不能 entity 削除 adopt fail-closed"
created: "2026-11-19"
updated: "2026-11-19"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/559"
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
    - "docs/impl/ir/issue-557-adopt-facts.md"
    - "docs/impl/ir/issue-553-intent-facts.md"
    - "docs/impl/ir/cb-06-track-bus-adapter.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/issue-559-adopt-unobservable-removal.md"
    - "docs/test-specs/cb-06-track-bus-adapter.md"
---

# Issue #559 discovery — stored candidate payload の観測不能 entity 削除 adopt fail-closed

## 背景

`synvelis#557` の監査で特定された残存経路: `synvelis#553`
（variant-edit inner facts 伝播修正）以前、つまり
`synvelis#550`（strip routing ops 着地）から `synvelis#553`
までの間に永続化された stored candidate payload を
`synvelis.branch.adopt` すると、send-owned
`ModulationSource`/`ModulationAssignment` 等の gateway
観測不能 entity の削除が fail-closed 検査なしに live へ
物質化し得る。`DirectoryBranchContentStore` は durable
であり payload は残り続ける。

## 調査結果（base `848ebb73` で再確認）

### 1. 再現（AC1 — 実測済み）

`crates/synvelis-project/tests/cb06_strip_fields.rs` に
pre-fix 再現 test
`cb_strip_559_repro_adopt_materializes_unobservable_
removal` を追加し、base HEAD で実行 — **Pass**
（`evidence/sv559/reproduction.md`）:

- fixture: track strip + return bus + key strip +
  strip→bus-strip `Send`（gain_target 未バインド）。
- fork 前に live へ send-owned modulation closure を構築
  （`ModulationSource` owner=Send・`ParameterTarget`
  owner=Send/role=SendLevel・`ModulationAssignment`
  source→target）。
- `{Sends}` mask で fork → domain `edit_branch` で
  `ProjectOp::RemoveSend` を candidate に適用 —
  `remove_control_owned` cascade が send・edge・
  SendLevel target・send-owned source・assignment を
  candidate から削除（`check_strip_fields_scope_
  removals` は `removal_supported_kind` で受理）。
- `adopt_branch_adopted` が `AdoptOutcome::Applied` で
  受理し、live から send・source・target・assignment が
  全て除去された。

### 2. fail-closed が欠けている境界

`plan_adopt_writes`
（`crates/synvelis-project/src/variant/engine.rs`）は
origin scope object が candidate scope から欠落した場合、
`entity::removal_supported_kind`
（`apply/entity.rs`）が true の kind を `Removed` write
として計画する。同 predicate は `ModulationSource`/
`ModulationAssignment`/`RoutingConnection` を含む。

`derive_effects`/`authorize_effects` の観測基盤は
`resource_map`（`synvelis-gateway/src/prepare.rs`）が
`snapshot::ENUMERATED_KINDS`（15 kind — `ResourceKind`
の全列挙）のみを走査する。`ModulationSource`/
`ModulationAssignment`/`RoutingConnection`/
`SemanticPort` は `ResourceKind` に存在せず、これらの
削除は before/after diff に一切現れない（SEC-14 境界）。
→ post-simulation gate は当該削除を観測できず、
`authorize_effects` を素通りする。

### 3. 観測不能 kind のうち fail-closed 対象となる集合

`removal_supported_kind` ∩ 観測不能 =
`{ModulationSource, ModulationAssignment,
RoutingConnection}`。うち `RoutingConnection` は
**fail-closed 対象外**とする — 既存の受理境界:

- `op_facts` の `RemoveSend` arm（synvelis#553 で追加）
  は send-owned `ModulationSource` と closure 内の entity を参照する
  `ModulationAssignment` のみを "unverifiable" として
  flag し、send↔connection edge・semantic port は
  「`ResourceRef` が名付けられない internal structure」
  として `Send` resource の lifecycle に内包する受理
  判定を既に下している（`insert-processor` の port
  posture と同じ）。
- sv#557 pin test のコメントも同じ境界を明記:
  send の専用 `Send`-kind edge は「unobservable kind、
  strictly 1:1 with the send — synvelis#550 の SEC-14
  boundary で受理」。
- `RoutingConnection` を reject 集合に含めると、send
  teardown を含む**全ての send.remove candidate が
  adopt 不能**になり、synvelis#550/#557 の受理済み
  契約を破壊する。connection 削除は wire 到達可能な
  cascade では必ず観測可能な owner 削除（Send/Processor）
  と不可分であり、owner を残して connection のみを
  消す wire op は存在しない（`RemoveConnection` 系は
  wire capability 非存在）。

→ fail-closed 対象は **`ModulationSource` と
`ModulationAssignment` の `Removed` write** に限定する。
これは `op_facts` `RemoveSend` arm が既に採用している
「unverifiable effects」分類と一致する。

### 4. 関連する別経路の観測（scope 外・記録のみ）

調査中に、candidate 保存後ではなく **live/variant-edit
適用時点**で同種の観測不能削除が起き得る経路を確認:

- `ProjectOp::SetSendGainTarget` の gain_target 付け替え/
  解除は `release_send_level_target` で旧 target の
  automation lane・**`ModulationAssignment`**・target
  自体を削除するが、`op_facts` の当該 arm は
  `unsupported` を報告しない。modulation assignment を
  持つ target の rebind/解除は wire 上も unverifiable
  cascade を報告なしに適用し得る。
- `ProjectOp::RemoveTrack` → `remove_strip_owned` →
  `remove_control_owned(Strip)` も strip-owned
  `ModulationSource`/`ModulationAssignment` を削除するが、
  `op_facts` `RemoveTrack` arm に unverifiable 検査がない
  （`track-has-no-lanes` precondition のみ）。

これらは adopt ではなく **edit/live-apply の provisional
facts 網羅性**の問題であり、本 Issue の対象外
（wire 側 honest-deny は companion Issue `synvelis#561`
の領域）。本修正の adopt fail-closed は、当該経路で
生成された candidate の adopt を正しく拒絶する。

（追記: `synvelis#561`（`48858bb1`）が着地し、
`SetSendGainTarget`/`RemoveTrack` 等の unverifiable
cascade は wire/edit prepare で正直に拒否されるように
なった — 本節の gap は解消済み。残る防御対象は
pre-#561 時代に永続化された durable stored payload
のみで、本 slice の adopt fail-closed がその防御層と
なる。）

## 結論と判断

- **AC1**: pre-fix で物質化を実測（`evidence/sv559/
  reproduction.md`・domain test Pass）。
- **AC2**: 修正方針は Issue 記載の候補 1 を採用 —
  adopt 時に planned `Removed` write の kind が gateway
  観測モデル外なら fail-closed。domain（synvelis-project）
  側に閉じた判定軸として `ObjectKind` の observability
  分類を `apply/entity.rs` に導入し、
  `plan_adopt_writes` が typed
  `VariantError::UnverifiableRemoval` で拒絶する。
  schema version 世代管理・gateway 側再検証は不採用
  （前者は全 pre-upgrade payload を巻き込み過剰、後者は
  実装コスト大・再現性の問題 — Issue 本文の評価どおり）。
- **AC3**: domain pin test で typed rejection + live
  不変を固定。gateway pin test で stored-payload 同型
  scenario（send-owned modulation entity 削除を含む
  durable candidate — sv#561 着地後は wire で生成
  不能のため domain 層 seed で注入）の adopt が
  `variant-unverifiable-removal` reason で
  `PreparationRejected` になることを固定。
- AC4: `removal_supported_kind`・`check_strip_fields_
  scope_removals`（edit-time scope policy）は非接触 —
  fail-closed は materialization の authoritative 境界
  `plan_adopt_writes` に置く。観測可能 kind
  （SendConfig/RoutingConnection/ParameterTarget/
  AutomationLane）の send teardown adopt は維持。

## 非目標

- `op_facts`/`declared_facts` の coverage 拡張
  （`SetSendGainTarget`/`RemoveTrack` 等の edit/live
  経路 unverifiable cascade — `synvelis#561` で
  着地済み）。
- candidate payload schema version・provenance 管理。
- `RoutingConnection`/`SemanticPort` 削除の reject
  （sv#550/#557 の受理済み SEC-14 境界を維持）。
- `check_strip_fields_scope_removals` の edit-time
  観測性検査追加（domain edit gate は scope policy で
  あり observability は prepare/apply の責務）。
- 実 PCM/実 plugin/実 GUI lane。
- docs index（`docs/**/README.md`）の編集・再生成
  （Coordinator が統合時に実施）。

## 関連リンク

- `synvelis#559`（本 Issue）/ `synvelis#557`（起票根拠の
  監査）/ `synvelis#553` / `synvelis#550` / `synvelis#561`
  （wire 側 honest-deny companion）/ `synvelis#283`
  （CB-06 tracker）
- `docs/specs/issue-559-adopt-unobservable-removal.md`
- `docs/impl/ir/issue-559-adopt-unobservable-removal.md`
- `evidence/sv559/`
- `docs/impl/ir/issue-557-adopt-facts.md`
- `crates/synvelis-project/src/variant/engine.rs`
  （`plan_adopt_writes`）
- `crates/synvelis-project/src/apply/entity.rs`
  （`removal_supported_kind`）
- `crates/synvelis-project/src/variant/error.rs`
  （`VariantError`）
- `crates/synvelis-gateway/src/variant.rs`
  （`variant_reason`）
- `crates/synvelis-gateway/src/prepare.rs`
  （`resource_map`/`derive_effects`/`op_facts`）
- `crates/synvelis-project/tests/cb06_strip_fields.rs`
- `crates/synvelis-gateway/tests/cb06_strip_gateway.rs`
- `POLICIES.md`
- `AGENTS.md`
