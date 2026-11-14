---
id: "EXP-cb06-strip-adapter"
title: "CB-06 Track/Bus Strip Adapter 設計実験メモ (synvelis#283)"
created: "2026-11-14"
updated: "2026-11-14"
---

# 実験記録: CB-06 Track/Bus Strip headless スライス（synvelis#283）

## E1 strip port（`PortOwner::Strip`）を scope object にするか

**仮説**: `StripFields` scope は常に strip entity を所有する。
strip の入出力 `SemanticPort` も「strip の一部」として
scope object に含めるべきか — 含めれば境界判定が単純に
なるが、adopt が外部 route の endpoint に触れる経路が
生まれないか。

- 調査: `scope_objects`（CB-01 正本）は strip port を
  scope object として導出しない — strip-affiliated として
  boundary 分析の対象にするだけ。port を scope object に
  すると、その port を endpoint に持つ main route・send・
  sidechain feed の adoption/confinement 境界が崩れ、
  「外部 routing は共有 context」という契約と矛盾する。
- 案 A（strip port を scope object 化）: 外部 connection
  の endpoint が候補所有 entity になり、adopt が共有
  routing を書き換える経路を許す。却下。
- 案 B（affiliated 判定のみ・scope object 非含有）:
  `boundary_dependencies` は strip port を持つ connection
  を typed inventory するが scope objects には含めない。
  両端が scope object の connection（完全内部 chain 配線）
  のみ boundary から除外する。

**観察**: 案 B を採用。`cb_strip_03_boundary_dependencies_
typed_and_mask_relative` で main route・send・sidechain
feed が型付き列挙され、`cb_strip_03_external_wiring_ops_
reject_typed` の adopt 後 connections/sends serialize 比較
で無変更を検証。`cb_strip_05_shared_strip_model_
unchanged_outside_scope` で strip model 非回帰を確認。

## E2 `ProcessorChain` の membership/順序を Inserts の scope object にするか

**仮説**: `Inserts` mask は strip の chain 全体（processor・
port・control closure・完全内部 connection）を所有する。
`ProcessorChain` entity 自体（および processor の並び順）
も scope object にすべきか。

- 調査: `scope_objects` は `ChainId`/`ProcessorId`/
  `SemanticPort`/control entity を導出するが当初は
  `ProcessorChain` record 自体を含めなかった — chain
  membership は adopt の after-snapshot 照合で再構成される
  共有構造と見なした。
- 案 A（chain record/順序を scope object 化）: 順序依存の
  scope identity が生まれ、live 側の reorder が scoped
  digest を動かして adopt を `StaleScope` で型付き拒否する。
  chain 構造 op の confinement は op 受理面が変わらない
  （`InsertProcessor` のみ受理・`Reorder`/`Remove` は
  `Unsupported`/`ScopeViolation` のまま）。
- 案 B（membership は entity 集合で所有・record は共有）:
  adopt plan の positional insert と `protected_fields` の
  processor list reset により、live reorder を挟んだ adopt が
  「候補でも live でもない第三の順序」を静かに生成し得る
  — review で確認された実害。却下。

**観察**: 案 A を採用（初版は案 B だったが review 指摘で
revert）。`ProcessorChain` を `Inserts` mask 時に scope
object として導出し、order 変化が `scope_fingerprint` に
現れる。`cb_strip_04_live_chain_reorder_stale_scope_rejects_
adopt` で live `ReorderProcessor` 後の adopt が
`VariantError::StaleScope` で拒否され live 不変であることを
検証。`cb_strip_02_inserts_and_channel_fields_gate_
independently` で insert が chain membership を正しく
更新し `ReorderProcessor`/`RemoveProcessor` が型付き拒否
されること、`cb_strip_04_adopt_mask_only_one_cas_undo_
save_reload` で chain membership が正しく transplant
されることを検証。

## E3 mask 外 field への波及をどう止めるか

**仮説**: `SetStripMute`/`SetStripSolo` は scope 内の
`Strip` entity を指すため `require_scope` だけでは通る。
しかし実際の変化は対応 `ParameterTarget.base`（mute/solo
channel）に波及する — mask 外 field の変化をどう
fail-closed にするか。

- 調査: `ParameterTarget` は `ControlOwner::Strip` +
  `ParameterRole::{Fader,Pan,Mute,Solo}` で channel field に
  写像済み（`strip_fields_objects` が role→field で所有
  導出）。`SetParameterBase`/automation/modulation op は
  対象 entity の `scope_objects` membership で自動的に
  mask 判定されるが、`SetStripMute`/`SetStripSolo` は
  `Strip` entity 自体への op であり entity 所有だけでは
  field 粒度の判定にならない。
- 案 A（op 適用後に project diff で mask 外変化を検出）:
  適用済み変更の rollback が必要で fail-closed 契約に
  反する。却下。
