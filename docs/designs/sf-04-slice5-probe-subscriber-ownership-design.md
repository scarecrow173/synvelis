---
id: "DESIGN-034VpkqegnYxvgZbOSsE3K"
type: "design"
status: "approved"
title: "SF-04 slice5 設計 — probe subscriber/window ownership（session 層 lease・capture owner）"
created: "2026-11-10"
updated: "2026-11-11"
owners: []
relations:
  source: []
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/sf-04-slice5-probe-subscriber-ownership.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/sf-04-slice5-probe-subscriber-ownership.md"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "docs/designs/sf-04-slice2-controlsession-probe-trace-design.md"
    - "docs/designs/sf-04-slice4-app-wiring-design.md"
  defers: []
  deferred-by: []
---

# SF-04 slice5 設計 — probe subscriber/window ownership（session 層 lease・capture owner）

## Context

slice 2 の `ProbeTraceSession` は `ProbeSubscriptionKey →
BTreeSet<ParameterTargetId>` の相関のみを持ち、subscriber
（Window/Inspector）識別子も lease も無い。`probe_remove`/
`probe_stop_capture`/`probe_set_mode` は key に無条件で効くため
「共有購読中の一方の Window close」を表現できない。port 側の
dedup は `ProbeSession::request` の key 冪等で済んでおり、
本 slice が足すのは **session 層の lease/refcount と capture
owner 追跡のみ** — port 契約（`ProbeTracePort`）・player・
project 層は変更しない。

## Scope

### In Scope

- `crates/synvelis-workspace/src/probe_trace.rs`: lease/capture
  owner state・additive API・`#[cfg(test)]` unit tests。
- `crates/synvelis-workspace/src/lib.rs`: export 追加のみ。
- `crates/synvelis-workspace/src/control.rs`: 変更しない
  （`probe_session` field は `ProbeTraceSession` のまま —
  state 追加は同 struct 内で完結）。

### Out of Scope

- `crates/synvelis-workspace/tests/`（別 Worker 占有）、
  `synvelis-app`/`synvelis-player`/`synvelis-project` 他 crate、
  `syn.probe.*` wire op・`ui/src/ipc.ts`、実 GUI gate、
  probe state の永続化。

## Components and Boundaries

### subscriber 識別子 — `ViewInstanceId` を採用

候補は ruviform-ui の `ViewInstanceId` と opaque
`ProbeSubscriberRef`（session mint u64）だった。**`ViewInstanceId`
を採用する**:

- SF-PROBE-06 の subscriber は定義上「複数 Inspector/Window」
  = この crate の論理 view であり、`ViewInstanceId` は move/
  detach/reattach を跨いで安定・consumer mint・window close
  （`ReleaseViews` → `ViewRegistry::unregister`）で死ぬ、まさに
  lease 寿命と一致する identity。opaque ref だと app 層に
  view→ref の対応表を新設する必要があり、得るものが無い。
- 先例: `control_edge.rs` の `ControlEdgeGesture.view`、
  `command.rs` の `origin_view` が既に `ViewInstanceId` を
  session 隣接 state に載せている。`ruviform-ui` は既存依存で
  新規依存なし。
- `ViewInstanceId` が再利用されない（mint のみ・一意）ため、
  close→再 open で新 id = 新 subscriber = 新規 lease となり
  stale lease の衝突が起きない。

opaque `ProbeSubscriberRef` は「view 以外の subscriber」
（headless job 等）が必要になった時点で再検討する — 現契約の
subscriber は view のみ。

### lease 保持者 — `ProbePrincipal`

```rust
/// lease / capture owner の principal — session 権限か論理
/// view subscriber のどちらか。
#[non_exhaustive]
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ProbePrincipal {
    /// Session 権限 — legacy `probe_request` の hold、
    /// session 権限 `probe_capture`、owner lease 喪失で
    /// 取り残された capture。
    Session,
    /// 論理 view（Window/Inspector）subscriber。
    View(ViewInstanceId),
}
```

`Session` variant が必要な理由: 既存 `probe_request` で作られた
key は「session が欲しい subscription」であり、後から view が
subscribe→release しても session 側の要求を道連れにしては
いけない。`probe_request` 成功時に `Session` lease を記録すれば
「最後の lease が外れたら remove」の規則が legacy key を
自然に保護する（view lease が外れても `Session` が残る）。

