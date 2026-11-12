---
id: "IMPL-034V1FQ09PSOnyDawvKX60"
type: "impl"
status: "completed"
title: "CTL-02 Strip/Send Host Parameter 正本・Plugin Binding・Routing 実装記録"
created: "2026-11-10"
updated: "2026-11-12"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/122"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-project/src/host_param.rs"
      - type: "file"
        path: "crates/synvelis-project/tests/ctl_02.rs"
      - type: "file"
        path: "crates/synvelis-player/src/host_param.rs"
      - type: "file"
        path: "crates/synvelis-player/tests/ctl_02.rs"
    modified:
      - type: "file"
        path: "crates/synvelis-project/src/control.rs"
      - type: "file"
        path: "crates/synvelis-project/src/ops.rs"
      - type: "file"
        path: "crates/synvelis-project/src/migration.rs"
      - type: "file"
        path: "crates/synvelis-project/src/routing_tx.rs"
      - type: "file"
        path: "crates/synvelis-project/src/store.rs"
      - type: "file"
        path: "crates/synvelis-project/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-player/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-project/tests/ctl_01.rs"
      - type: "file"
        path: "crates/synvelis-project/tests/sf_02.rs"
      - type: "file"
        path: "crates/synvelis-project/tests/routing_sf01.rs"
      - type: "file"
        path: "crates/synvelis-project/tests/md_a.rs"
      - type: "file"
        path: "crates/synvelis-project/tests/tim_02_smf.rs"
      - type: "file"
        path: "crates/synvelis-project/tests/vcs01_snapshot.rs"
      - type: "file"
        path: "docs/impl/ir/README.md"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on:
    - "docs/specs/ctl-01-control-owner-automation-time-scope-typed-schema.md"
    - "docs/designs/ctl-01-control-owner-time-scope-design.md"
    - "docs/plans/ctl-01-control-owner-time-scope.md"
    - "docs/specs/sf-01-semantic-routing-model-bus-send-chain-stable-ports.md"
    - "docs/specs/pc-c4-plugin-param-binding.md"
    - "docs/designs/pc-c4-plugin-param-binding-design.md"
    - "docs/designs/sv185-mixer-gain-projection-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/ctl-01-e2e-control-owner-time-scope.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# CTL-02 実装記録: Strip/Send Host Parameter 正本・Plugin Binding・Routing

## Summary

Issue #122 の headless slice を実装した。すべての Host DSP
Parameter を `Project::control` の `ParameterTargetId` 正本に統一し、
strip channel param(Fader/Mute/Solo、初回使用時の Pan)を strip
作成時に永続 target として登録、send は transaction 経路で必ず
`SendLevel` target を伴うことを強制し、stage-1 semantic plugin
binding(project 側)と stage-2 verified runtime binding(player 側
既存契約)を世代照合つき evidence として接続した。旧 raw `ParamId`
経路の誤配送はゼロであることを fixture で固定した。

## Context

Issue #122「CTL-02 Strip/Send Host Parameter正本・Plugin Binding・
Routing Atomic Commit統合」の headless slice。Start 契約は
#121(CTL-01 `ControlOwner`/`AutomationTimeScope`/`ParameterRole`)と
#105(SF-02 `RoutingTransaction`/`ParameterTarget`)が close 済みで
利用可能。#106(SF-03 `RoutingControlDispatch`/`RoutingSourceMap`)・
#100(PC-C3 ControlTrace)・#101/Ruviform #307(Verified Plugin
Binding)は Integration として既存契約をそのまま利用し、製品側への
generic parameter/graph 契約の複製は行わない。実 Plugin/OS/実機の
受入は別 Gate であり本 slice では `NotRun`。

## Implementation

### Strip channel ParameterTarget 正本 (`control.rs` / `ops.rs`)

- `STRIP_CHANNEL_ROLES = [Fader, Pan, Mute, Solo]` と canonical
  factory `strip_channel_target(id, strip, role)` を新設。
  `Fader`=Continuous base 1.0、`Pan`=Continuous base 0.5、
  `Mute`/`Solo`=Boolean base 0.0、すべて `ProjectMusical`・`Linear`・
  `capability: None`(internal channel gain は `ParamId` を持たない)。
- `STRIP_SEED_ROLES = [Fader, Mute, Solo]` のみを seed 対象とする。
  **Pan は意図的に非 seed**: `routing_lower::strip_gains` は Pan
  target 不在を bypass `[gain, gain]` として扱うが、base 0.5 の
  constant-power 則は `[0.707, 0.707]` となり全 strip が約 -3 dB
  変わるため、bit-exact を壊さないよう初回明示使用時に登録する。
