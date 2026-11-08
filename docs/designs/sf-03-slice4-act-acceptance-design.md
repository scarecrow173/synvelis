---
id: "DESIGN-6tbKfWyun38H7ERAnefMsA"
type: "design"
status: "approved"
title: "SF-03 slice4 設計 — NoRebuild Equivalence・世代連動 processor control 払い出し"
created: "2026-11-09"
updated: "2026-11-09"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/106"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/sf-03-slice4-act-acceptance-norebuild.md"
  implemented-by: []
  depends-on:
    - "docs/specs/sf-03-slice4-act-acceptance-norebuild.md"
    - "docs/designs/sf-03-slice3-plugin-host-connect-design.md"
    - "docs/designs/sf-03-slice2-fault-reconcile-design.md"
    - "docs/designs/sf-03-slice1-lowering-sourcemap-activation-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/adr/sf-03-semantic-lowering-source-map-activation-boundary.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/sf-03-slice4-act-acceptance-norebuild.md"
  derived-by:
    - "docs/plans/sf-03-slice4-act-acceptance.md"
    - "docs/tasks/sf-03-slice4-norebuild-equivalence.md"
    - "docs/tasks/sf-03-slice4-processor-control-vending.md"
    - "docs/tasks/sf-03-slice4-acceptance-tests.md"
    - "docs/test-specs/sf-03-slice4-act-acceptance.md"
  verifies: []
  verified-by:
    - "docs/test-specs/sf-03-slice4-act-acceptance.md"
  references:
    - "docs/specs/sf-03-slice1-semantic-lowering-source-map-activation.md"
    - "docs/specs/sf-03-slice2-fault-taxonomy-lastgood-reconcile.md"
    - "docs/specs/sf-03-slice3-plugin-host-connect.md"
  defers: []
  deferred-by: []
---

# SF-03 slice4 設計 — NoRebuild Equivalence・世代連動 processor control 払い出し

## Context

slice 1〜3 で `RoutingEngine` は semantic `RoutingModel` の lowering・
compile・publish・activation receipt・last-good・`ProcessorHost`
boundary・generation-scoped lease teardown を確立した。残る SF-ACT
系受入のうち player/project 境界で扱えるのは次の 2 つの未実装経路
である。

1. **NoRebuild（SF-ACT-09）**: `RoutingBuildKey::semantic_eq`
   （`project_revision` を除く全 field 同値）は slice 1 で契約化
   済みだが、それを使う判定経路が存在しない。現行 `prepare` は
   常に lowering→host connect→compile を実行し、Project 名変更の
   ような graph 非関連 revision 進行でも毎回全 compile を行う。
   Issue #106 は「Graph Fingerprint が同値なら NoRebuild。ただし
   新 Revision が実 RT Active になったという偽 Receipt を出さず
   `ExecutionEquivalence` を別証拠化」と要求する。
2. **世代連動 dispatch（SF-ACT-06）**: slice 3 の lease teardown
   closure は `attach` の返す `PluginNodeControl` を捕捉して drop
   するだけで、制御半体を外部へ払い出す経路がない。Issue #106 は
   「Active Generation で存在/同一性検証された Parameter Binding
   以外へ送らない」「stale ParamDispatch/Event QueueFull/wrong-
   plugin 0」を要求し、slice 3 design の Deferred Design Concerns
   は「`ProcessorLease` の retained 経路を公開」と明示していた。

## Approaches（検討した設計）

### NoRebuild の判定位置と戻り値

| 案 | 内容 | 判定 |
| --- | --- | --- |
| A. `prepare` の戻り値を `RoutingPrepare` enum 化 | `Built(PreparedRouting)` / `Equivalent(ExecutionEquivalence)` を返す | 採用。equivalence は「build の結果」であり、別メソッドに分けると呼出側が 2 経路を使い分ける規約を自前で抱える。enum は『compile したか否か』を型で強制する |
| B. `prepare` は据え置き、`prepare_or_equivalent` を追加 | 既存 `prepare` = 常に build | 不採用。`prepare` が「常に rebuild」を意味する第 2 の入口になり、NoRebuild を使うべき経路が bypass され得る。呼出側は現状 test のみで、契約を 1 本に絞る方が誠実 |
| C. equivalence を `publish` 側で処理 | prepare は常に build し、publish が semantic_eq で plan を差し替えない | 不採用。compile/host connect のコストを削減できず、Issue の「無駄な Compile 防止」を満たさない |