### `ProbeTraceSession` state 拡張

```rust
pub(crate) struct ProbeTraceSession {
    session_id: ProjectSessionId,                       // 既存
    targets: BTreeMap<ProbeSubscriptionKey,
                      BTreeSet<ParameterTargetId>>,     // 既存（相関）
    /// key → その subscription を保持する principal 群。
    /// 空集合の key は存在しない（最後の lease 解放で除去）。
    leases: BTreeMap<ProbeSubscriptionKey,
                     BTreeSet<ProbePrincipal>>,
    /// view → その view が保持する key 群（release_subscriber
    /// の逆引き。Session principal はここに現れない）。
    by_view: BTreeMap<ViewInstanceId,
                      BTreeSet<ProbeSubscriptionKey>>,
    /// key → 進行中 capture の owner（明示開始者）。
    /// stop/remove/最終 release で除去。
    capture_owners: BTreeMap<ProbeSubscriptionKey,
                            ProbePrincipal>,
}
```

**不変条件**（session が維持し、全 API がこれを前提にする）:

1. `leases` に key が存在する ⟺ session がその key の port
   subscription を live と見なす。`probe_remove`・最終 lease
   解放は port remove 成功時にかぎり lease を除去するため、
   「lease 有・port 側 Stopped」は session API 経路では
   発生しない（port を session 外から直接叩く行為は契約外）。
2. `leases` の key 集合は `targets`（相関）の key 集合の
   部分集合 — lease は常に相関を伴う（`probe_request`/
   `probe_subscribe` が port 成功時に両方登録）。逆向きは
   成り立たない: `probe_remove` 系は相関を**残す**
   （Stopped binding の revival のため — 既存契約）が lease
   は除去するため、remove 済み key は「相関有・lease 無」
   = revival 待ち状態になる。
3. `by_view` は `leases` の `View` 要素の正確な逆引き
   （双方向で同期更新）。空集合の entry は除去する
   （`leases`/`by_view` に空集合を残さない）。
4. `capture_owners` の key は必ず `leases` に存在する
   （capture は lease 保持 key に対してのみ開始可能）。

不変条件 2 の帰結: **`probe_subscribe` は
`leases.get(&key)` が空（または無し）のときだけ
`port.probe_request` を呼ぶ**。lease 有 = live hold あり =
port binding は live（不変条件 1）なので port 呼出は不要 —
これが「2 つ目の Window が subscribe しても Broker request が
重複しない」の session 層証拠になる。lease 無 + 相関有 =
remove 済み（`Stopped`）なので `port.probe_request` が
revival を担う（`ProbeSession::request` は `Stopped` を
再 install する）。新規 key も同じ呼出で install される。

### API（`impl ControlSession` in probe_trace.rs）

port は従来どおり per-call `&mut dyn ProbeTracePort`。

#### additive — subscriber lease

- `probe_subscribe(project, port, subscriber: ViewInstanceId,
  target, point, kind, mode) -> Result<ProbeSubscriptionKey,
  ProbeTraceError>`
  1. `project.control.targets` に `target` が無ければ
     `UnknownTarget`（port を呼ばない — `probe_request` と同じ
     fail-closed）。
  2. `key = { session_id, point, kind }` を構成。
  3. `leases` に key の hold が無ければ
     `port.probe_request(project, key, mode)` — `Ok` のみ先へ、
     `Err` は `ProbeTraceError::Port` で伝播し局所状態を
     汚さない。hold があれば port を呼ばない（dedup —
     `mode` はこの経路では適用されない。live binding の mode
     変更は `probe_set_mode` のみ）。
  4. `targets[key].insert(target)`、
     `leases[key].insert(View(subscriber))`、
     `by_view[subscriber].insert(key)` — 全て set insert で
     冪等（同 view・同 key・同 target の再 subscribe は
     no-op）。
  5. `Ok(key)`。

