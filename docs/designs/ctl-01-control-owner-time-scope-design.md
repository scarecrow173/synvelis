---
id: "DESIGN-034CTL01Qy1wZt7aC3dE4fGh"
type: "design"
status: "approved"
title: "CTL-01 ControlOwner/AutomationTimeScope 設計 — Schema v7 Migration"
created: "2026-11-05"
updated: "2026-11-05"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/121"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/ctl-01-control-owner-automation-time-scope-typed-schema.md"
    - "docs/adr/ctl-01-control-owner-time-scope-schema-v7.md"
  implemented-by: []
  depends-on:
    - "docs/designs/sf-01-semantic-routing-model-design.md"
    - "docs/designs/pc-a-parameter-control-design.md"
    - "docs/designs/pc-b-parameter-dispatch-note-scope-design.md"
    - "docs/adr/sf-01-routing-model-schema-v6-migration.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/adr/ctl-01-control-owner-time-scope-schema-v7.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/ctl-01-e2e-control-owner-time-scope.md"
  references:
    - "https://github.com/scarecrow173/synvelis/issues/105"
    - "https://github.com/scarecrow173/synvelis/issues/122"
    - "https://github.com/scarecrow173/synvelis/issues/101"
  defers: []
  deferred-by: []
---

# CTL-01 ControlOwner/AutomationTimeScope 設計

## 1. 目的と現行構造

`ControlScope`(control.rs)は `{"scope": "...", "id": ...}` で永続化
され、`ParameterTarget.owner`・`ModulationSource.owner` が所有と
時刻座標を兼務する。`resolve_scope_pos(project, scope, pos)` が
project beat → scope-local beat 解決を一手に担い、`evaluate_control`・
`EvalPlan::eval_node`・`ControlSession::record_points` が共有する。

SF-01 で導入された `RoutingModel` は `StripOwner::{Track,Bus}`・
`PortOwner::{Strip,Processor}`・`ConnectionKind::Send`・
`SendConfig.gain_target` を持ち、`RoutingModel::validate(&Project)`
が唯一の routing oracle。

## 2. 新規型(control.rs)

```rust
/// Parameter を所有する entity。DSP/bus 値は Strip が唯一の正本。
#[serde(tag = "owner", content = "id")]
pub enum ControlOwner {
    Project,
    Track(TrackId),     // 制作 container。DSP param は Track に帰属可
    Strip(StripId),     // Fader/Pan/Mute/Solo 等。bus 所有 strip もここ
    Send(SendId),       // Send level target
    Processor(ProcessorId),
    Clip(ClipId),       // ClipLocal と組合せ
    Note(NoteId),       // NoteLocal と組合せ
}

/// automation 点の pos が属する時刻座標系。
#[serde(rename_all = "kebab-case")]
pub enum AutomationTimeScope {
    /// project tempo-map beats — pos をそのまま使う
    ProjectMusical,
    /// absolute media time — 型のみ。解決は明示 ScopeUnavailable
    ProjectAbsolute,
    /// owner Clip の anchor 起点 beats
    ClipLocal,
    /// owner Note の domain 内 beats
    NoteLocal,
}

/// target の用途。singleton role は (owner, role) 一意。
#[serde(rename_all = "kebab-case")]
pub enum ParameterRole {
    Generic,
    Fader,
    Pan,
    Mute,
    Solo,
    SendLevel,
    ProcessorParam,
}
```

`ParameterTarget` 変更: `owner: ControlOwner` + `role: ParameterRole`
+ `time_scope: AutomationTimeScope`(wire 上 `role`/`time_scope` は
`#[serde(default)]` — v6 body は field を持たない。`role` default =
`Generic`、`time_scope` default = `ProjectMusical` で、migration step
が厳密導出する)。

`ModulationSource` 変更: `owner: ControlOwner` のみ。`role`/
`time_scope` は持たない — source signal は assignment 先 target の
解決済み座標で評価される契約(現行 `source.kind.value(local)` が
target の local pos を受ける)を維持する。

### `ControlOwner` の wire 互換

Deserialize は `{"owner": "<Variant>", "id": "<tagged>"}` に加え、
legacy `{"scope": "<Variant>", "id": "<tagged>"}` を受理する手書き
impl(DTO `{ tag(field alias: owner|scope), id?: String }` →
variant 別 typed id `from_tagged`)。v6 body は `SnapshotEnvelope`
全体が先に deserialize されるため、ここが migration 境界となる。
Serialize は新形式のみ。`ControlScope` 型自体は削除する。

## 3. 座標解決

```rust
pub fn resolve_scope_pos(
    project: &Project,
    owner: ControlOwner,
    time_scope: AutomationTimeScope,
    pos: BeatPoint,
) -> Result<BeatPoint, Error>
```

- `ProjectMusical` → `Ok(pos)`(owner は検証済み前提だが、Clip/Note
  owner との不整合は defensive に `Error::Validation`)。
- `ClipLocal` → owner が `Clip(id)` でなければ `Error::Validation`、
  現行の clip anchor 解決(Musical anchor 以外・domain 外 →
  `ScopeUnavailable`)をそのまま適用。
- `NoteLocal` → 同様に owner `Note(id)` → clip 解決 + note domain 検査。
- `ProjectAbsolute` → `Error::ScopeUnavailable{ "absolute time scope
  not yet supported" }`。型のみ導入、暗黙解釈しない(ADR D5)。

## 4. Validation oracle

`crates/synvelis-project/src/control.rs` に共有検査を集約し、
`Project::validate_references`(最終 oracle)と `ops.rs` の write-time
検査の双方から呼ぶ:

