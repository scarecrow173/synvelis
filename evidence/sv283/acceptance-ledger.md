# Acceptance Ledger — synvelis#283 CB-06 Track/Bus Strip headless スライス

Date: 2026-11-14. Slice: CB-06 `VariantScope::StripFields`
実ドメイン Adapter（`StripFieldsAdapter`）+ 安定 `StripId`
解決・`StripDomainMask` field gating・`SemanticPortId` 型付き
境界依存 inventory・外部 routing fail-closed・headless
fixture/回帰テスト。残ドメイン Adapter（CB-07/08 =
#284/#285）・Compare Orchestrator（#285）・GPUI（#286）・
Action/Gateway 統合（#287）・Cross-Gate/実機受入（#288）は
DEFER。

対象 Issue: https://github.com/scarecrow173/synvelis/issues/283
（OPEN・`Refs` のみ・Close しない）。

## 計測環境

| 項目 | 値 |
| --- | --- |
| Synvelis SHA | scarecrow173/sv283-cb06 HEAD（ledger は同 commit 内） |
| OS / CPU | Windows 11 / Intel Core i9-9900K |
| Toolchain | mise 管理 Rust 1.99.1（`mise run` 経由） |
| Project schema | v14（schema 非変更 — 永続 model への entity 追加なし） |

## 判定語彙

- **Pass (headless fixture)**: `crates/synvelis-project/tests/
  cb06_strip_fields.rs` が `mise run test synvelis-project`
  で green。
- **NotRun**: 実 OS/実 Plugin/実 RT/実 Render/実デバイス
  入力 pipeline 依存の判定 — 本スライスでは実行していない。
- **Deferred**: 後続 Issue の範囲として明示的に送った部分。
- **Unsupported**: project 表現または engine surface が存在
  せず、型付き拒否が正しい verdict のもの。

## 受入条件 → 証拠対応

