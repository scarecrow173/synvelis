---
id: "DESIGN-034UfwVzxaw6WwUYS3nMDN"
type: "design"
status: "approved"
title: "SF-03 slice3 設計 — processor host 接続 boundary・世代連動 teardown"
created: "2026-11-08"
updated: "2026-11-08"
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
    - "docs/specs/sf-03-slice3-plugin-host-connect.md"
  implemented-by: []
  depends-on:
    - "docs/specs/sf-03-slice3-plugin-host-connect.md"
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
    - "docs/specs/sf-03-slice3-plugin-host-connect.md"
  derived-by:
    - "docs/plans/sf-03-slice3-plugin-host-teardown.md"
  verifies: []
  verified-by:
    - "docs/test-specs/sf-03-slice3-plugin-host-connect.md"
  references:
    - "docs/specs/sf-03-slice1-semantic-lowering-source-map-activation.md"
    - "docs/specs/sf-03-slice2-fault-taxonomy-lastgood-reconcile.md"
  defers: []
  deferred-by: []
---

# SF-03 slice3 設計 — processor host 接続 boundary・世代連動 teardown

## Context

slice 1/2 で `RoutingEngine` は semantic `RoutingModel` を typed
`GraphSpec` へ lowering し、compile/publish/activation の証拠と
last-good を確立した。しかし chain processor node の実体供給は
`RoutingDeps.processors`（`BTreeMap<ProcessorId, Box<dyn
GraphProcessor>>`）への直接注入のみで、plugin host の
instantiate/activate/endpoint attach ライフサイクルは synvelis 側に
存在しなかった（slice 1 spec の範囲外欄「実 plugin instance の
host 接続(chain processor は注入 boundary)」）。

本 slice はその注入 boundary を「呼び出し側が `Box<dyn
GraphProcessor>` を用意する」形から「semantic `Processor` を渡すと
host が接続済み RT leg + teardown lease を返す」形へ置き換え、
`ruviform-plugin-host` の実 lifecycle（`Instantiate` →
`Initialize` → `Activate` → `StartProcessing` → `attach` →
`PluginNode`）を headless 契約として接続する。

調査済みの上流事実（ruviform `863a7a9`）:

- `attach(&mut PluginHost, PluginHandle, &AttachConfig)` は
  declared bus table の validate → host bus report 照合
  （`BusReport::Reported` のみ。`Unavailable` は unverified で
  失敗にしない）→ `prepare_endpoint` → `PluginNode` +
  `PluginNodeControl` + `ports` を返す。bus/port 対応は
  `NodeIo::input(i)`/`output(i)` = i 番目の同方向 port で、
  declared bus の同方向順がそのまま port index になる。
- `PluginNode: GraphProcessor` で `Send`。endpoint は detach 済みで
  `PluginHost`/生 plugin object/child handle を持たない。
  `Drop` は CAS detach のみ。endpoint 退役は plan collector の
  非 RT 責務。
- `HostError` は `UnknownPluginId`/`InvalidHandle`/`Lifecycle`/
  `Prepare`/`PluginFailed`/`Unsupported`/`Failed`/`Ipc`/
  `DuplicatePluginId`/`InstanceLimit`/`EndpointUnavailable`/
  `InvalidValue`/`SpawnFailed` を区別する。
- `ErasedPlugin` は既定で `!Send`（`erase_sendable` が opt-in）。
  `PluginHost` は `!Send` であり、`Arc<Mutex<PluginHost>>` は
  `Send` にならない → lease teardown は `Rc<RefCell<…>>` 共有で
  実現し、boundary/engine は非 RT・単一スレッド前提とする。

## Approaches（検討した接続形態）

| 案 | 内容 | 判定 |
| --- | --- | --- |
| A. `RoutingDeps` に `&mut dyn ProcessorHost` を毎回注入 | prepare ごとに host を借用 | lease/接続が prepare 呼出を超えて生存するため、publish/activation 時点で host へ到達する経路が別途必要。`PreparedRouting` 破棄時の teardown も自前で抱える必要があり、境界が分散する |
| B. `RoutingEngine` が `Box<dyn ProcessorHost>` を所有 | `set_processor_host` で一度注入 | host lifetime = engine lifetime で素直。lease は `Rc<RefCell<PluginHost>>` を内包する自己完結 teardown とし、drop 箇所（PreparedRouting 破棄・publish 失敗・世代置換・Active 観測・engine 破棄）を engine が統制できる。`RoutingDeps` は per-build 入力に専念できる |
| C. `deps.processors` map を維持し host adapter が map を満たす | 呼び出し側が adapter 経由で instance を map に詰める | 接続タイミングが prepare 外に出て generation 連動・失敗 fault・lease 管理を engine が制御できない。slice 1 契約の「compile factory が instance を要求」を弱める |

