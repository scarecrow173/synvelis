---
id: "DESIGN-552SendGainSeal03"
type: "design"
status: "approved"
title: "Issue #552 design — SendGainTarget rebind の seal 解放（削除 vs unseal の確定）"
created: "2026-11-18"
updated: "2026-11-18"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/552"
    - "docs/specs/issue-552-sendgain-seal.md"
    - "docs/discovery/issue-552-sendgain-seal.md"
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
    - "docs/specs/issue-552-sendgain-seal.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/plans/issue-552-sendgain-seal.md"
    - "docs/designs/ctl-02-adopt-sendlevel-design.md"
    - "crates/synvelis-project/src/ops.rs"
    - "crates/synvelis-project/src/variant/adapter.rs"
---

# Issue #552 design — SendGainTarget rebind の seal 解放

## Context

`crates/synvelis-project/src/ops.rs` の
`ProjectOp::SetSendGainTarget` apply arm は
`send.gain_target` を差し替え新 target に
`owner = Send(send_id)`・`role = SendLevel` を seal
するが、旧 target の seal を解放しない。
`ParameterRole::SendLevel` は `(owner, role)`
singleton であり、post-apply oracle の
`validate_control_binding` が同一 send owner の
重複 `SendLevel` を必ず拒否するため、Some→Some
rebind と Some→None→Some（孤立 seal 残存経由）は
commit 不能だった。sv#550 は
`variant/adapter.rs::check_strip_routing_op_targets`
で typed `Unsupported` として封じ込めた。

編集範囲は `crates/synvelis-project/` の lease 済み
4 ファイルと doc chain / evidence のみ。

## Scope

### In Scope

- `src/ops.rs`: `SetSendGainTarget` apply arm での
  旧 seal 解放（release）と新 seal の不可分適用、
  および単一 target 粒度の解放 helper。
- `src/variant/adapter.rs`: 同 op の typed
  `Unsupported` を positive 受理へ置換。
- `tests/ctl_02.rs`・`tests/cb06_strip_fields.rs`:
  rebind/unconfigure の positive 実測。
- `docs/` chain + `evidence/sv552/`。

### Out of Scope

- spec の Out of Scope と同一（wire/gateway 変更・
  send 以外の seal 系 op・automation 移送
  semantics・cb-06 系文書）。

## Components and Boundaries

### 解放 semantics の確定 — 旧 target の削除を採る

rebind/unconfigure で `send.gain_target` が
`Some(old)` から別値へ遷移したとき、旧 target が
実際に `(Send(send_id), SendLevel)` seal を保持
していれば、その target を中心とした排他閉包を
削除する:

- `old` を `target` とする `AutomationLane`。
- `old` を `target` または `depth_source` とする
  `ModulationAssignment`。
- `old` の `ParameterTarget`。

send-owned modulation source と send の他 role
target は残す（`remove_control_owned` の owner
単位カスケードではなく target 単位 — send は
`SendLevel` 以外の control entity を追加所有し得る
ため、owner 単位では無関係な entity を巻き込む）。

### 検討した代替案と棄却理由

- **unseal（owner 差替）**: 旧 target の owner を
  `ControlOwner::Project` 等へ動かし role を
  `Generic` へ落とす案。(a) strip-fields scope の
  所有導出は send-owned target を role 不問で
  scope object とするため、send 以外への owner
  差替は entity 残存のまま scope object 集合を
  離脱し、`check_strip_fields_scope_removals` が
  「entity が残る move-out」として typed reject
  する — candidate 経路と live 経路で semantics
  が分岐する。(b) adopt 時に失われた旧 owner
  （例: Track 所有 Generic）を復元する情報は
  persisted されておらず、`Project`+`Generic`
  への退避は provenance を捏造する。(c) automation
  を「誰も読まない param」に残すのは seal 解放
  ではなく死んだデータの温存。
- **unseal（owner=Send 維持・role のみ降格）**:
  scope object には残るため removal check は
  通るが、「send が所有する level でない param」
  という domain 未定義の中間状態を新設し、
  付随 automation を実質死んだまま残す。削除
  より一貫した semantics を持たない。
- **automation の新 target への移送**: lane の
  region は target 非依存の点列であり技術的には
  付け替え可能だが、「rebind は gain の
  param 参照を変える操作であり automation の
  編集ではない」という op の責務を越える。
  補間・競合解決の semantics を本 slice で
  新設するのは scope 超過 — 別 Issue の領域。