- `ControlModel::bootstrap` は master strip に `Fader`/`Mute`/`Solo`
  を well-known ID(`MASTER_FADER_RAW`=+6、`MASTER_MUTE_RAW`=+7、
  `MASTER_SOLO_RAW`=+8)で登録。`insert_strip` は全 strip に
  `STRIP_SEED_ROLES` を発行 ID で登録する。
- `set_strip_bool` は canonical factory を再利用し、登録済み target
  の base 更新に一本化。teardown は `remove_control_owned` へ一般化
  し、strip/bus/send 削除時に所有 target・lane・assignment・
  source をカスケード削除して dangling owner を残さない。

### Guard (`ops.rs`)

- `AddSend`/`SetSendGainTarget` の adoption は well-known
  structural target を拒否(再所有・除去を防ぐ)。
- `RemoveParameterTarget` は登録済み strip channel target の除去を
  拒否。

### Send level authority (`routing_tx.rs`)

- `SendGainSpec::None` を撤去。`RoutingEdit::CreateSend` は
  `Adopt(既存target)` または `Create{name, initial}` のみを受理し、
  新規 enabled send は必ず `SendLevel` target を同一 transaction
  内で取得する。`SendConfig.gain_target` の `Option` は legacy/
  unconfigured 表現と明示的 `SetSendGainTarget { None }` 経路のみ
  に限定される。

### Host Parameter module (`host_param.rs`, project/player)

- project 側: `strip_channel_target_id` / `send_level_target_id` の
  正本 lookup と `semantic_plugin_binding`(stage-1: `ParameterTargetId`
  → `(ProcessorId, PluginRef, ParamId)`)を新設。fail-closed:
  processor owner でない・`ProcessorBinding::Unresolved`・
  capability 無しはいずれも typed `Error`。
- player 側: `binding_evidence` + `HostParamEvidence` +
  `HostParamDelivery` を新設。`Unbound`/`Stale{pinned,current}`/
  `Pending{generation,active}`/`Observed{generation}` の 4 状態で
  surface generation・source map generation・RT active generation
  の 3 軸を照合する。internal channel target は常に `Unbound`
  (lowering-time `base` 投影という別 binding であり plugin
  `ParamId` を触らない)ことを型として観測可能にした。

### Migration (`migration.rs` / `store.rs`)

- `SCHEMA_VERSION = 10`。v9→v10 `migrate_v9_strip_params` が全
  strip の欠落 seed target を補完する。master strip は well-known
  ID、その他 strip は `max(live issuable, id_watermark)+1` から
  `StripId`→role 順で決定的に採番し watermark を前進させる。
  既存 `(owner, role)` singleton は重複作成しない。
- 採番が 0 件の snapshot(master のみ等)では watermark を進めない
  (初版は無条件 +1 で ID を 1 つスキップする欠陥があり、ctl_01 の
  v6 fixture で検出・修正した)。

## Decisions During Implementation

- **Pan の非 seed 化**: canonical center(base 0.5)は legacy bypass
  と bit-exact でないため、seed せず初回使用時登録とした
  (`STRIP_CHANNEL_ROLES` には残し factory は完全対応)。
- **`SendGainSpec::None` 撤去**: transaction 経路の新規 send は
  必ず persisted level authority を持つ。`SendConfig.gain_target:
  Option` は legacy snapshot 互換のため残すが、新規作成経路では
  `None` を生成しない。
- **stage-1 は authored claim のみ**: `semantic_plugin_binding` は
  plugin の live param 列挙を持たないため param_id の実在性は
  stage-2 catalog 検証に委ねる。param 99 のような phantom
  capability は stage-1 で `(processor, plugin, 99)` を返すが
  runtime binding は張られず `Unbound` → 誤配送ゼロ。

## Related Experiments

### Adopted

なし(既知解の機械的実装のため exp は作成しなかった)。

### Rejected

なし。

## Validation

- `cargo test -p synvelis-project`: 全 suite green。新規
  `tests/ctl_02.rs` 9 件 — seed 登録・bit-exact base・Pan 初回
  登録・正本 lookup・send atomic commit/undo・adoption/removal
  guard・semantic binding・v9→v10 決定的 migration。
- `cargo test -p synvelis-player`: 全 suite green。新規
  `tests/ctl_02.rs` 3 件 — internal channel target が plugin
  `ParamId` に到達しないこと・世代 3 軸照合
  (`Observed`/`Pending`/`Stale`)・phantom capability/未存在
  target/ghost ParamId の typed reject と誤配送ゼロ。