**採用: B**。host は engine の activation lifecycle の一部であり、
所有権を engine に置くのが責務境界として正しい。lease の
drop-teardown により `PreparedRouting` の早期破棄でも接続が
解放され、engine は「どの世代の lease をいつ捨てるか」だけを
管理すればよい。

## Components and Boundaries

### 新規 module `crates/synvelis-player/src/host.rs`

```rust
/// connect への入力 — engine が semantic entity から組み立てる。
pub struct ProcessorConnectRequest {
    /// semantic processor entity（binding + port_ids）。
    pub processor: Processor,
    /// `processor.port_ids` 順に解決した semantic port 実体。
    pub ports: Vec<SemanticPort>,
    /// この接続が埋める lowered graph node。
    pub node: NodeId,
    /// build generation（接続の世代タグ）。
    pub generation: u64,
    /// build mode（Offline/Realtime）。
    pub mode: BuildMode,
    /// engine audio format（rate/layout/format）。
    pub port_format: AudioFormat,
    /// process block の frame 上限（= engine.block_frames）。
    pub max_frames: u32,
}

/// connect の成功結果 — RT leg と非 RT lease の対。
pub struct ProcessorConnection {
    /// compile factory が install する RT processor。
    pub processor: Box<dyn GraphProcessor>,
    /// 接続の非 RT 半分を内包する teardown token。
    pub lease: ProcessorLease,
}

/// 接続 1 件の teardown token。drop で host 側 teardown
/// （stop-processing → deactivate → destroy、best-effort）を
/// 自己完結で実行する。中身は opaque — engine は保持/破棄のみ。
/// `PluginNodeControl` 等の非 RT 半分は teardown closure が捕捉し、
/// lease と同寿命で保持する（本 slice では `input: None` のため
/// `InputTx` は発生しない）。
pub struct ProcessorLease {
    teardown: Option<Box<dyn FnOnce()>>,
}

/// connect 失敗 — `fault()` で [`RoutingFault`] へ決定的写像。
#[non_exhaustive]
pub enum ProcessorConnectError {
    /// instance を供給できない（Unresolved binding・未知 plugin id・
    /// instance 枯渇・endpoint 能力なし）。
    Unavailable,
    /// Initialize/Activate/StartProcessing/attach の host・
    /// lifecycle 拒否。
    Lifecycle,
    /// declared bus table と host report/bounds の不一致。
    Incompatible,
    /// request 内部不整合（port 解決不能などの防御経路）。
    Invalid,
}

/// chain processor 接続 boundary — engine が所有する注入点。
pub trait ProcessorHost {
    /// semantic processor を host 接続し RT leg + lease を返す。
    /// 失敗時はそれまでの接続を呼出側が lease drop で解放する。
    fn connect(
        &mut self,
        request: ProcessorConnectRequest,
    ) -> Result<ProcessorConnection, ProcessorConnectError>;
}
```

`ProcessorConnectError::fault()` の写像（spec R4）:
`Unavailable | Lifecycle → PluginUnavailable`、
`Incompatible → UnsupportedFormatOrTimeMap`、
`Invalid → InvalidRouting`。

### `StaticProcessorHost`（headless stub）

```rust
pub struct StaticProcessorHost {
    instances: BTreeMap<ProcessorId, Box<dyn GraphProcessor>>,
}
```

- `connect`: `instances.remove(&req.processor.id)` → Some なら
  `ProcessorConnection { processor, lease: ProcessorLease::empty() }`、
  None なら `Err(Unavailable)`。binding は参照しない
  （instance 供給 = 呼出側の解決済み表明。`ProcessorBinding::
  Unresolved` + 供給 instance の既存 test 契約を維持するため、
  binding 検査は実 host adapter の責務）。
- lease は no-op（`retained` があれば drop に委ねる）。
- 用途: 実 host を持たない test と、`RoutingEngine` 既定値
  （空 map → chain processor は `PluginUnavailable`、slice 1/2 の
  「instance 未供給」契約と同じ結果）。

### `PluginHostProcessors`（実 `ruviform-plugin-host` adapter）

