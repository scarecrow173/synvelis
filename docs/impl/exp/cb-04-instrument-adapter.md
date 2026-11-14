---
id: "EXP-cb04-instrument-adapter"
title: "CB-04 Instrument Slot Adapter 設計実験メモ (synvelis#281)"
created: "2026-11-14"
updated: "2026-11-14"
---

# 実験記録: CB-04 Instrument Slot headless スライス（synvelis#281）

## E1 candidate 内の plugin binding/preset 変更をどう表現するか

**仮説**: CB-INST-02 は Plugin 種類・Preset・State 差を
Semantic Diff として返すことを要求する。candidate が fork
後に plugin/preset を差し替える編集 surface が必要かもしれない。

- 調査: `ProjectOp` variant を全走査したが、`Processor`
  entity の binding/preset を書き換える専用 op は存在しない
  （`InsertProcessor` は chain 構造変更で scope 外、
  `Composite` は unsupported）。`ProcessorBinding` の
  Resolved↔Unresolved 遷移は live 側の host param/binding
  sync が担う領域であり、candidate confinement の対象 op
  として定義されていない。
- 案 A（新規 `ProjectOp` variant を追加）: model 変更 +
  apply/validate/migration の全経路整備が必要で、
  `synvelis-project` の永続 model に触れる — schema v14
  据置の本スライス契約を超え、CB-05 以降の adapter 全てに
  影響する engine/model slice の責務。却下。
- 案 B（現行 op surface のみ・差分は semantic diff が報告）:
  candidate は fork 時点の live clone を出発点に control
  state のみ編集可能。plugin 種類/Preset 差は fork 時点の
  live との差として `diff/adapters.rs` の processor
  adapter + `PluginInstance` diff が報告し、opaque state
  は digest/品質区分のみ記録。candidate 側 binding 変更は
  `Unsupported` で fail-closed — silent substitution も
  fake success もない。

**観察**: 案 B を採用。`cb_inst_02` で semantic diff
（ParameterTarget Modified/Added）と private param の
`Unavailable`（capability-less target は stage-1 binding
で typed reject）を検証。plugin/preset の candidate 側
変更 op は follow-up として ledger/PR に記録。

## E2 `PluginInstance` を scope objects に含めるか

**仮説**: Instrument Slot の独立 plugin state を保証する
なら、`PluginInstanceIdentities` の instance も scope
object として adopt で materialize すべきかもしれない。

- 案 A（scope objects に追加）: `ObjectKind::PluginInstance`
  は keyed-synthetic kind で `adopt_supported_kind` に
  含まれず、entity transplant 経路が存在しない。追加には
  adopt surface の拡張が必要 — keyed-synthetic kind の
  扱いは diff/apply 契約全体に関わる engine slice の
  責務。却下。
- 案 B（scope 外・`sync_binding` dispatch に委譲）:
  `PluginInstanceIdentities::sync_binding` が全 mutation
  path で Resolved→`ensure`/Unresolved→`remove` を単一
  dispatch として共有済み（TIM-04/plugin instance
  identity 既存契約）。candidate は full project clone
  として独自の `plugin_instances` map を持ち、adopt
  materialize 後も live 側 sync が正しく同期する —
  fixture で Resolved processor の instance id 保持と
  sibling への非波及を検証。

**観察**: 案 B を採用。`cb_inst_01`/`cb_inst_04` で
candidate の `plugin_instances.instances[inst].id` が
live と独立に保持され、adopt 後も live の instance
identity が正しく解決されることを確認。

## E3 control entity confinement を adapter 複製か helper 拡張か

**仮説**: instrument 系 op（parameter/automation/modulation）
の scope confinement を `InstrumentSlotAdapter` 固有に
実装するか、共有 helper を拡張するか。

- 案 A（adapter 固有実装）: `check_op_targets`/
  `check_created_within_scope` は現状これらの op を
  `Unsupported` whitelist で reject する構造。adapter 側に
  別実装を置くと Fake/Audio/MIDI/Instrument で confinement
  規則が drift する — helper docstring が明示する共有契約
  に反する。却下。
- 案 B（helper 拡張）: `PreEditIds` に control entity
  4 集合（targets/lanes/sources/assignments）を追加し、
  `check_op_targets` に instrument scope の target 解決を
  実装 — 既存 entity 参照は `require_scope`、新規生成は
  owner=scope Processor のみ許可。CB-02/03 と同一 helper
  を共有し、entity kind 追加時の拡張点も明確。

**観察**: 案 B を採用。`cb_inst_05_unsupported` で
shared-context op=`Unsupported`、scope 外 control
entity=`ScopeViolation`、scope entity removal=
`Unsupported` の 3 語彙が型付きで返ることを検証。
lint/test 全 green。

## E4 state 品質の記録形 — assessment を adapter に置くか

**仮説**: `PluginStateAssessment`（binding + outcome +
achieved quality）をどこに置くか。

- 背景: TIM-04 の `PluginStatePort`/`PluginStateOutcome`/
  `StateCaptureQuality` は `plugin_state.rs` の既存契約。
  adapter は opaque blob を deep-inspect せず、port の
  outcome をそのまま記録する責務のみを持つ。
- 案 A（engine/variant types に配置）: assessment は
  domain adapter の報告語彙であり、engine は adapter を
  通じてのみ domain を知る — engine 層への配置は依存
  方向を歪める。却下。
- 案 B（`InstrumentSlotAdapter::assess_state` 静的
  メソッド + `PluginStateAssessment` 型を instrument.rs
  に配置）: port を注入して outcome を assessment に
  写すだけの薄い変換。`exact_eligible()` は TIM-04 の
  `StateCaptureQuality::exact_eligible` をそのまま
  委譲し、品質昇格の経路を構造的に排除。

**観察**: 案 B を採用。`cb_inst_02`/`cb_inst_rcp_01` で
Coordinated/Quiescent=exact eligible、BestEffort=非
elevated、Unsupported/Failed=artifact 無しの区別を
検証 — Missing Plugin × state 品質 matrix の全セルで
Safe Edit/Adopt・Exact Artifact・BitExact regenerate の
3 区別が保たれることを確認。