**採用: A**。`prepare` が `Result<RoutingPrepare, RoutingFault>`
を返す。`Equivalent` は publish 対象を持たず receipt も更新
しない — live plan の `Active`/`Published` 記述をそのまま維持
する（新 revision が active になった偽証拠を出さない、
という Issue 要求と一致）。

### equivalence の成立条件

`prepare` 内で `checked_key`（expected revision・runtime
instance/epoch・budget の検査）通過後、generation 予約・
lowering の前に判定する。

```text
equivalent ⟺ source_map が存在する
          ∧ live_key.semantic_eq(&new_key)
          ∧ live_key.project_revision != new_key.project_revision
```

- `semantic_eq` は `project_revision` 以外の全 field
  （project id・routing fingerprint・capability/engine/
  source-format generation・lowering policy version・build
  mode・device clock epoch・runtime instance）を比較する
  — 「device config-only change で必要 Compile」（engine/
  source-format generation 差）と「Unknown/Plugin opaque
  state changed なら保守的に Rebuild」（capability generation
  差）は自動的に build 側へ落ちる。
- **同一 revision の再 prepare は常に build** とする。
  `live_key.project_revision == new_key.project_revision` の
  場合は equivalence にしない — 同一 revision の再 build は
  runtime 状態を明示的に再構築する経路として残す
  （`sf03_l2_09` の連続 publish・queue-full 再現 test が依存
  する「同一 revisionで新しい generation を得る」契約を維持）。
- 判定は `source_map`（最後に publish された plan の key）に
  対して行う。pending（Published 未 Active）の plan も
  「いずれ active になる live plan」として equivalence の
  相手にできる — receipt はその plan の実状態をそのまま
  示し続ける。

### `reconcile` の revision-drift 扱い

`reconcile` は現行で `key.project_revision != revision` を無条件に
`ProjectRevision` mismatch（→ `StaleProjectSnapshot`）とする。
NoRebuild 経路では「committed revision が進んだが semantic は
同値」が正常状態になるため、このままでは rename 後の reconcile
が false positive を出す。

`reconcile` 内で live state から key を再構成し
（`project_id`/`project_revision=引数`/`routing_fingerprint`/
`capability_generation`/引数の engine・source-format
generation/`LOWERING_POLICY_VERSION`/published key の
`build_mode`/`self.epoch`/`self.instance`）、
`published_key.semantic_eq(&live_key)` が真なら
`ProjectRevision` mismatch を push しない。

- semantic が実際に drift した場合は `semantic_eq` が偽となり
  従来どおり `ProjectRevision` mismatch が報告される —
  `StaleProjectSnapshot` の優先順位位置は不変。
- `build_mode` は project 由来でないため live key には
  published key の値を使う（「この plan が live project に
  対してなお正しいか」の判定であり、mode の意図変更は
  次回 prepare の key 差として現れる）。
- report の `mismatches`/`fault` の構造は変更しない —
  equivalence は「mismatch がない」ことで表現され、
  `RoutingReconcileReport` に新 field は追加しない。

### 世代連動 control 払い出し

`ProcessorConnection` に `control` を追加する。

```rust
pub struct ProcessorConnection {
    pub processor: Box<dyn GraphProcessor>,
    pub lease: ProcessorLease,
    /// Non-RT control half retained for the generation's lifetime —
    /// `Some` only for real plugin hosts (`attach` の control)。
    /// `None` for `StaticProcessorHost` and other non-plugin hosts.
    pub control: Option<PluginNodeControl>,
}
```

- `PluginHostProcessors::connect` は `attach` の返す
  `attached.control` を lease closure から `ProcessorConnection`
  へ移す。lease の teardown closure は host lifecycle のみを
  実行し、`drop(control)` の行を除去する。control の寿命は
  generation ledger（後述）が所有し、lease と同じタイミング
  で drop される — slice 3 の「teardown 後に control drop」
  の順序は `RetainedConnection` の field 宣言順
  （`lease` → `control`）で維持する。
