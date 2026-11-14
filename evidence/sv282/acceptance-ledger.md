# Acceptance Ledger — synvelis#282 CB-05 FX Processing Range headless スライス

Date: 2026-11-14. Slice: CB-05 `VariantScope::ProcessingRange`
実ドメイン Adapter（`ProcessingRangeAdapter`）+ 安定
`ProcessorId`/`SemanticPortId` 境界・外部 routing fail-closed・
headless fixture/回帰テスト。残ドメイン Adapter（CB-06〜08 =
#283〜#285）・Compare Orchestrator（#285）・GPUI（#286）・
Action/Gateway 統合（#287）・Cross-Gate/実機受入（#288）は
DEFER。

対象 Issue: https://github.com/scarecrow173/synvelis/issues/282
（OPEN・`Refs` のみ・Close しない）。

## 計測環境

| 項目 | 値 |
| --- | --- |
| Synvelis SHA | scarecrow173/sv282-cb05 HEAD（ledger は同 commit 内） |
| OS / CPU | Windows 11 / Intel Core i9-9900K |
| Toolchain | mise 管理 Rust（`mise run` 経由） |
| Project schema | v14（schema 非変更 — 永続 model への entity 追加なし） |

## 判定語彙

- **Pass (headless fixture)**: `crates/synvelis-project/tests/
  cb05_fx_range.rs` が `mise run test synvelis-project`
  で green。
- **NotRun**: 実 OS/実 Plugin/実 RT/実 Render/実デバイス
  入力 pipeline 依存の判定 — 本スライスでは実行していない。
- **Deferred**: 後続 Issue の範囲として明示的に送った部分。
- **Unsupported**: project 表現または engine surface が存在
  せず、型付き拒否が正しい verdict のもの。

## 受入条件 → 証拠対応

| AC | 判定 | 証拠 |
| --- | --- | --- |
| CB-FX-RCP-01 | Pass (headless) | `cb_fx_rcp_01_render_identity_independent_captured_immutable` — captured payload digest が store key として live 共有編集後も不変（candidate bytes は fork 時点のまま decode 可能）。`BranchRenderPin` が scope/shared-context digest 合成で RCP cutset 非依存に branch identity を提供。capability 報告（`routing`/`mid_chain_end`=Unsupported、`fx_insert_inside`/`param_base`=Supported）が enforcement と一致。semantic diff が scoped 変更を `Modified`/`Added` で報告。比較 orchestration 実行本体・実 Render 再利用評価は RCP-01/`synvelis-render` 側・Deferred。 |
| CB-FX-01 | Pass (headless) | `cb_fx_01_entry_and_mid_boundary_fork_to_chain_end` — chain 入口（`pre`）→末尾と任意 processor 境界（`fx1`）→末尾の 2 scope がそれぞれ fork 可能（candidate は full project clone で chain 全体を保持）。`cb_fx_01_boundary_validation_fail_closed` — mid-chain end → `Unsupported`、reversed order → `Project(Validation)`、未知 `ProcessorId` → `NotFound`、foreign scope kind → `InvalidScope`。重複 range は `ScopeConflict`（`scopes_conflict`）。FX 番号・runtime NodeId/PortId は identity に不使用 — `ProcessorId`/`ChainId` のみ。 |
| CB-FX-02 | Pass (headless) | `cb_fx_02_shared_context_flows_b_edits_stay_in_range` — adopted(A)/candidate(B) の render pin が同一 `shared_context_digest` を共有し `scope_digest` のみ分岐。B の rich scoped 編集（param base・新規 target・automation lane+region・modulation source+assignment・範囲内 insert）が candidate のみへ適用され、live の control/routing/plugin_instances が serialize 比較で不変。共有 upstream（`pre`）の live 編集が両 branch の shared-context digest を同一に移動させ scope digest は不変 — 比較 context の stale 検出が正しく機能。 |
| CB-FX-03 | Pass (headless) | `cb_fx_03_stable_boundary_survives_fx_add_and_reorder` — range 外 head への FX 追加は scope digest 不変・shared-context のみ stale。`fx1..=fx4` の内側 reorder は scope identity（順序非依存 object 集合）を保持しつつ pin を stale にする。boundary 跨ぎ reorder は membership 変化として scoped digest が移動。`cb_fx_03_boundary_deletion_is_unresolved_or_stale` — start boundary 削除で `scope_digest` が `NotFound`（Unresolved、silent re-anchor なし）、非境界 member 削除は live scope digest 移動 → pin 済み intent の adopt が `StaleScope` で拒否・live 不変。削除された processor の暗黙再接続なし。 |
| CB-FX-04 | Pass (headless) | `cb_fx_04_boundary_dependency_detected_and_shared` — key strip → `fx2` sidechain input の feed が `boundary_dependencies` にちょうど 1 件・`source_in_scope=false` で検出され scope objects に含まれない（共有 context）。`SetConnectionEnabled` での無効化も依存として残り shared-context digest が移動。`cb_fx_04_external_wiring_ops_reject_typed` — 境界 connection への `RemoveConnection`/`SetConnectionEnabled`/`SetConnectionChannelMap`、新規 `AddConnection`（sidechain/main）・`AddSend`、foreign chain の `ReorderProcessor`/`RemoveProcessor`/`InsertProcessor`、`SetStripMute` が全て `Unsupported`/`ScopeViolation`。upstream `pre` の control op が `ScopeViolation`、scope entity removal が `Unsupported`。adopt は `routing.connections` を一切変更しない（implicit double-mix なし）。zero-delay cycle 検証は routing cycle oracle 側の既存契約 — candidate は cycle を作る op を持たないため構造的に発生しない。 |
| CB-FX-05 | Pass (headless) | `cb_fx_05_adopt_range_only_one_cas_undo_save_reload` — adopt が宣言 range の control state + 範囲内 insert のみを materialize（live revision +1・undo_steps=1 の 1 CAS）。connections/strips/sends/`pre` processor は serialize 比較で不変。Undo で採用前復元・Redo で再適用、adopted branch は captured content へ freeze。save→load で `Project` 全体（variant_sets 含む）が round-trip。`cb_fx_05_stale_scope_digest_rejects_adopt` — pin 後の live scoped 編集で `StaleScope` 拒否・live 不変。`cb_fx_05_adopt_while_recording_rejects_typed` — owning lane の recording Open で `RecordingActive` 先行拒否。`cb_fx_05_runtime_publish_failure_keeps_project_committed` — `FakeRuntimePublish` の `Failed` が project commit を rollback せず別 receipt に記録・undo は通常動作。 |
| CB-FX-06 | Pass (headless) | `cb_fx_06_plugin_instances_independent_state_assessment` — candidate が fork 時点の `plugin_instances` identity を独立保持し、範囲内 insert が candidate のみへ新 instance を mint（live 非波及）。`assess_range_state` が range 全 processor（insert 含む 4 件）を chain 順に評価し `Coordinated`/`BestEffort`/`Failed{Timeout}` を verbatim 記録 — `BestEffort` は `exact_eligible()=false` で昇格なし、失敗は型のまま。`NoPlugins` availability で adopt が `MissingPlugin` fail-closed（live 不変）、`YesPlugins` で `Applied` — project-truth edit/adopt は runtime availability を越えて進行。実 plugin instance 独立性・PDC/RT bounded・PCM 比較は fake vs real-plugin の別 gate — 実機 lane は全行 NotRun（偽装昇格なし）。 |