| AC | 判定 | 証拠 |
| --- | --- | --- |
| CB-STRIP-RCP-01 | Pass (headless) | `cb_strip_rcp_01_render_identity_capability_diff` — captured payload digest が store key として live 共有編集後も不変（candidate bytes は fork 時点のまま decode 可能）。`BranchRenderPin` が scope/shared-context digest 合成で RCP cutset 非依存に branch identity を提供。`StripFieldsCapabilities` 報告（fader/pan/mute/inserts/automation/modulation=Supported、solo/entity_remove/routing/plugin_rebind/preset_select=Unsupported）が enforcement と一致し、狭い mask は狭い真実を報告。semantic diff が scoped 変更を `Modified` で報告。比較 orchestration 実行本体・実 Render 再利用評価は RCP-01/`synvelis-render` 側・Deferred。 |
| CB-STRIP-01 | Pass (headless) | `cb_strip_01_track_and_bus_strip_resolve_same_stable_identity` — track-owned strip と bus-owned strip が同一 `StripId` で解決され、candidate は full project clone で既存 `ChannelStrip`/`ChainId`/port をそのまま参照 — 二重 DSP strip・二重 chain・二重 port の instantiate なし（serialize 比較で strips/chains 不変）。scope は安定 `StripId` + 非空 `StripDomainMask` のみを宣言し entity payload を複製しない。 |
| CB-STRIP-02 | Pass (headless) | `cb_strip_02_mask_gated_channel_edits` — `{Mute}` mask で `SetStripMute`・mute target base・mute automation lane+region・strip-owned modulation source が受理される一方、`SetStripSolo`・fader/pan/solo target base・processor target・`AddParameterTarget`（未宣言 role/foreign strip owner）・`InsertProcessor`・未宣言 target への modulation assignment が全て `Unsupported`/`ScopeViolation`。`cb_strip_02_inserts_and_channel_fields_gate_independently` — `{Fader,Pan,Inserts}` で channel+insert 面は受理・mute/solo/foreign/chain 構造は拒否。`cb_strip_02_assignment_effect_target_gates_ownership` — strip-owned LFO source から未宣言 Fader target への既存 assignment が `Set`/`RemoveModulationAssignment` で `ScopeViolation`、宣言済み Pan target への assignment は編集可（effect-target confinement）。`cb_strip_02_processor_assignment_effect_gates_ownership` — processor-owned source → 未宣言 target の assignment も同様に拒否。`cb_strip_02_assignment_depth_source_gates_ownership` — 宣言済み target でも depth_source が scope 外なら assignment は非所有で `Set`/`Remove` が `ScopeViolation`、depth_source が宣言済みなら編集可。`cb_strip_02_independent_masks_edit_and_adopt_isolated` — `{Mute}`(track) と `{Inserts}`(bus) の独立 mask が別々に編集・adopt され互いの field・他 strip・外部 routing は不変。mask 判定は `scope_objects` membership を正本とし silent widen/normalize なし。 |
| CB-STRIP-03 | Pass (headless) | `cb_strip_03_boundary_dependencies_typed_and_mask_relative` — strip main route・strip→bus send・key strip→scoped processor sidechain feed が `boundary_dependencies` に `SemanticPortId` endpoint・connection identity・kind・scope 内外判定付きで typed inventory され、scope objects に含まれない（共有 context）。mask が狭まると依存 inventory も mask 相対で縮む。`cb_strip_03_external_wiring_ops_reject_typed` — 境界 connection への `RemoveConnection`/`SetConnectionEnabled`/`SetConnectionChannelMap`、新規 `AddConnection`（sidechain/main）・`AddSend`、send surface 3 op、chain 構造・bus/track/monitor op が全て型付き拒否。adopt は connections/sends を serialize 比較で一切変更しない（implicit reconnection/double-mix なし）。`cb_strip_03_boundary_change_stales_shared_context_only` — live 側 feed 変更が shared-context digest のみを動かし scope digest を保持 — false equivalence なし。zero-delay cycle 検証は routing cycle oracle 側の既存契約 — candidate は cycle を作る op を持たないため構造的に発生しない。 |
| CB-STRIP-04 | Pass (headless) | `cb_strip_04_adopt_mask_only_one_cas_undo_save_reload` — adopt が宣言 mask の全 field（mute/fader/pan base・first-use Pan target・chain membership・automation・modulation・processor-owned source）を materialize（live revision +1・undo_steps=1 の 1 CAS）。connections/sends/strips/solo/fx2・sibling strip channel state は serialize 比較で不変。Undo で採用前復元・Redo で再適用、adopted branch は captured content へ freeze。save→load で `Project` 全体（variant_sets 含む）が round-trip。`cb_strip_04_stale_scope_digest_rejects_adopt` — pin 後の live scoped 編集で `StaleScope` 拒否・live 不変。`cb_strip_04_live_chain_reorder_stale_scope_rejects_adopt` — intent 取得後の live `ReorderProcessor` が `ProcessorChain` scope 所有を通じて scope digest を動かし `StaleScope` 拒否（positional merge による未 author 順序の生成を防止）。`cb_strip_04_adopt_while_recording_rejects_typed` — owning lane の recording Open で `RecordingActive` 先行拒否（bus strip は全 lane fail-closed の共有 helper）。`cb_strip_04_missing_plugin_gate_rejects_before_mutation` — `NoPlugins` availability で adopt が `MissingPlugin` fail-closed（live 不変）、`YesPlugins` で `Applied` — project-truth edit/adopt は runtime availability を越えて進行。`cb_strip_04_runtime_publish_failure_keeps_project_committed` — `FakeRuntimePublish` の `Failed` が project commit を rollback せず別 receipt に記録。`cb_strip_06_insert_state_assessment_verbatim` — `assess_insert_state` が chain 全 processor（insert 含む 3 件）を chain 順に評価し `Coordinated`/`BestEffort`/`Failed{Timeout}` を verbatim 記録 — `BestEffort` は `exact_eligible()=false` で昇格なし、失敗は型のまま。実 plugin instance 独立性・PDC/RT bounded・PCM 比較は fake vs real-plugin の別 gate — 実機 lane は全行 NotRun（偽装昇格なし）。 |
| CB-STRIP-05 | Pass (headless) | `cb_strip_05_shared_strip_model_unchanged_outside_scope` — live 側の他 strip op（key strip fader base・bus strip solo）が variant scope の影響なく通常動作し、adopt は宣言 mask のみを書いて共有編集を温存（key fader 0.8・bus solo 1.0・sends/connections 数不変）。**SF-06 Oracle mapping**: 本 adapter は Bus/Return/Main/Solo/Automation の strip model を再実装せず、SF-06 cross-gate oracle（ADR-034・Issue #109）が対象とするのと同一の `ChannelStrip`/`StripId`/`SemanticPortId`/`RoutingConnection` model を共有する — scope 判定は `scope_objects` 導出の読み取り専用であり、live project への書き込み経路は既存 op/CAS/Undo のみ。`mise run test synvelis-project` 全 suite green（routing/control/snapshot/vcs 系を含む）で SF-06 Oracle が依存する既存 model の非回帰を確認。SF-06 oracle 自体の cross-gate 受入 lane は本 slice の範囲外（別 Issue の責務）であり、headless PASS を oracle PASS に昇格しない。 |