```rust
pub struct PluginHostProcessors {
    host: Rc<RefCell<PluginHost>>,
}
```

- `new(host: PluginHost)` — factory/spec 登録は wrap 前に
  caller が `PluginHost` へ行う（`register_factory` /
  `register_out_of_process` は `PluginHost` の公開 API）。
- `connect` の手順（全て非 RT）:
  1. `req.processor.binding` が `Resolved(plugin)` でなければ
     `Err(Unavailable)`。
  2. `semantic_buses(&req.ports)` で `Vec<BusDescriptor>` を導出
     （後述）。空・異常は `Err(Invalid)`。
  3. `host.borrow_mut()` で `Instantiate { plugin_id:
     plugin.descriptor }` → `HostReply::Instantiated { handle }`。
     `UnknownPluginId`/`InstanceLimit`/`EndpointUnavailable` →
     `Unavailable`、その他 `HostError` → `Lifecycle`。
  4. `Initialize` → `Activate { config: ProcessConfig::new(
     rate, 1, max_frames) }` → `StartProcessing`。各失敗で
     `teardown(handle)`（後述の best-effort 列）を実行してから
     `Err(Lifecycle)`。
  5. `attach(&mut host, handle, &AttachConfig { buses,
     port_format: req.port_format, bounds: EndpointBounds {
     max_frames: req.max_frames, channels: layout.channels() },
     process_config, frame_base: rate.time_base(),
     policy: DeliveryPolicy::Exact, budget: ScheduleBudget::default(),
     input: None })`。`Bus`/`BoundsMismatch` → `Incompatible`、
     `Host(e)` は 3. と同じ写像、`InputConfig` → `Invalid`。
     失敗時は `teardown(handle)` して `Err`。
  6. `Attached { node, control, .. }` →
     `ProcessorConnection { processor: Box::new(node), lease }`。
     lease の teardown closure は `Rc<RefCell<PluginHost>>` clone +
     `handle` + `control` を捕捉し、drop 時に
     `try_borrow_mut` → `teardown` を実行する。
- `teardown(host, handle)`: `StopProcessing` → `Deactivate` →
  `Destroy` を順に `let _ =` で best-effort 実行する
  （既に Failed/途中状態の instance でも後続 command を試す。
  個別失敗は無視 — teardown は best-effort であり、残存は
  `PluginHost` drop が回収する）。
- bus 導出 `semantic_buses(ports)`:
  `processor.port_ids` 順の `SemanticPort` 各要素を
  `BusDescriptor { direction, layout: port_format.layout,
  name: "", role, enabled: true }` に写す。
  `PortDirection::Input/Output → BusDirection::Input/Output`、
  `PortRole::Main/Sidechain → BusRole::Main/Sidechain`。
  `name` は `""`（validate は name を比較しない）。
  この順序は lowered `NodeDef` の port 順（`emit_processor_node`
  が `port_ids` 順に emit）と同方向 per-index で一致し、
  `NodeIo` の port index 対応を保証する。plugin reported table
  との照合は `attach` 内 `validate_buses` が行う
  （reported の同方向順と declared の同方向順が一致しない場合は
  `BusError` → `Incompatible` で fail closed — port index が
  ずれる接続を黙って張らない）。
- `ProcessConfig::new(port_format.sample_rate, 1,
  req.max_frames)`。`min_frames=1` は pc_b の既存 host 契約と
  同じ寛容側（engine は常に `block_frames` で呼ぶ）。
  `SampleRate`/`TimeBase` は `port_format.sample_rate` から導出。

## `RoutingEngine` 統合

### `RoutingDeps` 改修

```rust
pub struct RoutingDeps<'a> {
    pub canonical_of: &'a dyn Fn(SourceId) -> Option<AudioFormat>,
    pub clips: Vec<PreparedClip>,
    pub engine_config_generation: u64,
    pub source_format_generation: u64,
    // processors map は削除 — host boundary が接続を担う。
}
```

### `RoutingEngine` フィールド追加

```rust
/// chain processor 接続 boundary（既定: 空 StaticProcessorHost）。
host: Box<dyn ProcessorHost>,
/// publish 成功済み世代 → その世代の接続 lease 群。
connections: BTreeMap<u64, Vec<ProcessorLease>>,
```

- `new` は `host: Box::new(StaticProcessorHost::default())` で初期化
  — 既存 test/呼出の「instance 未供給 → PluginUnavailable」結果を
  維持する。
