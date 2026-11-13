---
id: "DESIGN-taIsR3SbHTaGKnhYkfth7f"
type: "design"
status: "approved"
title: "CTL-02 adoption SendLevel canonical spec 強制 設計"
created: "2026-11-13"
updated: "2026-11-13"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/122"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/ctl-02-adopt-sendlevel.md"
    - "docs/adr/ctl-02-adopt-sendlevel.md"
  implemented-by: []
  depends-on:
    - "docs/specs/ctl-02-adopt-sendlevel.md"
    - "docs/adr/ctl-02-adopt-sendlevel.md"
    - "docs/impl/ir/ctl-02-strip-send-host-parameter-plugin-binding-routing.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/ctl-02-adopt-sendlevel.md"
    - "docs/adr/ctl-02-adopt-sendlevel.md"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "docs/designs/ctl-01-control-owner-time-scope-design.md"
  defers: []
  deferred-by: []
---

# CTL-02 adoption SendLevel canonical spec 強制 設計

## Context

`crates/synvelis-project/src/ops.rs` の `ProjectOp::AddSend` /
`SetSendGainTarget` validate は、adopt 対象 target の存在・
well-known bootstrap guard・project-time scope・二重 bind・
Strip/Processor owner guard を検査するが、persisted spec
（kind/mapping/time_scope/capability）の canonical 契約を検査
しない。apply（同ファイル `Self::AddSend` / `Self::SetSendGainTarget`
腕）は `target.owner = ControlOwner::Send(..)`・
`target.role = ParameterRole::SendLevel` を seal するのみで spec
を触らない。

`canonical_internal_spec`（`src/control.rs`）は
`Strip`+channel role と `Send`+`SendLevel` に canonical
`(kind, mapping, time_scope)` を返し、`AddParameterTarget`
validate が capability `None` と併せて強制する — しかし adopt
経路は `AddParameterTarget` を通らない。

編集範囲は `crates/synvelis-project/` と `docs/` の
`ctl-02-adopt*` chain のみ。

## Scope

### In Scope

- `src/control.rs`: `Send`+`SendLevel` canonical spec の単一
  source 化と採用前判定 helper。
- `src/ops.rs`: `AddSend` / `SetSendGainTarget` validate への
  canonical spec 検査追加。
- `tests/ctl_02.rs`: adoption canonical 契約の fixture 追加。
- `docs/` chain + 管理 index 再生成。

### Out of Scope

- spec の Out of Scope と同一（`SendGainSpec::Create` 受理の変更・
  他 role/owner/capability 利用・他 crate・load-time oracle・
  Issue #122 外部 gate 残件）。

## Components and Boundaries

### Canonical spec 単一 source（`control.rs`）

```rust
/// `Send`+`SendLevel` の canonical persisted spec
/// `(kind, mapping, time_scope)` — capability は常に None。
/// `canonical_internal_spec` と採用検査が共有する定義。
pub(crate) fn send_level_spec()
    -> (ValueKind, ParamMapping, AutomationTimeScope)
// => (Continuous, Linear, ProjectMusical)

/// target が canonical SendLevel spec を既に満たすか。
/// name/base は契約対象外。
pub(crate) fn has_canonical_send_level_spec(t: &ParameterTarget) -> bool
```

- `canonical_internal_spec` の `Send`+`SendLevel` 分岐を
  `send_level_spec()` 呼び出しに置き換え、strip channel 分岐は
  従来どおり `strip_channel_spec` 由来 — 2 契約が 1 ファイル内で
  定数を共有し、adoption 検査と `AddParameterTarget` 検査が乖離
  しない構造にする。
- `has_canonical_send_level_spec` は `kind`/`mapping`/
  `time_scope`/`capability.is_none()` の 4 条件を判定する純粋
  helper。`ParameterTarget` は `pub` 型だが helper は
  `pub(crate)` — write-time 契約の内部実装詳細を公開 API に
  しない。

### Adoption 検査（`ops.rs` validate）

`Self::AddSend` / `Self::SetSendGainTarget` の
`if let Some(target) = gain_target` ブロック内、既存 guard
（well-known → project-time → already-bound → Strip/Processor
owner）の後に追加:

```rust
if !crate::control::has_canonical_send_level_spec(t) {
    return Err(Error::Validation(format!(
        "param {target} does not carry the canonical send level spec"
    )));
}
```