```rust
pub fn validate_control_binding(
    project: &Project,
    owner: ControlOwner,
    role: ParameterRole,
    time_scope: AutomationTimeScope,
    exclude: Option<ParameterTargetId>, // 自身除く uniqueness 判定用
) -> Result<(), Error>
```

1. **owner 存在検査**: `Track→tracks`, `Strip→routing.strips`,
   `Send→routing.sends`, `Processor→routing.processors`,
   `Clip→clips`, `Note→notes`, `Project→常に可`。
2. **owner×time_scope matrix**: `ClipLocal↔Clip`、`NoteLocal↔Note`、
   `ProjectMusical|ProjectAbsolute↔{Project,Track,Strip,Send,Processor}`。
   不整合は `Error::Validation`。
3. **(owner, role) 一意性**: `role.is_singleton()` =
   `Fader|Pan|Mute|Solo|SendLevel` のとき、同一 owner の singleton
   role target が既に存在すれば `Error::Validation`。
   `Generic`/`ProcessorParam` は複数許容。

`ModulationSource` は owner 存在検査のみ(`validate_control_owner`)。

`validate_references` は加えて以下を行う:

- 全 target について上記 1–3 を走査(uniqueness は owner+role を
  `BTreeSet` に集めて重複検出)。
- **`gain_target` owner mapping**: `routing.sends` の各 `send` で
  `gain_target = Some(t)` なら `targets[t].owner ==
  ControlOwner::Send(send.id)` を要求(SF-01 で文書化された CTL-01
  接続点)。routing.validate が先に存在検査を行うが、owner 検査は
  control map が要るため `validate_references` 層で行う。

既存の Track/Bus→Strip 一意性(`track_strips` set・bus→strip count
==1・`StripOwner::Bus` 双方向一致)は `RoutingModel::validate` が既に
保持しており変更しない。CTL-01 はテストで担保を明示する。Track の
strip 未配線は合法(`AddTrack` と `AddTrackStrip` は別 op)。

## 5. Ops / eval / record / dispatch

- `ProjectOp::AddParameterTarget` → `{ name, owner: ControlOwner,
  role: ParameterRole, time_scope: AutomationTimeScope, kind, mapping,
  capability, base }`。`validate_scope` を `validate_control_binding`
  (exclude = None)に置換。
- `ProjectOp::AddModulationSource` → `{ owner: ControlOwner, kind }`。
- `eval_inner` → `resolve_scope_pos(project, target.owner,
  target.time_scope, pos)`。
- `PreparedTarget` → `owner`+`time_scope` を freeze(placement は
  従来通り live 解決、`is_stale` 契約不変)。
- `ControlSession::record_points` → target から owner+time_scope を
  取り `resolve_scope_pos` に渡す。domain 外 skip/伝播契約不変。
- `lower_event`/`dispatch_check`/`ControlGraph::compile` → 変更なし。
- 依存 entity 削除: Track/Clip/Note 削除時の cascade は現行通り
  `owner` 一致で走査。`RemoveBus`(strip 連鎖削除)で Strip/Send/
  Processor 所有 target が dangling しないよう、各削除 op の
  cascade/preflight に routing entity 所有 target の除去または拒否を
  確認・追加する(実装時に `remove_strip_owned` 等の現行 cascade を
  監査し、欠けがあれば削除側で cascade する — oracle 違反状態を
  作らない)。
- `RemoveParameterTarget` は従来の lane/assignment/depth_source 参照
  検査に加え、どこかの `SendConfig.gain_target` が対象を指す場合も
  拒否する(逆方向の dangling を防ぐ)。

## 6. Migration(v6→v7、ADR D3/D4)

`migrate_v6_control(mut project) -> Result<Project, Error>`:

- 各 `ParameterTarget`: owner は deserialize 済み `ControlOwner` を
  そのまま使い、`role = Generic`、`time_scope = match owner {
  Clip(_) => ClipLocal, Note(_) => NoteLocal, _ => ProjectMusical }`。
- 各 `ModulationSource`: owner そのまま(型のみ変更)。
- ID 採番なし・`id_watermark` 不変。決定的・冪等な read-only 投影。
- `SCHEMA_VERSION = 7`、`STEPS` に `{ from: 6 }` 追加。

## 7. Gateway / workspace projection

- `snapshot.rs` `control_scope_json` → `control_owner_json` に改名し
  `{"owner": "<kebab>", "id": "<tagged>"}` を emit(project は id 無
  し)。param target projection に `"role"`・`"time_scope"` を追加。
- `control_scope_ref` → `ControlOwner` に追随。`Strip`/`Send`/
  `Processor` owner を表せるよう `ResourceRef`/`ResourceKind` に
  `Strip`/`Send`/`Processor` 変種を追加し、parse/name/enumerate を
  更新(Bus は owner 経路に現れないため本件では追加しない)。
- `synvelis-workspace::control.rs`: `resolve_scope_pos` 呼出を新
  arity に、`target_scope` を `(ControlOwner, AutomationTimeScope)`
  を返す形に更新。
- `synvelis-project/src/midi.rs`・`store.rs`・`lib.rs` export・全
  テストの `ControlScope` 参照を新 API に追随。

## 8. 検証方針

`docs/test-specs/ctl-01-e2e-control-owner-time-scope.md` の
Feature/Rule/Example に従い `crates/synvelis-project/tests/ctl_01.rs`
を中心に headless 検証。`mise run test` workspace 全体 +
`fmt-check`/`check`/`lint`/`test-rt`/`test-msrv` を PASS。