- `set_processor_host(&mut self, host: Box<dyn ProcessorHost>)` を
  追加。将来の接続にのみ効く（既存 lease は自己完結 teardown の
  ため影響しない）。

### `prepare`/`stage_build` 改修

`prepare` の順序（slice 2 §3 を接続点だけ改訂）:

1. `checked_key`（不変）。
2. generation 予約（不変）。
3. `lower_semantic`（不変）。
4. skipped 昇格・clip 数検査（不変・`stage_build` より前）。
5. **`stage_build`** — 内部で chain processor ごとに
   `self.host.connect(request)` を呼ぶ:
   - `project.routing.processors[&processor]` を clone し、
     `processor.port_ids` を `project.routing.ports` で解決して
     `ProcessorConnectRequest` を組み立てる（port 欠落は
     `RoutingFault::PortUnresolved` — lowering が既に保証する
     防御経路）。
   - `connect` 成功 → `pending.insert(node, conn.processor)` +
     `connections.push(conn.lease)`。
   - `connect` 失敗 → `self.fail(request, key,
     err.fault())`。この時点までの `connections`/`pending` は
     drop で lease teardown + processor 破棄が走る
     （last-good は untouched）。
   - `SemanticNodeRole::ChainProcessor` 以外の role は従来どおり
     （Playback/StripFader/SendGain は engine 内蔵 processor）。
6. **offline capability gate を `stage_build` 直後に移動** —
   `staged.pending` の `ChainProcessor` node の
   `offline_support()` を検査（`deps.processors` 廃止のため
   接続済み instance を見る必要がある）。`Verified` 以外は
   `OfflineCapabilityMismatch`。失敗時 `staged` drop → lease 解放。
7. `project_rt.prepare` → compile（不変）。compile の
   `MissingProcessor` は到達不能になる（connect で供給済み）が
   `compile_fault` の写像は防御として残す。
8. `PreparedRouting` に `connections: Vec<ProcessorLease>` を追加
   — publish 成功まで staged 接続を保持する。

### `publish` 改修

`self.graph.publish(prepared.compiled)` 成功 arm で:

```rust
// 直前の published 世代が pending のまま置き換わるなら
// その接続を即時解放（RT がまだその plan を観測していない）。
if let Some(old) = self.ledger.published_generation
    && Some(old) != self.ledger.last_good_generation
    && let Some(leases) = self.connections.remove(&old)
{
    drop(leases);
}
self.connections.insert(generation, prepared.connections);
```

- publish の各 reject 経路（`blocked`/`fail_receipt`）では
  `prepared` が drop され `connections` の lease teardown が走る
  — 追加コード不要。
- `PublishQueueFull`（`graph.publish` Err）でも同様に
  `prepared` drop → teardown。`compiled` は publish に消費済み
  なので plan 自体は棄却され、接続も解放される。

### `observe_activation` 改修

Active 確定時（`last_good_generation` 前進と同じ箇所）:

```rust
self.ledger.last_good_generation = Some(generation);
// 旧世代の接続を解放 — RT は新世代を観測済みで旧 plan の
// processor は retire 対象。endpoint は instance destroy 後も
// detach 済みで安全。
self.connections.retain(|g, _| *g == generation);
```

- `retain` が外した世代の `Vec<ProcessorLease>` は drop →
  teardown。`connections` は高々 `{last_good, latest_pending}`
  の 2 世代を保持する不変条件になる。
- `seek` の drain `rt.process` 後にも `observe_activation` を
  呼ぶ（現状 seek 中の活性化が次回 `run` まで観測されず、
  その隙間に pending 世代が置き換わると旧世代 lease を
  稼働中に解放し得るため。1 行追加の観測漏れ修正）。

### `Drop`

`RoutingEngine` drop で `connections` map が drop → 全 lease
teardown。`host` field との drop 順は `Rc<RefCell<PluginHost>>`
の共有により安全（lease が host を保持し続ける）。

## Data and Control Flow