## 補足: scope validation 回帰

`cb_strip_01_track_and_bus_strip_resolve_same_stable_
identity` — 空 mask・未知 `StripId` は `VariantError::
Project`、foreign scope kind（`InstrumentSlot`）は
`InvalidScope` で型付き拒否（silent widen なし）。
`cb04_instrument_slot.rs` 8 件・`cb05_fx_range.rs` 13 件
全件 green — `adapter.rs` の共有 op-gate 追加
（`is_strip_fields_op`/`check_strip_fields_op_targets`）
による CB-04/CB-05 挙動変更なし（dispatch は scope kind
で分岐し既存経路を保持）。

## NotRun / Deferred 一覧

- 実 OS provider・実 Plugin host（実 VST3/CLAP/AU
  instance）・外部 Plugin editor/Native Dock GUI・実 RT・
  実 Render pipeline・実デバイス入力 pipeline・workspace/
  GPUI projection: NotRun（headless fixture のみ。
  Fake/Domain Oracle の PASS を実機 PASS に昇格しない）。
- 実 plugin の instance 独立性・PDC/RT bounded 動作・
  PCM/A-B 比較・opaque state の実 capture/restore 品質:
  NotRun/Deferred（CB-STRIP-06 実機 lane・#114・#288 の
  責務）。
- Routing op（send/connection/sidechain/bus 編集）の候補
  受理: `Unsupported` が正直な verdict — 安全な confinement
  表現がないため後続 Routing 拡張 slice。
- scope entity removal（processor/target/lane/source）の
  候補編集: `Unsupported` — 後続 slice。
- Compare orchestration・semantic merge・blend: CB-08
  （#285）Deferred。
- engine の set 内 branch 追加 op（literal な同時 N>2
  candidate）: CB-03/04/05 と同じく後続 engine slice。

## CB-STRIP-06 実 Plugin 受入対応表（OS・Format 別 — 全行 NotRun）

| Format | OS | Plugin 例 | 判定 | 理由 / 後続 |
| --- | --- | --- | --- | --- |
| VST3 | Windows | 実 insert plugin | NotRun | 実 plugin host・実 OS 環境が必要（#114・#288） |
| VST3 | macOS | 実 insert plugin | NotRun | 同上 |
| VST3 | Linux | 実 insert plugin | NotRun | 同上 |
| CLAP | Windows | 実 insert plugin | NotRun | 同上 |
| CLAP | macOS | 実 insert plugin | NotRun | 同上 |
| CLAP | Linux | 実 insert plugin | NotRun | 同上 |
| AU | macOS | 実 insert plugin | NotRun | 同上（AU は macOS のみ） |
| 外部 Plugin GUI / Native Dock | Windows | #176 Dock gate | NotRun | GUI lane は #176・Floating/Generic 表示のみでは実 Dock Pass としない |
| PDC / RT bounded | Windows | 実 chain | NotRun | 実 RT pipeline・plugin latency 報告が必要 |
| PCM / A-B 比較 | Windows | 実 render | NotRun | 実 Render pipeline・比較 orchestration（#285/#288） |