- 案 B（pre-edit で op variant ごとに mask 照合）:
  `check_strip_fields_op_targets` が `SetStripMute`→
  `Mute`、`SetStripSolo`→`Solo` の宣言を要求し、未宣言は
  `Unsupported`。`InsertProcessor` は `Inserts` 宣言を
  要求。control closure op は `scope_objects` membership
  に委ねる — 正本を再解釈しない。
- 補足（review 指摘で追加）: `ModulationAssignment` の
  所有導出も mask 外波及経路になる。共有
  `control_closure` は「target・depth_source・source の
  いずれかが所有済みなら assignment を所有」とするため、
  strip-owned source から未宣言 target への既存 assignment
  が scope 所有になり `Set`/`RemoveModulationAssignment`
  が通ってしまう。effect target 自体が mask 外 field の
  変化を candidate に持ち込む。

**観察**: 案 B を採用。`cb_strip_02_mask_gated_channel_
edits`（`{Mute}` のみ — mute 面は受理・fader/pan/solo/
inserts/foreign は全て型付き拒否）と
`cb_strip_02_inserts_and_channel_fields_gate_
independently`（`{Fader,Pan,Inserts}` — mute/solo 拒否・
channel+insert 面受理）で検証。capability 報告
（`StripFieldsCapabilities`）が同一 mask 判定を反映し
enforcement と一致することを `cb_strip_rcp_01` で確認。

assignment の所有は StripFields 側で post-filter し、
「effect target が scope 所有」かつ「depth_source が
存在する場合はそれも scope 所有」のときのみ scope 所有と
する（source の所属だけでは所有しない）。これは lane の
target confinement と同じ向きの規則で、CB-04/CB-05 が
依存する共有 `control_closure` の ANY-endpoint 規則は
変更しない。`cb_strip_02_assignment_effect_target_gates_
ownership`（strip-owned source → 未宣言 target 拒否・
宣言済み target は編集可）と
`cb_strip_02_processor_assignment_effect_gates_ownership`
（processor-owned source → 未宣言 target 拒否）で検証。

## E4 routing op を部分受理するか（send/sidechain/bus/monitor）

**仮説**: strip 候補が自身の send や monitor に触れる
余地を残すべきか — Issue は「Routing 外部依存 → 明示検証
または Reject」を要求。

- 調査: CB-05 と同じく、外部配線 op を候補編集経路で
  受理すると shared context の confinement 保証が崩れる。
  安全な confinement 表現（boundary 越えの差分適用）は
  本 slice の契約を超える。部分受理は「どの op が安全か」
  の判断を adapter ごとに分散させる。
- 案 A（send tap/gain 等の限定受理）: 受理面の根拠が
  routing semantics に依存し、CB-05 の全 reject 契約と
  不整合。却下。
- 案 B（routing op 全 `Unsupported` + typed inventory）:
  `boundary_dependencies` が strip-affiliated
  `SemanticPortId` endpoint を持つ connection を列挙し、
  配線変更は shared-context digest を動かして比較 context
  を stale にする — false equivalence を作らない。

**観察**: 案 B を採用。`cb_strip_03_external_wiring_ops_
reject_typed` で connection/send/tap/channel-map/bus/
monitor/chain 構造 op が全て型付き拒否され、adopt が
connections/sends を一切変更しないことを検証。
`cb_strip_03_boundary_change_stales_shared_context_only`
で live 側の feed 変更が shared-context digest のみを
動かし scope digest を保持することを検証。

## E5 bus strip の recording gate lane 写像

**仮説**: CB-05 は track strip → 所有 track lane、bus
strip → 全 project lane の fail-closed 写像を
`strip_recording_gate_lanes` として共有化した。CB-06 は
bus-owned strip を直接 scope root にする初の adapter —
同 helper が bus strip で正しく全 lane を検査するか。

- 調査: `strip_recording_gate_lanes` は `StripOwner::Track`
  → track の lane、`StripOwner::Bus` → 全 project lane を
  返す。bus strip の帰属 lane は一意に決められないため
  全 lane 検査が安全側。track/bus 両 owner の scope が
  同一 `StripId` 解決を共有するため、gate も同一経路で
  よい。
- 案 A（bus strip 専用の lane 推定）: 帰属を推定すると
  recording 中の lane を見逃し得る。却下。
- 案 B（共有 helper をそのまま適用）: CB-05 の権威ある
  実装を再利用 — 判定規則の authoritative 実装は 1 箇所。

**観察**: 案 B を採用。`cb_strip_04_adopt_while_recording_
rejects_typed` で track strip の owning lane recording
Open が `RecordingActive` で adopt を先行拒否し
live/revision 不変であることを検証（bus strip の全 lane
検査は同一 helper の既存契約 — CB-05 fixture で検証済み）。