```text
prepare(request, project, revision, deps)
  └─ checked_key → lower_semantic → skipped/clip 検査
       └─ stage_build
            ├─ clip players / StripProcessor / GainProcessor（内蔵）
            └─ ChainProcessor ごと:
                 project.semantic → ProcessorConnectRequest
                 → host.connect()                      ← 非 RT
                      StaticProcessorHost: map から pop
                      PluginHostProcessors:
                        Instantiate→Initialize→Activate→StartProcessing
                        → attach → PluginNode + lease(Rc<RefCell<PluginHost>>)
                 → pending[node] = conn.processor
                 → connections.push(conn.lease)
            → offline gate（接続済み instance）
       → project_rt.prepare → compiler.compile
  → PreparedRouting { compiled, connections, ... }

publish(prepared, project, revision)
  ├─ reject 経路: prepared drop → lease teardown（best-effort destroy）
  └─ 成功: pending 置換世代の lease 解放 + connections[gen] = leases

run() 各 block 後 / seek drain 後
  └─ observe_activation: pending gen が RT 観測と一致・frames>0
       → last_good 前進 + connections.retain(gen) → 旧世代 lease 解放
```

RT 側に出るのは `PluginNode`（detach 済み `PluginEndpoint` 内包）
のみ。`PluginHost`・`PluginNodeControl`・`InputTx` は lease/adapter
内に留まり `GraphProcessor` には入らない。

## Non-Functional Constraints

- **RT 安全性**: `connect`/teardown/lease drop は全て非 RT。
  `PluginNode::process` は bridge 実装済みの RT-safe 経路
  （endpoint `process_audio`・preallocated scratch・明示的
  reject/bypass）。synvelis 側で新たな RT 処理を追加しない。
- **決定性**: `node_roles` は `BTreeMap<NodeId,_>` 走査で connect
  順序は node id 順に決定的。bus 導出は `port_ids` 宣言順。
- **Send/スレッド**: `ProcessorLease` は `Rc<RefCell>` 捕捉のため
  `Send` ではない → `PreparedRouting`/`RoutingEngine` は
  `Send` でなくなる。headless engine は制御スレッド単独で
  使う前提（`GraphProcessor: Send` の RT 境界は維持）。
  app 層が別スレッドを要求する場合は後続 slice で境界を再検討
  （本 slice は headless 契約のみ）。
- **エラー**: host/lifecycle 失敗は全て typed fault + best-effort
  teardown。panic 経路を新設しない（`try_borrow_mut` で lease
  teardown の再入 panic を回避）。

## Risks and Trade-offs

| リスク | 対応 |
| --- | --- |
| `PreparedRouting` が publish されず破棄 → host instance 残存 | lease の drop-teardown で自己解放（本設計の中核） |
| pending 世代が RT に活性化された直後に未観測のまま置き換わると稼働中 instance を destroy し得る | `observe_activation` を seek drain にも追加して観測遅延を除去。残存窓は publish→次 process のみで、pending 世代は「RT が一度も観測していない」場合に限り即時解放するため安全側 |
| plugin reported bus 順が semantic port 順と異なる → attach 拒否 | `validate_buses` の per-direction 順照合が fail closed。port index 対応が壊れる接続を黙って張らない（正しい拒否） |
| `PluginNode.offline_support() = Unknown` → Offline build が常に `OfflineCapabilityMismatch` | slice 2 契約どおり fail closed。verified 宣言の昇格は ruviform/synvelis-render 側の後続責務 |
| `Rc<RefCell<PluginHost>>` により engine/PreparedRouting が `!Send` | headless 契約では制御スレッド単独利用。app 配線時に再検討（spec 範囲外） |
| `capability_revision`/`version` の深い照合なし | bus report 照合 + Project 側 semantic identity で本 slice を構成。port 単位照合は SF-ACT-06 系へ defer |
| teardown の best-effort で Failed instance に対し Destroy 以外が失敗 | `let _ =` で個別失敗を許容。残存は `PluginHost` drop が回収。順序は StopProcessing→Deactivate→Destroy の定義順 |

## Deferred Design Concerns

- `PluginNodeControl` の保持・dispatch（param/automation）|
  SF-ACT-06 系 | control channel の event 配送契約が必要 | 後続 slice で `ProcessorLease` の `retained` 経路を公開 |
- L1 out-of-process plugin の実機検証 | 子プロセス spawn/IPC が必要 | adapter は composite `PluginHost` 経由で `register_out_of_process` 可能。検証は実 OS slice |
- `OfflineRenderSupport::Verified` の plugin 側証明 | ruviform 側の検証責務 | verified 宣言経路が確定したら offline gate を再評価 |

## Verification Notes

- [ ] `cargo check`/`clippy` で `host.rs` + engine 統合が lint clean
- [ ] `sf03_l1.rs`/`sf03_l2.rs` の `deps.processors` 参照を
      `StaticProcessorHost` 経由へ機械的に追従し green
- [ ] `sf03_l3.rs` で static/実 host の接続・fault・世代連動を検証
