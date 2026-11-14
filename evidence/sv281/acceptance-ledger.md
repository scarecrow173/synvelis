# Acceptance Ledger — synvelis#281 CB-04 Instrument Slot headless スライス

Date: 2026-11-14. Slice: CB-04 `VariantScope::InstrumentSlot`
実ドメイン Adapter（`InstrumentSlotAdapter`）+ processor-owned
control entity の scope confinement + headless fixture/回帰
テスト。残ドメイン Adapter（CB-05〜08 = #282〜#285）・
Compare Orchestrator（#285）・GPUI（#286）・Action/Gateway
統合（#287）・Cross-Gate/実機受入（#288）は DEFER。

対象 Issue: https://github.com/scarecrow173/synvelis/issues/281
（OPEN・`Refs` のみ・Close しない）。

## 計測環境

| 項目 | 値 |
| --- | --- |
| Synvelis SHA | scarecrow173/sv281-cb04 HEAD（ledger は同 commit 内） |
| OS / CPU | Windows 11 / Intel Core i9-9900K |
| Toolchain | mise 管理 Rust（`mise run` 経由） |
| Project schema | v14（schema 非変更 — 永続 model への entity 追加なし） |

## 判定語彙

- **Pass (headless fixture)**: `crates/synvelis-project/tests/
  cb04_instrument_slot.rs` が `mise run test synvelis-project`
  で green。
- **NotRun**: 実 OS/実 Plugin/実 RT/実 Render/実デバイス入力
  pipeline 依存の判定 — 本スライスでは実行していない。
- **Deferred**: 後続 Issue の範囲として明示的に送った部分。
- **Unsupported**: project 表現または engine surface が存在
  せず、型付き拒否が正しい verdict のもの。

## 受入条件 → 証拠対応

| AC | 判定 | 証拠 |
| --- | --- | --- |
| CB-INST-RCP-01 | Pass (headless) | `cb_inst_rcp_01_missing_plugin_x_state_quality_matrix` — Missing Plugin（`NoPlugins` availability）× state 品質（Captured exact/degraded `BestEffort`/`Unsupported`/`Failed`）の全セルで、Safe Edit/Adopt（project truth は runtime availability で block されない）・Exact Artifact（capture 成功時のみ blob+digest 存在）・`BitExact` regenerate（`exact_eligible()` は `Quiescent`/`Coordinated` のみ true、`BestEffort` は昇格しない）の 3 区別を検証。実 Render 再利用評価・invalidation 実行本体は RCP-01/`synvelis-render` 側・Deferred。 |
| CB-INST-01 | Pass (headless) | `cb_inst_01_candidates_isolated_shared_midi_untouched` — 同一 MIDI source/clip を読む 2 候補で、B の param base/新規 plugin param/automation lane+region/modulation source+assignment が candidate のみへ適用され、live・adopted mirror・sibling processor の control state・`midi_sources`/`clips`（serialize 比較）・`plugin_instances` identity が不変。scope digest のみ変化し shared_context_digest は不変。A/B の opaque state/voice 独立性は full project clone の構造的分離 + `sync_binding` dispatch で担保（実 plugin の voice/tail 独立性は NotRun）。 |
| CB-INST-02 | Pass (headless) | `cb_inst_02_semantic_diff_state_quality_and_capabilities` — param base 変更が `Modified`・新規 target が `Added` の Semantic Diff として返る。capability-less（非公開）target は stage-1 binding で typed `Validation` reject = `Unavailable`（値を捏造しない）。`InstrumentSlotCapabilities` が param_base/automation_edit/modulation_edit=Supported・plugin_rebind/preset_select/routing/midi_input=Unsupported を報告（display = enforcement）。`assert_state_assessments` で `Coordinated`/`Quiescent`/`BestEffort`/`Unsupported`/`Failed{Timeout}` を verbatim 記録し `exact_eligible()` の昇格なしを確認。Plugin 種類/Preset 差は fork 時点差分として processor/`PluginInstance` diff が報告 — candidate 側 binding/preset 変更 op は `ProjectOp` に存在せず `Unsupported`（exp E1）。 |
| CB-INST-03 | Pass (headless・identity 層) | `cb_inst_03_shared_midi_edit_stale_context_detected` — pin した scope/shared-context digest に対し、共有 MIDI source への live 編集が `shared_context_digest` を移動させ、stale expected revision の adopt を型付き拒否。scoped candidate 編集は `scope_digest` のみ移動。比較 orchestration 本体・実音響差分は CB-08（#285）/実機・Deferred。Tempo/PDC は scope 外 live object として shared-context digest に含まれる既存契約（`resolve_render_project` 投影済み）。 |
| CB-INST-04 | Pass (headless) | `cb_inst_04_adopt_swaps_slot_one_cas_undo_redo` — Adopt B が対象 Instrument Slot + control closure のみを差し替え（live revision +1 の 1 CAS）、sibling processor・MIDI source/clip・他 scope を不変に保つ。Undo で採用前 live 復元・Redo で B′ 復帰、variant set と candidate content ref は維持。Slot+binding 参照検証は既存 `plan_adopt_writes` + `preflight_assets`/`preflight_plugins`/`preflight_ports` を継承。 |
| CB-INST-05 | Pass (headless) | `cb_inst_05_unsupported_and_out_of_scope_ops_reject_typed` — `Composite`/MIDI source・note・processor insert 等の scope 外 op が `Unsupported`、sibling owner の control entity 生成/参照が `ScopeViolation`、scope entity removal が `Unsupported` で型付き拒否・live/candidate 不変。`cb_inst_05_adopt_while_recording_rejects_typed` — owning track lane の recording session Open で `RecordingActive` 先行拒否・live 不変。`cb_inst_05_missing_plugin_and_state_gates_are_honest` — Missing Plugin 下でも scoped edit/adopt は project-truth として進行（runtime availability は別 receipt domain）。実 Recording Writer/実デバイス入力・Last-good 音声実行は NotRun/Deferred（runtime/publish 側責務）。 |