- `StaticProcessorHost::connect` は `control: None`。
- `ProcessorLease` 自体の契約は変更しない — teardown closure
  は opaque のまま。

engine 側の保持構造:

```rust
/// One host connection's non-RT halves retained for its
/// generation: teardown lease + optional plugin control + the
/// semantic/runtime ids needed to vend the control.
struct RetainedConnection {
    processor: ProcessorId,
    node: NodeId,
    lease: ProcessorLease,
    control: Option<PluginNodeControl>,
}
```

- `StagedBuild.leases: Vec<ProcessorLease>` →
  `Vec<RetainedConnection>`、`PreparedRouting.leases` →
  `connections: Vec<RetainedConnection>`、
  `RoutingEngine.connections: BTreeMap<u64, Vec<ProcessorLease>>`
  → `BTreeMap<u64, Vec<RetainedConnection>>`。
  publish/observe_activation/seek/Drop の既存の保持・解放規則は
  そのまま `RetainedConnection` 全体に適用される。
- `stage_build` の connect 成功 arm で
  `RetainedConnection { processor: semantic.id, node: *node,
  lease, control }` を構築する。

払い出し API:

```rust
/// One-shot access to a published generation's chain-processor
/// control channel.
pub struct ProcessorControl {
    /// Generation-local graph node the control drives.
    pub node: NodeId,
    /// The node's non-RT control handle (`PluginNodeControl`).
    pub control: PluginNodeControl,
    /// The attach-time `AttachConfig::frame_base` this node was
    /// bound with (`TimeBase::sample_rate(engine.sample_rate)`) —
    /// `PluginNodeControl` exposes no accessor for it and
    /// `ParamDispatch::new` requires the exact value, so the engine
    /// vends it rather than leaving callers to re-derive it.
    pub frame_base: TimeBase,
}

impl RoutingEngine {
    /// Takes the non-RT control handle for `processor` of
    /// `generation` — `None` for unknown generation/processor,
    /// non-plugin (`StaticProcessorHost`) connections, or an
    /// already-taken handle.
    pub fn take_processor_control(
        &mut self,
        generation: u64,
        processor: ProcessorId,
    ) -> Option<ProcessorControl>;
}
```

- `frame_base` は engine の `sample_rate` から再構成する
  （`PluginHostProcessors::connect` が `AttachConfig.frame_base`
  に使ったのと同じ `TimeBase::sample_rate(port_format.
  sample_rate)` で、`port_format` は `engine_format(engine)` 由来
  のため `engine.sample_rate` に一致する）。`sample_rate == 0` は
  `engine_format` が既に panic する不変条件違反であり、
  `take_processor_control` は `Option` を返すため `.ok()?` で
  `None` に倒す（公開済み接続が存在し得ない状態）。

- one-shot（`Option::take`）: `ParamDispatch` は control を所有
  するため、engine が保持し続ける必要はない。2 回目の呼出しは
  `None`。
- 払い出し対象は `self.connections` に存在する世代 =
  publish 成功済みの世代のみ。`PreparedRouting` 内の接続は
  publish 前であり払い出せない（plan が live でないため —
  正しい境界）。
- 払い出し後に世代が退役（pending 置換・Active 観測・engine
  drop）しても、caller が所有する control は残る。その channel
  の consumer は当該世代の `PluginNode` であり、node が retire
  されれば push は bounded channel の `QueueFull`（型付き拒否）
  か、consumer 死後も ring buffer が受理する有限個の message が
  配送先なく滞留するだけ — **他世代の plugin へ届く経路は
  channel が instance ごとに独立しているため存在しない**
  （wrong-plugin 0 は接続構造で保証）。rtrb channel は consumer
  切断を producer に通知しないため、滞留 message の silent
  acceptance は upstream channel 契約の範囲であり、配送・
  誤配送にはならない。
- `ParamDispatch` への組み立て（bind/dispatch/observe）は
  caller の責務 — engine は control の払い出しのみ。
  `ParamDispatch::new(control.control, control.node,
  control.frame_base)` がそのまま使える形で払い出す
  （`frame_base` は attach 時と同一値を engine が同梱する）。

### SF-ACT-05/07/08 の扱い