削除は `RemoveSend` の `remove_control_owned`
（seal が死ぬときその閉包も死ぬ）・sv#550 の
send teardown adopt（send・edge・target・lane を
除去）・`removal_supported_kind` の既収録 kind
という 3 つの先例と整合し、`synvelis#122` の
「孤立した Persisted Gain を残さない」契約に
最も素直に従う。

### apply arm の変更（`ops.rs`）

```rust
Self::SetSendGainTarget { send_id, gain_target } => {
    let previous = project
        .routing
        .sends
        .get(send_id)
        .and_then(|send| send.gain_target);
    if let Some(send) = project.routing.sends.get_mut(send_id) {
        send.gain_target = *gain_target;
    }
    if let Some(old) = previous
        && Some(old) != *gain_target
        && project.control.targets.get(&old).is_some_and(|target| {
            target.owner == ControlOwner::Send(*send_id)
                && target.role == ParameterRole::SendLevel
        })
    {
        release_send_level_target(project, old);
    }
    // 従来どおり新 target を seal
    if let Some(t) = gain_target
        && let Some(target) = project.control.targets.get_mut(t)
    {
        target.owner = ControlOwner::Send(*send_id);
        target.role = ParameterRole::SendLevel;
    }
}
```

- `previous` を `gain_target` 上書き前に捕捉する。
- release の発火条件は「旧参照があり」「新参照と
  異なり」「旧 target が実際に当該 send の
  `SendLevel` seal を保持」の 3 条件 — 再 assert
  （同一 target）や seal を持たない参照
  （あり得ないが oracle 未検査の手細工 state への
  防御）では発火しない。
- `release_send_level_target` は lane/assignment の
  `retain` と target の `remove` のみで失敗しない
  — apply arm 全体が不可分で、controller は
  scratch clone への apply + `validate_references`
  通過後にのみ commit するため、release と seal の
  間に観測可能な半端状態は存在しない。

### adapter の変更（`variant/adapter.rs`）

`check_strip_routing_op_targets` の
`SetSendGainTarget` arm から typed `Unsupported`
分岐を除去し、send の `SendConfig` scope 検査と
新 target の `ParameterTarget` scope 検査のみを
残す。release で離脱する scope object（旧
target・その lane・assignment）は
`check_strip_fields_scope_removals` が supported
deletion として受理する — `ParameterTarget`・
`AutomationLane`・`ModulationAssignment` は全て
`removal_supported_kind` 収録済みで、新 kind の
追加は不要。

### 影響しない面

- `control.rs`（singleton 検査・canonical spec
  helper）・`project.rs`（post-apply oracle）・
  `routing_tx.rs`（lowering）・`apply/entity.rs`
  （`removal_supported_kind` は既収録）・
  gateway wire surface — 全て無変更で整合する。
- `RemoveSend`/`AddSend` の apply・validate は
  無変更。

## Data and Control Flow

```
caller
 ├─ ProjectOp::SetSendGainTarget { send_id, gain_target }  (低層)
 ├─ RoutingEdit::SetSendGainTarget                          (routing_tx → 同 op)
 └─ synvelis.send.set-gain-target                            (gateway → 同 op)
        │
        ▼  op.validate（従来どおり: send 存在・新 target
           の well-known/project-time/二重 bind/owner/spec 検査）
        ▼  op.apply（scratch clone 上・不可分）
   previous = send.gain_target を捕捉
   send.gain_target = gain_target
   previous=Some(old) かつ old≠new かつ old が
   (Send(send_id), SendLevel) seal 保持
     → release_send_level_target: lane/assignment 除去 + target 削除
   gain_target=Some(t) → owner=Send(send_id), role=SendLevel を seal
        │
        ▼  post-apply oracle（validate_references）
   singleton (Send, SendLevel) は高々 1 件・
   gain_target の Send-owned 逆参照・lane/assignment の
   endpoint 実在を最終状態で検査 → 合格時のみ commit
```

variant candidate 経路では同じ apply が candidate
に対して走り、離脱した scope object が
`check_strip_fields_scope_removals` の supported
deletion 判定を通る。

## Non-Functional Constraints

- persisted 形状・schema version・snapshot 形式は
  不変 — 「rebind が受理される」方向のみ。
- atomicity: 1 apply = 1 CAS = 1 undo entry。
  release 途中の失敗経路は存在しない（retain/remove
  は非失敗）。
- headless で Some→Some・Some→None→Some・
  automation/assignment 閉包の削除・undo 復元を
  実測する。実 PCM/plugin/GUI/RT lane は本 slice
  の対象外（NotRun 記録）。