- `probe_unsubscribe(subscriber, port, key)
  -> Result<ProbeReleaseReport, ProbeTraceError>`
  1. `leases[key]` が `View(subscriber)` を含まなければ
     `NotSubscriber`（fail-closed — lease の無い解放は呼出側の
     誤り）。
  2. `leases[key]` が `View(subscriber)` のみを含む
     （= この解放で空になる）場合は `remove_key` を呼ぶ —
     `remove_key` は **port remove 成功後にのみ** 局所状態を
     commit するため、port 失敗時は lease が保持されたまま
     `Err` が伝播し retry 可能。成功時は `report.removed` に
     key を入れる。
  3. lease が残る場合は `leases[key]` から
     `View(subscriber)` を除去し `by_view[subscriber]` から
     key を除去（port 呼出なし — 他 principal の hold と
     subscription は継続。`by_view[subscriber]` が空に
     なれば entry を除去）。`capture_owners[key] ==
     View(subscriber)` なら `Session` へ退避する。
  4. `report.released` に key を入れて返す。

- `probe_release_subscriber(subscriber, port)
  -> Result<ProbeReleaseReport, ProbeTraceError>` — Window
  Close 経路。`by_view[subscriber]` の key 集合（clone）を
  `probe_unsubscribe` と同じ手順で全て解放する。処理済み key
  は `by_view`/`leases` から除去済みなので、port エラーで
  途中 return しても残りは retry 可能（冪等）。終了時に
  `by_view[subscriber]` の空 entry を除去する。lease を
  持たない view の呼出は空の `ProbeReleaseReport` を返す
  （Window Close の冪等性 — 未購読 view の close を
  エラーにしない）。

- `probe_release_all(port) -> Result<ProbeReleaseReport,
  ProbeTraceError>` — Session stop 経路。`leases` の全 key を
  `remove_key` で除去する（session 保持・view 保持を問わない）。
  port エラーは最初の失敗で伝播（それ以前の key は除去済み）。

#### 既存 API への内部変更（外部挙動不変）

- `probe_request`: port 呼出・相関登録は不変。成功時に
  `leases[key].insert(Session)` を追加するのみ。
- `probe_set_mode`: 変更なし。
- `probe_capture`: port 呼出は不変。成功時に
  `capture_owners[key] = Session` を記録。
- `probe_stop_capture`: port 呼出は不変。成功時に
  `capture_owners.remove(&key)`。
- `probe_remove`: port 呼出 + `Missing` 記録は不変。成功時に
  `leases.remove(&key)`（`by_view` 逆引きも除去）と
  `capture_owners.remove(&key)` を追加 — session 権限の明示
  remove は全 lease を revoke する。
- `probe_drain`/reads: 変更なし。

#### additive — capture owner

- `probe_capture_for(subscriber, port, key, max_frames)
  -> Result<(), ProbeTraceError>`
  1. `leases[key]` が `View(subscriber)` を含まなければ
     `NotSubscriber`。
  2. `port.probe_capture(key, max_frames)` — `Err` 伝播、
     owner は記録しない。
  3. `Ok` で `capture_owners[key] = View(subscriber)`。
     進行中 capture があれば port が `AlreadyCapturing` で
     拒否するため owner の上書きは起きない。

- `probe_abort_capture(subscriber, port, key)
  -> Result<(), ProbeTraceError>` — owner 限定の明示 abort。
  1. `leases[key]` が `View(subscriber)` を含まなければ
     `NotSubscriber`。
  2. `capture_owners[key] != View(subscriber)` なら
     `NotCaptureOwner`（owner=`Session`・他 view・capture
     無しを区別しない — いずれも「この subscriber の
     capture ではない」）。
  3. `port.probe_stop_capture(key)` — `Err` 伝播。
  4. `Ok` で `capture_owners.remove(&key)`。

- `probe_capture_owner(key) -> Option<ProbePrincipal>` — read。
  記録上の owner を返す。capture が budget 切れ・generation
  cut 等で port 側終了しても session は通知を持たないため、
  owner エントリは明示 stop/remove/release まで残り得る
  （帰属記録であり job 状態の証明ではない — doc 明記）。

#### additive — reads

- `probe_lease_holders(key) -> Vec<ProbePrincipal>` — lease
  保持者の read-only view。
- `probe_subscriber_keys(subscriber) -> Vec<ProbeSubscriptionKey>`
  — view が保持する key の read-only view（release 前の
  preview/診断用）。

### `remove_key` helper（内部）