- SF-ACT-05 の headless 部分は既存契約の固定が中心 —
  `sf03_l4.rs` で rapid publish の receipt 系列・`Superseded`
  publish 拒否・`PublishQueueFull`・stale counter→`EventStale`
  reconcile・`DeviceEpochChanged`・fresh-retry を検証する。
  実機部分（RT latency/underrun/device restart）は後続送り。
- SF-ACT-07 は `skipped_fault` の両 mode fail-closed を維持し、
  preview 許容 policy の分岐は「明示 BuildMode/Policy が未規定」
  として後続送り（設計判断として記録）。MissingSource 系の
  deterministic failure は既存 test が担保するため新規 fault
  経路は追加しない。
- SF-ACT-08 は ruviform RT（`test-rt` lane）・synvelis-render・
  AudioLens の責務 — player/project での実装なし。`reconcile`
  の retire/stale counter surface を受入 evidence として記録。

## Components and Boundaries

### `crates/synvelis-project/src/routing_build.rs`

```rust
/// Evidence that a newly configured revision needs no rebuild:
/// the live published plan's semantic build key already covers it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExecutionEquivalence {
    /// The revision the caller just configured (not physically
    /// built into a new plan).
    pub configured_project_revision: ProjectRevision,
    /// The revision the live plan was actually built from.
    pub active_built_from_revision: ProjectRevision,
    /// The live plan's build key — every field except
    /// `project_revision` matched the new request's key.
    pub matching_semantic_build_key: RoutingBuildKey,
    /// Generation of the live published plan.
    pub plan_generation: u64,
}
```

- `RoutingBuildKey`/`semantic_eq`/`RoutingFault` 等の既存契約は
  変更しない。`lib.rs` の `routing_build` re-export に
  `ExecutionEquivalence` を追加する。

### `crates/synvelis-player/src/host.rs`

- `ProcessorConnection` に `pub control: Option<PluginNodeControl>`
  を追加（`PluginNodeControl` は既存の bridge 依存から import）。
- `PluginHostProcessors::connect`: `attached.control` を
  `ProcessorConnection.control` へ移し、lease closure から
  `drop(control)` を除去。
- `StaticProcessorHost::connect`: `control: None`。
- `ProcessorControl { node, control }` struct を追加。

### `crates/synvelis-player/src/routing_engine.rs`

- `RoutingPrepare` enum（`Built`/`Equivalent`）を `pub` で定義。
  `into_built()`/`expect_built()` helper を提供。
- `prepare` の戻り値を `Result<RoutingPrepare, RoutingFault>` に
  変更し、`checked_key` 後に equivalence 判定を挿入。
- `reconcile` に live-key 再構成と `semantic_eq` による
  `ProjectRevision` mismatch 抑制を追加。
- `RetainedConnection` struct 導入、`StagedBuild`/`PreparedRouting`/
  `connections` map の値型を更新。
- `take_processor_control` 追加。

### `crates/synvelis-player/src/lib.rs`

- `RoutingPrepare`・`ProcessorControl`・`ExecutionEquivalence`
  （project re-export 経由）の export を追加。

### 既存 test の追従

- `sf03_l1.rs`/`sf03_l2.rs`/`sf03_l3.rs` の `prepare` 呼出しは
  戻り値型変更に追従（error 経路は `Result<_, RoutingFault>` の
  ままなので `.err().unwrap()` は無変更。成功経路は
  `expect_built()` 等で `PreparedRouting` を取り出す）。
- `sf03_l3.rs` の `RecordingHost` が構築する
  `ProcessorConnection` に `control: None` を追加。
- equivalence が発火し得る既存 test（publish 済み plan と
  semantic 同値の新 revision を prepare する経路）は個別に
  確認し、意図が「再 build」なら同一 revision 経路か semantic
  差分を入れて契約を維持する。

## Data and Control Flow

