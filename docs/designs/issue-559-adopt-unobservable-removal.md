---
id: "DESIGN-559AdoptUnobservableRemoval03"
type: "design"
status: "approved"
title: "Issue #559 design — adopt 時の観測不能 entity 削除 fail-closed"
created: "2026-11-19"
updated: "2026-11-19"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/559"
    - "docs/specs/issue-559-adopt-unobservable-removal.md"
    - "docs/discovery/issue-559-adopt-unobservable-removal.md"
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
    - "docs/specs/issue-559-adopt-unobservable-removal.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/plans/issue-559-adopt-unobservable-removal.md"
    - "crates/synvelis-project/src/variant/engine.rs"
    - "crates/synvelis-project/src/apply/entity.rs"
    - "crates/synvelis-project/src/variant/error.rs"
    - "crates/synvelis-gateway/src/variant.rs"
    - "crates/synvelis-gateway/src/prepare.rs"
---

# Issue #559 design — adopt 時の観測不能 entity 削除 fail-closed

## 全体構造

fail-closed は adopt の materialization 境界
`plan_adopt_writes`（`crates/synvelis-project/src/
variant/engine.rs`）に置く。同関数は `variant::simulate`
（prepare の scratch 実行）と `variant::execute`
（commit の live 実行）の双方から呼ばれるため、
prepare で `PreparationRejected`、万が一 prepare を
迂回しても commit で `VariantCommitError::Rejected`
として fail-closed になる。

変更面:

- `crates/synvelis-project/src/apply/entity.rs` —
  観測不能 removal kind の判定 predicate を 1 本追加
  （`removal_supported_kind` の隣 — kind 分類の
  authoritative な置き場）。
- `crates/synvelis-project/src/variant/error.rs` —
  `VariantError::UnverifiableRemoval { detail }` を追加
  （`Unsupported` と別 discriminant — removal は
  supported だが検証不能、という判定を型で区別）。
- `crates/synvelis-project/src/variant/engine.rs` —
  `plan_adopt_writes` の `Removed` write 計画 loop で
  `removal_supported_kind` 通過後に新 predicate を検査し、
  該当 kind で `UnverifiableRemoval` を返す。
- `crates/synvelis-gateway/src/variant.rs` —
  `variant_reason` の exhaustive match に
  `unverifiable-removal` arm を追加（stable kebab-case
  reason — journal/provenance reader の契約）。
- `crates/synvelis-project/tests/cb06_strip_fields.rs` —
  再現 test を deny pin に置き換え。
- `crates/synvelis-gateway/tests/cb06_strip_gateway.rs` —
  観測不能 kind 削除入り stored payload の adopt 拒絶
  pin（domain 層 seed 方式）+ fixture 追加。
- issue-559 doc chain 9 件 + `issue-559-adopt-
  unobservable-removal-exp.jsonl` + `evidence/sv559/`。

## 判定軸 — なぜこの kind 集合か

### 観測モデルの正本

gateway が before/after で観測する resource は
`resource_map`（`prepare.rs`）が
`snapshot::ENUMERATED_KINDS`（15 kind）を走査して構成
する。`ResourceKind` に存在しない domain `ObjectKind`
の entity は、存在・削除・変更のいずれも effect として
現れない。

### fail-closed 対象 = `{ModulationSource,
### ModulationAssignment}`

`removal_supported_kind` が true かつ観測モデル外の kind
は `ModulationSource`/`ModulationAssignment`/
`RoutingConnection` の 3 つ。このうち:

- **`ModulationSource`/`ModulationAssignment`** — 独立した
  control graph の entity で、削除はどの観測可能 resource
  の lifecycle にも内包されない。`op_facts`
  `RemoveSend` arm（synvelis#553）が同じ集合を
  "send-owned modulation entities are outside the
  observable model" として unverifiable 判定しており、
  本修正はその分類を adopt の `Removed` write 計画へ
  適用する。owner を残して source/assignment のみを
  削除する wire op は存在しないが、owner cascade
  （send.remove・track 除去の strip closure・gain
  target release）で生成された stored payload は
  実在し得るため、adopt は kind ベースで fail-closed
  する。
- **`RoutingConnection`** — 対象外。send の専用 edge
  （`send.connection` が id 参照する 1:1 構造）であり、
  `op_facts` が send op の説明で「internal structure」
  として受理しているのと同じ境界。wire 到達可能な
  cascade では必ず観測可能な owner（Send/Processor）
  の削除と不可分であり、owner を残した connection 単独
  削除は wire op で生成不能（`RemoveConnection` 系は
  capability 非存在）。reject に含めると全 send.remove
  candidate が adopt 不能となり synvelis#550/#557 の
  受理済み契約を破壊する。

### residual（明示的受理判定の範囲）