`probe_remove`・unsubscribe 最終解放・`probe_release_all` が
共有する除去手順:

1. `port.probe_remove(key)?` — 失敗時は局所状態を一切変更
   しない（lease/owner/相関を保持 — subscription はまだ
   生きている）。
2. `Ok` で相関 `targets[key]` の各 target に
   `port.record_probe(target, ProbeObservation::Missing)`
   （既存 `probe_remove` と同じ同期 Missing — RT `Removed`
   evidence が後で届いても冪等）。
3. `leases.remove(&key)` の**前に**除去対象の lease 集合を
   clone し、その `View` member 全員について `by_view`
   から key を除去（空集合 entry も除去）— `probe_remove`
   や `probe_release_all` は複数 holder を一括 revoke する
   ため、unsubscribe 最終解放（単一 holder）と同じ手順で
   両方を正しく掃除できる。`capture_owners.remove(&key)`。
   `targets` 相関は残す（revival 契約）。

### 新規エラー variant

```rust
pub enum ProbeTraceError {
    UnknownTarget(ParameterTargetId),   // 既存
    /// `subscriber` は `key` の lease を保持していない —
    /// unsubscribe/abort/capture_for は fail-closed。
    NotSubscriber { key: ProbeSubscriptionKey,
                    subscriber: ViewInstanceId },
    /// `subscriber` は lease を持つが `key` の進行中
    /// capture の owner ではない。
    NotCaptureOwner { key: ProbeSubscriptionKey,
                      subscriber: ViewInstanceId },
    Port(ProbePortError),                // 既存
}
```

`ProbeReleaseReport`:

```rust
/// 1 回の release pass の会計 — どの key が lease を失い、
/// どの subscription が port remove されたか。drop を silent
/// にしない。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ProbeReleaseReport {
    /// lease が外れた key（BTreeMap 走査順）。
    pub released: Vec<ProbeSubscriptionKey>,
    /// 最後の lease が外れ port remove された key。
    pub removed: Vec<ProbeSubscriptionKey>,
}
```

### export

`lib.rs`: `pub use probe_trace::{ProbeDrainReport,
ProbePrincipal, ProbeReleaseReport, ProbeTraceError};` に
`ProbePrincipal`/`ProbeReleaseReport` を追加。

## Data and Control Flow

```
Window/Inspector open ──► probe_subscribe(project, port, view,
                          target, point, kind, mode)
   │ leases[key] 無 → port.probe_request（install/revive）
   │ leases[key] 有 → port 不呼出（dedup）
   ▼ leases[key]+=View(view)、by_view[view]+=key、
     targets[key]+=target

Window Close ──► probe_release_subscriber(view, port)
   │ by_view[view] の各 key: leases[key]-=View(view)
   │   ├─ lease 残 → capture owner が view なら Session 退避
   │   └─ lease 空 → remove_key: port.probe_remove
   │       + targets Missing + lease/owner 掃除
   ▼ ProbeReleaseReport { released, removed }

Capture 開始 ──► probe_capture_for(view, port, key, max_frames)
   │ lease 必須 → port.probe_capture → capture_owners[key]=View
Abort ──► probe_abort_capture(view, port, key): owner==view のみ
   │ port.probe_stop_capture → capture_owners 除去
Session 権限 ──► probe_capture / probe_stop_capture /
   probe_remove / probe_set_mode: 無条件（明示 session 操作）
   │ それぞれ owner 記録/消去・lease revoke を内側で同期
Session stop ──► probe_release_all(port): leases 全 key を
   remove_key
```

Window Move は `ViewInstanceId` が move/detach/reattach を跨いで
不変のため session 層に遷移・API が存在しない（lease は view
id に紐づき window 帰属を参照しない — 設計上の no-op であり
unit test は「別 view id の release が当該 key に影響しない」
ことで間接的に固定する）。

## Non-Functional Constraints

- RT 経路不変・port 契約不変: `ProbeTracePort`・
  `ProbeSession`・`DispatchTraceAdapter` に触れない。全処理は
  非 RT orchestration。
- allocation は bounded: lease map ≤ subscription 数、
  `by_view` ≤ view 数×key、`ProbeReleaseReport` ≤ key 数 —
  すべて非 RT 経路。