- 検査対象は adopt 前の authored spec — apply が owner/role を
  seal するため、post-adoption の `(Send, SendLevel)` に対する
  `canonical_internal_spec` と同じ 4 条件を pre-adoption の
  spec fields に適用するのが等価であり、`SendId` 未発行の
  `AddSend` でも判定できる。
- 既存の project-time 検査（`ProjectMusical | ProjectAbsolute`）
  は残す — canonical 検査が `ProjectMusical` を要求するため
  `ProjectAbsolute` は新検査で reject されるが、検査順序上
  より具体的な既存 error を先に返す構成は維持する
  （最終的な reject 理由は canonical spec メッセージ）。
- apply 側（owner/role seal）は無変更 — persisted される
  `SendLevel` は validate 通過後にのみ canonical spec を持つ。

### Transaction 経路

`RoutingEdit::CreateSend { level: SendGainSpec::Adopt(t) }` と
`RoutingEdit::SetSendGainTarget { target: Some(t) }` は
`staging.apply_op(&ProjectOp::AddSend/SetSendGainTarget)` に
stage され、`apply_op` が `op.validate` → `op.apply` を通す。
したがって op 層の検査追加だけで両経路が覆われ、staging 固有の
重複検査は追加しない（DRY: 契約は op validate が authoritative）。
`SendGainSpec::Create` は staging が canonical
`AddParameterTarget` を発行してから `SetSendGainTarget` で bind
するため、新検査をそのまま通過する。

## Data and Control Flow

```
caller
 ├─ ProjectOp::AddSend { gain_target: Some(t) }      (低層)
 ├─ ProjectOp::SetSendGainTarget { gain_target: Some(t) } (低層)
 ├─ RoutingEdit::CreateSend { level: Adopt(t) }     → ProjectOp::AddSend
 └─ RoutingEdit::SetSendGainTarget { target: Some(t) } → ProjectOp::SetSendGainTarget
        │
        ▼  op.validate(project)
   既存 guard 列（NotFound → well-known → project-time →
   already-bound → Strip/Processor owner）
        │
        ▼  新規: has_canonical_send_level_spec(t)
   kind==Continuous && mapping==Linear &&
   time_scope==ProjectMusical && capability.is_none()
   └─ 違反 → Error::Validation（project 無変更・undo 不変）
        │
        ▼  op.apply
   owner=Send(send_id), role=SendLevel を seal
   （kind/mapping/time_scope/capability/name/base は authored のまま
    — 全て canonical 契約を満たすことが validate で保証済み）
```

## Non-Functional Constraints

- persisted 形状の変更は「非 canonical が書けなくなる」方向のみ
  — canonical な既存 state・snapshot・fixture の読み書きは不変。
- `SendGainSpec::Create`・strip channel factory・migration の
  挙動は不変（単一 source 参照のみの内部整理）。
- lint: workspace `[lints]`（clippy `-D warnings`）適合、
  MSRV 1.89 / edition 2024。

## Risks and Trade-offs

- **`ProjectAbsolute` adoption の reject は意図的な契約狭め**:
  旧来「project-time なら可」だった target のうち
  `ProjectAbsolute` は canonical `SendLevel`（`ProjectMusical`
  固定）と一致せず reject される。automation 座標系の異なる
  target を send gain に転用する需要は契約上存在しないため
  fail-closed を選ぶ（ADR Decision 4）。
- **capability 持ち自由形式 target の adopt も reject**:
  `capability: Some` は plugin `ParamId` binding であり、
  send level（内部 gain・ParamId なし）に転用すると binding が
  実体と乖離する — reject が正しい。
- **既存 persisted 非 canonical `SendLevel` は対象外**:
  load-time oracle（`validate_references`）は本 slice で拡張
  しない。write 経路を塞いだ上で、既存データへの対処が必要なら
  別 slice で migration/検査を検討する。

## Deferred Design Concerns

- `validate_references` への canonical spec oracle 追加
  （load-time fail-closed 化）は別 slice の判断事項。
- Issue #122 残件の外部 gate（実 Plugin/OS・#103/#109 Domain
  Oracle・AudioLens・実 Plugin verified binding 受入・Device
  Oracle）は NotRun/Deferred のまま。

## Verification Notes

- canonical: `mise run fmt-check` / `check` / `lint` / `test` /
  `test-rt` / `check-app-native` / `lint-app-native` /
  `test-msrv` / `build`。
- test-spec `docs/test-specs/ctl-02-adopt-sendlevel.md` の各
  Feature が `tests/ctl_02.rs` の fixture で担保されること。