`RoutingConnection` の `Removed` write は kind のみでは
「owner 削除に随伴する内部構造」を型レベルで証明しない
— domain API 経由で作られた connection 単独削除
candidate は原理上 adopt を通り得る。対象は send 温存
ケースに限らず、scope 所有の全 edge（send 専用 edge・
Main route・Sidechain・chain-internal）を含む。
ただし当該 payload は wire 経路では生成不能であり
（`RemoveConnection`/`AddConnection`/`ConfigureBus` 系は
capability 非存在）、domain API 呼出は trusted 内部
コードの責務（apply と同じ trust level）とみなす。
この residual は discovery/spec に記録する。wire 側の
honest-deny 強化は `synvelis#561`（`48858bb1`）で
着地済み — 本 slice の gateway pin は当該 wire 拒否を
迂回せず、domain 層 seed で durable payload を注入する。

同じ観測モデル外の非対称として、`plan_adopt_writes`
第 2 loop の `Added`/`Modified` write は本修正の対象外
— 観測不能 kind の追加・変更も `derive_effects` を
素通りするが、synvelis#550 以来の採用済み設計であり、
本 Issue の durable 削除 payload 範囲を超えるため
follow-up 記録に留める。

## 実装詳細

### `entity.rs` — predicate

```rust
pub(crate) fn unverifiable_removal_kind(kind: ObjectKind) -> bool {
    matches!(
        kind,
        ObjectKind::ModulationSource | ObjectKind::ModulationAssignment
    )
}
```

`removal_supported_kind` の直後に配置し、doc comment で
「removal は supported だが削除効果が観測モデル外」
という判定の意味と `RoutingConnection` を含めない
理由（sv#550/#557 の受理済み SEC-14 境界）を記す。

### `error.rs` — variant

```rust
/// A scope entity removal is structurally supported but its
/// deletion is outside the observable effect model — the adopt
/// cannot verify what it would destroy (synvelis#559).
UnverifiableRemoval {
    /// What is unverifiable and why.
    detail: String,
},
```

`Unsupported` の隣に配置。`Display` は
`"unverifiable removal: {detail}"`。

### `engine.rs` — `plan_adopt_writes`

origin loop 内、`removal_supported_kind` 検査の直後:

```rust
if entity::unverifiable_removal_kind(origin.kind) {
    return Err(VariantError::UnverifiableRemoval {
        detail: format!(
            "scope entity removal has no observable effect to authorize: {:?}: {}",
            origin.kind, origin.stable_id
        ),
    });
}
```

関数 doc の fail-closed rules に新 verdict を追記。

### `variant.rs` — `variant_reason`

```rust
VariantError::UnverifiableRemoval { detail } => {
    ("unverifiable-removal", detail.clone())
}
```

→ wire reason は `variant-unverifiable-removal: {detail}`
（simulate 失敗時は `variant simulation failed: ` prefix
付きで `PreparationRejected` に載る）。

## 変更しないもの

- `removal_supported_kind` — edit-time scope policy
  （`check_strip_fields_scope_removals`）と adopt の
  materialization 可否は別責務。observability は
  prepare/apply の権限であり、edit gate に持ち込まない。
- `op_facts`/`declared_facts` — edit/live-apply 経路の
  coverage は `synvelis#561`（`48858bb1`）で着地済み。
- `VariantIntent::Adopt` の `declared_facts` — gap 無し
  （synvelis#557 実証済み）。
- candidate payload schema/provenance — 採用方針は
  kind 分類であり世代管理ではない。

## 検証設計

- domain pin: 同一 scenario（send-owned modulation
  closure を持つ send の `RemoveSend` candidate）で
  `try_adopt` が `VariantError::UnverifiableRemoval`
  を返し、live の send/source/target/assignment と
  project revision・adopted branch が不変であること。
- gateway pin: `ModulationAssignment`/`ModulationSource`
  を削除した candidate の adopt prepare が
  `PreparationRejected` / `variant-unverifiable-removal`
  で拒絶され、live 不変であること。payload は
  domain 層の `fork_variant_set` + `edit_branch_op`
  （`RemoveSend` cascade）で rig の shared
  `BranchContentStore` に生成・seed する —
  post-sv#561 では wire の candidate edit が同
  cascade を `op_facts` unverifiable として
  prepare で正直に拒否するため、wire op では
  当該 durable payload を新規生成不能。
- 非回帰: synvelis#550 send teardown adopt・
  synvelis#553/#557 の既存 pin が全て Pass。

## 関連リンク

- `synvelis#559`（本 Issue）
- `docs/specs/issue-559-adopt-unobservable-removal.md`
- `docs/discovery/issue-559-adopt-unobservable-removal.md`
- `docs/plans/issue-559-adopt-unobservable-removal.md`
- `docs/test-specs/issue-559-adopt-unobservable-removal.md`
- `evidence/sv559/`
- `synvelis#557` / `synvelis#553` / `synvelis#550` /
  `synvelis#561`
- `POLICIES.md`
- `AGENTS.md`