- `mise run fmt-check` / `check` / `lint` / `test` /
  `test-rt` / `check-app-native` / `build-app-native`: すべて exit 0。
- #103/#109 Domain Oracle・実 Plugin/OS/実機行: **NotRun**。

## Risks

- 旧 v9 以前 snapshot は load 時に v10 で strip target を補完するが、
  補完前の in-flight 表現(automation lane 等)は migration で新規
  生成されない — seeded target への既存 lane は存在し得ないため
  実害はない想定だが、実データでの確認は残る。
- GUI/mixer projection 側が seeded target をどう表示するかは本
  slice の範囲外(正本は project 層にあり projection は reader)。

## Follow-ups

- GUI/mixer が strip channel target を fader/pan/mute/solo として
  投影・編集する結線は別 slice。
- 実 Plugin での verified binding 受入は別 Gate(NotRun)。

## 追記: canonical internal spec の write-time 強制(2026-11-12)

Issue #122 残件 slice。`AddParameterTarget` の validation が
owner 存在・time_scope 行列・singleton・base 範囲・
`cap.param_info` のみを検査し、internal channel/send target の
canonical spec を強制していなかったため、public op 経路で
「`Pan`（中央）」「内部 Gain/Pan は Plugin Param ID なし」の
契約に反する persisted 状態を作れた。あわせて
`owner:Send + role:SendLevel` の直接作成が `send.gain_target`
非参照の orphan target を生じ得た。

### 変更

- `control.rs`: strip channel role ごとの `(name, kind, seeded
  base)` テーブルを `strip_channel_spec` として抽出し、
  `strip_channel_target` factory と新設の
  `canonical_internal_spec(owner, role)` が同一 source を参照する
  構造に集約。`canonical_internal_spec` は `Strip`+
  `STRIP_CHANNEL_ROLES` と `Send`+`SendLevel` の `(owner, role)`
  に対し canonical `(kind, mapping=Linear, time_scope=
  ProjectMusical)` を返し、それ以外は `None`(自由 spec を維持)。
  `capability` は契約上一律 `None`。
- `ops.rs` `AddParameterTarget::validate`: 上記 2 クラスに対し
  canonical `kind`/`mapping`/`time_scope`/`capability: None` を
  要求し、違反は typed `Error::Validation` で reject。`name`/`base`
  は従来どおり authored。さらに `role: SendLevel` が `Send` 以外の
  owner を持つ直接作成を orphan として reject
  (`send_level_target_id` が解決するのは `Send` 所有
  `SendLevel` のみで、`gain_target` の persisted 不変条件も Send
  所有を要求するため)。
- `tests/ctl_02.rs`: `strip_channel_targets_require_the_canonical_spec`
  と `send_level_targets_require_the_canonical_spec` を追加 —
  `Strip+Pan`/`Send+SendLevel` の non-canonical
  kind/mapping/time_scope/capability:Some/base 範囲外を各々
  reject、canonical 受理(Pan base 0.25 authored・SendLevel は
  既存 low-level `SetSendGainTarget` 経路で bind)、非 Send owner
  の `SendLevel` orphan 作成 reject を fixture 化。

### 非破壊確認

- `SendGainSpec::Create` 経路(routing_tx staging が内部で発行する
  `AddParameterTarget{owner:Send, role:SendLevel}`)は canonical
  spec のためそのまま受理 — `sf_02`/`ctl_02`/`sf_xr_01` の send
  系 fixture は無変更で green。
- `Generic`/`ProcessorParam` 等他 role、Track/Project/Clip/Note/
  Processor owner、`capability: Some` の既存利用
  (`sv47_controls` の readonly fixture 等)は非対象のまま。
- 既存 seeded factory 呼出(`insert_strip`/`ControlModel::bootstrap`/
  `set_strip_bool`/snapshot migration)は `strip_channel_spec`
  経由で同一 spec を生成し、挙動不変。

### 残存観測(本 slice 対象外)

- `AddSend`/`SetSendGainTarget` の adoption は既存 target の
  owner/role を apply 時に `Send`/`SendLevel` へ書き換えるが、
  kind/mapping/time_scope/capability は書き換えない。canonical
  spec を持たない target を adopt すると非 canonical な
  `SendLevel` が persisted され得る — write-time 検査は
  `AddParameterTarget` のみが対象のため別 slice 候補として記録。
- 外部 gate(実 Plugin/OS/#103/#109 Oracle・AudioLens)は NotRun で
  あり本 slice の委譲先は変わらない。