```text
prepare(request, project, revision, deps)
  ├─ checked_key（revision/epoch/budget 検査・fault 記録）
  ├─ equivalence 判定:
  │    source_map.build_key.semantic_eq(&key)
  │    ∧ revision 差あり → RoutingPrepare::Equivalent(evidence)
  │      （generation 予約・lowering・connect・compile なし、
  │        receipt 不変）
  └─ Built: 既存 pipeline（lower_semantic → stage_build
       （host.connect → RetainedConnection{lease,control}）
       → offline gate → project_rt.prepare → compile）
            → PreparedRouting { connections, ... }

publish(prepared, project, revision)
  └─ 既存検査 → graph.publish 成功
       → connections[gen] = prepared.connections
         （{lease, control, ids} を世代 ledger へ）

caller:
  engine.take_processor_control(gen, processor_id)
    → Option<ProcessorControl{node, PluginNodeControl}>
  → ParamDispatch::new(control, node, control.frame_base())
  → bind/dispatch → node scheduler → plugin instance
    （channel は instance 専用 → wrong-plugin 0）
```

## Non-Functional Constraints

- **RT 安全性**: equivalence 判定・control 払い出し・
  `RetainedConnection` の drop は全て非 RT。`PluginNodeControl`
  の push API（`try_push`）は upstream の non-blocking 契約で
  あり、engine は RT path に新規処理を追加しない。
- **決定性**: equivalence 判定は `semantic_eq` の field 比較
  のみで副作用を持たない。`take_processor_control` は
  `BTreeMap`/`Vec` の走査順で決定的。
- **Send/スレッド**: `PluginNodeControl` の Send 性は upstream
  契約に従う。engine/`PreparedRouting` が `!Send` である
  slice3 の前提は変更しない。
- **エラー**: 新規 fault variant なし。equivalence は fault で
  はなく正常な build 不要の証拠。control 払い出しの失敗は
  `Option::None` で表現し、新しい error 型を作らない。

## Risks and Trade-offs

| リスク | 対応 |
| --- | --- |
| 同一 revision 再 prepare まで equivalence にすると「明示的 rebuild」経路が消える | `project_revision` 差を必須条件にして同一 revision は常に build（l2_09 の連続 publish 契約を維持） |
| rename 後の `reconcile` が `StaleProjectSnapshot` を誤報する | live-key `semantic_eq` で `ProjectRevision` mismatch を抑制。semantic drift 時は従来どおり報告 |
| 払い出した control が世代退役後も caller に残り stale message を受理し得る | channel は instance 専用で誤配送なし。node 死後の push は `QueueFull` または配送先なし滞留 — upstream channel 契約の範囲。誤配送（wrong-plugin）にはならないことを test で固定 |
| `ProcessorConnection` への field 追加で外部 host 実装が壊れる | `ProcessorConnection` を構築するのは workspace 内の `StaticProcessorHost`/`PluginHostProcessors`/test stub のみ — 追従は機械的 |
| `prepare` 戻り値型変更で既存 test が広範に壊れる | error 経路は不変。成功経路のみ `expect_built()` 系で追従。件数は限定的（l1/l2/l3 合わせて数十箇所、機械的） |
| `PluginNodeControl` を engine が保持することで drop 順序が変わる | `RetainedConnection` の field 順で `lease`（teardown）→ `control`（channel close）の順を維持。control drop 自体は副作用を持たない channel end の解放 |

## Deferred Design Concerns

- automation/control plan → `TimedParamEvent` lowering の engine
  配線・Fast Control の active-generation gate（Blocked/有界
  Queue replay 再検証）| app/session 層の責務・GUI fast-control
  契約が必要 | 後続 slice（SF-ACT-06 残件）
- preview 許容 policy（Partial Preview vs Export fail-closed の
  mode 分岐）| 明示 policy 未規定 | 後続 slice で policy 決定後
- 実機検証（RT latency/underrun/device restart/実 OS plugin）
  | 実環境必須 | 後続 slice
- `capability_revision`/`version` の port 単位照合 | semantic
  identity pin は Project 側責務・照合強化は別契約 | 後続 slice

## Verification Notes

- [ ] `cargo check`/`clippy` で `routing_build.rs`・`host.rs`・
      `routing_engine.rs` が lint clean
- [ ] `sf03_l1.rs`/`sf03_l2.rs`/`sf03_l3.rs` が新署名へ追従し
      green（既存契約の回帰なし）
- [ ] `sf03_l4.rs` で equivalence・control 払い出し・SF-ACT-05
      headless 固定を検証