## 補足: scope validation 回帰

`cb_inst_05_unsupported_and_out_of_scope_ops_reject_typed` 末尾 —
media clip scope への `InstrumentSlotAdapter` 適用が
`InvalidScope`、`InstrumentSlot` scope への `MidiClipAdapter`
適用が `InvalidScope` の鏡像拒否（CB-02/03 先例と同型）。

## NotRun / Deferred 一覧

- 実 OS provider・実 Plugin host（実 VST3/CLAP/AU
  instance）・外部 Plugin editor/Native Dock GUI・実 RT・
  実 Render pipeline・実デバイス入力 pipeline・workspace/
  GPUI projection: NotRun（headless fixture のみ。
  Fake/Domain Oracle の PASS を実機 PASS に昇格しない）。
- 実 plugin の voice/tail 独立性・opaque state の実
  capture/restore 品質・OS/Format 別受入: NotRun/Deferred
  （CB-INST-06・#114・#288 の責務）。
- Compare orchestration・semantic merge・blend: CB-08
  （#285）Deferred。
- candidate 内 plugin binding/preset 差し替え: `ProjectOp`
  surface 拡張が前提の別 Issue（現状 `Unsupported` が
  正直な verdict）。
- engine の set 内 branch 追加 op（literal な同時 N>2
  candidate）: CB-03 と同じく後続 engine slice。

## CB-INST-06 実 Plugin 受入対応表（OS・Format 別 — 全行 NotRun）

| Format | OS | Plugin 例 | 判定 | 理由 / 後続 |
| --- | --- | --- | --- | --- |
| VST3 | Windows | 実 instrument plugin | NotRun | 実 plugin host・実 OS 環境が必要（#114・#288） |
| VST3 | macOS | 実 instrument plugin | NotRun | 同上 |
| VST3 | Linux | 実 instrument plugin | NotRun | 同上 |
| CLAP | Windows | 実 instrument plugin | NotRun | 同上 |
| CLAP | macOS | 実 instrument plugin | NotRun | 同上 |
| CLAP | Linux | 実 instrument plugin | NotRun | 同上 |
| AU | macOS | 実 instrument plugin | NotRun | 同上（AU は macOS のみ） |
| 外部 Plugin GUI / Native Dock | Windows | #176 Dock gate | NotRun | GUI lane は #176・Floating/Generic 表示のみでは実 Dock Pass としない |