- 既存 `probe_*` の port 呼出順序・戻り値・`ProbeDrainReport`
  集計は不変（`sf04_s2_probe_trace.rs` 等の既存テストが
  無変更で通ることが外部挙動不変の証拠）。
- 公開 enum は `#[non_exhaustive]`、`unsafe` なし、lint は
  workspace 共通。

## Risks and Trade-offs

- **「lease 有 ⟹ port live」の不変条件は session API 経路に
  限定**: port を per-call で外部から直接 `probe_remove`
  されると lease が残ったまま binding が死ぬ。その場合
  `probe_subscribe` は port を呼ばず join するが、観測は
  流れない — これは「session の drain/lifecycle API が唯一の
  session 経路」という slice 2 からの契約の範囲内であり、
  port 直叩きは composition root の契約外行為として扱う
  （design doc に明記）。
- **`mode` を既存 hold への subscribe で無視**: port の
  `ProbeSession::request` 冪等と同じ意味論に揃えた。
  silent ignore に見えるが、共有 binding の mode は
  per-subscriber に存在しない契約であり、変更経路は
  `probe_set_mode` のみ — doc comment と spec で明示。
  `ModeConflict` error 案は session 側に port と乖離し得る
  mode 状態を複製するため棄却。
- **owner lease 喪失時の `Session` 退避**: 代替は (a) 死んだ
  view を owner に残す（abort 判定が不能になる）、(b) capture
  を abort する（「Close Windowは進行JobをStopしない」違反）
  — 退避が唯一契約を満たす。帰属は「最後に明示開始した者」
  の記録であり job 生死は明示操作のみが決める。
- **相関は key 級で per-subscriber target を持たない**:
  view A が足した target は key 生存中 view B の drain でも
  更新され続ける。bounded で honest（観測は target の
  trace に行くだけ）であり、per-subscriber target lease は
  YAGNI — Deferred へ。
- **`probe_capture_owner` は job 状態を証明しない**: port 側
  で暗黙終了（budget/generation cut）しても session は知ら
  ず、owner 記録は明示 stop/remove/release まで残る。
  「最後に明示開始した者」の帰属記録として doc 明記 —
  binding ledger（`probe_bindings`）が真の状態を持つ。

## Deferred Design Concerns

- `syn.probe.*` wire op への subscriber 識別子と view close
  hook の app 配線 | `synvelis-app` は SV#117 占有中 |
  後続 Issue（#108 系）| wire 面の要求適合は app 実装時に検証。
- 実 2-window GUI での観測 | #108/#114 gate | gate 通過後 |
  headless では実 UI 寿命との整合を証明できない。
- opaque `ProbeSubscriberRef` / view 以外の subscriber 種別 |
  現契約の subscriber は view のみ | 非 view subscriber が
  必要になった時点 | 識別子型の変更は additive で可能。
- per-subscriber mode・per-subscriber target lease |
  共有 binding/相関の契約に存在しない | UI が要求した時点 |
  port/契約拡張が必要。

## Verification Notes

- `probe_trace.rs` `#[cfg(test)]` unit tests（fake
  `ProbeTracePort` — trait のため容易、先例は project/
  library/recording/gateway の src 内 test）:
  - 2 view が同一 `(point,kind)` に subscribe →
    `port.probe_request` は 1 回のみ（dedup）、両 view の
    lease・target 相関が残り drain が fan-out する。
  - `probe_release_subscriber` で片方 close → port remove
    されず他 view の lease・capture・観測が継続、最後の
    release で `probe_remove` 相当（`Missing` 記録）。
  - Window Move 相当: view id は不変なので API 呼出なし —
    別 view の release が key に影響しないことで固定。
  - `probe_release_all` で全 subscription が port remove
    される（session stop）。
  - `probe_capture_for`/`probe_abort_capture` の owner 限定:
    非 owner・非 lease の abort/capture は
    `NotCaptureOwner`/`NotSubscriber`、owner の abort は
    `port.probe_stop_capture` へ届く。owner lease 喪失で
    `Session` 退避・capture 継続。
  - legacy `probe_request`/`probe_remove` が lease を
    `Session` で保護し、view release で道連れにならない。
- `mise run fmt-check` / `check` / `lint` / `test` /
  `test-rt` / `test-msrv`。