## 補足: scope validation 回帰

`cb_fx_01_boundary_validation_fail_closed` —
`InstrumentSlot` scope を `ProcessingRangeAdapter` に
適用すると `InvalidScope`（adapter の scope-kind gate）。
`cb04_instrument_slot.rs` 全件 green — adapter.rs の
processor-control checker 共有化による CB-04 挙動変更
なし。project oracle は declared scope が参照する
boundary `ProcessorId` の削除を live op 段階で拒否する
（`NotFound`）— scope 宣言済み entity の保護は engine
既存契約として fail-closed に機能。

## NotRun / Deferred 一覧

- 実 OS provider・実 Plugin host（実 VST3/CLAP/AU
  instance）・外部 Plugin editor/Native Dock GUI・実 RT・
  実 Render pipeline・実デバイス入力 pipeline・workspace/
  GPUI projection: NotRun（headless fixture のみ。
  Fake/Domain Oracle の PASS を実機 PASS に昇格しない）。
- 実 plugin の instance 独立性・PDC/RT bounded 動作・
  PCM/A-B 比較・opaque state の実 capture/restore 品質:
  NotRun/Deferred（CB-FX-06 実機 lane・#114・#288 の責務）。
- 任意 mid-chain `end` の受理: Issue が初期受入不要と
  規定 — schema 表現は残し adapter が `Unsupported`。
  受理には境界跨ぎ main-path connection の confinement
  設計が前提の別 slice。
- `RemoveProcessor`/`ReorderProcessor`/send・sidechain・
  multi-output wiring の候補編集: `Unsupported` が正直な
  verdict — 安全な confinement 表現がないため後続 slice。
- Compare orchestration・semantic merge・blend: CB-08
  （#285）Deferred。
- engine の set 内 branch 追加 op（literal な同時 N>2
  candidate）: CB-03/04 と同じく後続 engine slice。

## CB-FX-06 実 Plugin 受入対応表（OS・Format 別 — 全行 NotRun）

| Format | OS | Plugin 例 | 判定 | 理由 / 後続 |
| --- | --- | --- | --- | --- |
| VST3 | Windows | 実 FX plugin | NotRun | 実 plugin host・実 OS 環境が必要（#114・#288） |
| VST3 | macOS | 実 FX plugin | NotRun | 同上 |
| VST3 | Linux | 実 FX plugin | NotRun | 同上 |
| CLAP | Windows | 実 FX plugin | NotRun | 同上 |
| CLAP | macOS | 実 FX plugin | NotRun | 同上 |
| CLAP | Linux | 実 FX plugin | NotRun | 同上 |
| AU | macOS | 実 FX plugin | NotRun | 同上（AU は macOS のみ） |
| 外部 Plugin GUI / Native Dock | Windows | #176 Dock gate | NotRun | GUI lane は #176・Floating/Generic 表示のみでは実 Dock Pass としない |
| PDC / RT bounded | Windows | 実 chain | NotRun | 実 RT pipeline・plugin latency 報告が必要 |
| PCM / A-B 比較 | Windows | 実 render | NotRun | 実 Render pipeline・比較 orchestration（#285/#288） |
