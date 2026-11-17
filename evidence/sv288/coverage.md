# CB-11 Creative Branch cross-gate headless coverage 棚卸し (Issue #288 / synvelis#288)

CB-11（7 領域 × PCM/RT/Project/GPUI/Plugin cross-gate 長期
tracker）の case-ID を、merge 済み CB-01..06 adapter・VCS-02
semantic diff・RCP-01..04 render-pin/recipe・既存 project/
player/render test へ写像した棚卸し結果。SF-06 先例
（`evidence/sv109/coverage.md`）と同じ Lane 語彙を使う。

凡例: ✅ headless 証拠あり（本 slice または既存 suite で
`mise run test` 実測 green）/ ◐ 部分的 / ⛔ 外部 Gate または
未 merge 前提（`external-gates.md`・所有 Issue 参照）

## 計測環境（2026-11-17・slice 2）

| 項目 | 値 |
| --- | --- |
| Synvelis SHA | `31e680aaa41083ea95848e4c65b382fb119e079b`（origin/main merge 済み base）+ 本 slice の commit（merge 後対象 SHA の CI 証拠は Coordinator 確認） |
| Ruviform resolved SHA | `84a68f25`（`Cargo.lock` git dep — slice 1 時点 `fd1d4d5f` から main 取り込みで更新） |
| OS / CPU / FS | Windows 11 Pro 10.0.26300 / Intel Core i9-9900K / NTFS（C:） |
| Device + Plugin | なし（headless — audio device・native plugin host 未接続） |
| Toolchain | Rust `1.99.1`（mise 管理。CI pin は `1.99.0`）、edition 2024 |
| Project schema | v14 envelope（`store::SCHEMA_VERSION = 14`） |

slice 1（2026-11-15・base `48f1e1e5`）の計測環境は
`verification.md` の履歴参照。slice 2 は CB-07（#284）・
CB-08（#285）・CB-10（#287）・RCP-07 headless（#296）の
merge 済み前提の上で runnable subset を拡張した。

## Lane 定義（sv109 と同一）

| Lane | 環境 | Pass にできるもの |
| --- | --- | --- |
| T0 headless (synvelis-project) | deterministic unit/integration test | ID/ref/schema/CAS/Undo/Diff・persisted 契約・scope/adapter 契約 |
| T2 headless E2E (synvelis-player/render) | lowering/offline render/probe integration | PCM・source map・fault recovery・offline export |
| T3 real OS GUI/Device | Windows/macOS/Linux + device/plugin | 実 Render/Focus/latency/plugin — **全て NotRun** |
| T4 both consumers | Synvelis + AudioLens resolved SHA matrix | public API/precision/RT/perf — **全て NotRun** |

状態語彙: `Pass`/`Fail`/`NotRun`/`Unsupported`（実行結果）と
`Required`/`Deferred`/`NotApplicable`/Blocked-by-issue
（要件・前提状態）を分離する（`POLICIES.md`）。

---

## Case → fixture → oracle → result

### CB-E2E-01（Audio Clip/Take/Comp・MIDI/Native Event の
Fork/Edit/Compare/Adopt/Undo/Save/Reload・永続 Source 不変）

| 範囲 | 被覆 fixture | oracle | 判定 |
| --- | --- | --- | --- |
| AudioClip/TakeUsage Project 軸 | `cb01_variant.rs`・`cb02_audio_take.rs`・**`cb11_cross_gate.rs`**（5 domain 同居 project での fork/edit/adopt/undo×5/redo×5/save/reload・single-CAS undo・raw immutability・provenance verbatim） | 独立 oracle（`Project` bitwise 復帰・`AdoptReceipt`・`BranchContentRef` digest） | ✅ T0 headless Pass |
| MIDI clip / Native Event | `cb03_midi_clip.rs`（SMF/division/tick fidelity・note diff・save/reload）・**`cb11_cross_gate.rs`**（MIDI note velocity adopt が他 domain と同一 CAS/undo/save 契約に乗る） | 独立 oracle + assessment | ✅ T0 headless Pass |
| Compare（semantic diff） | `vcs02_diff.rs`・**`cb11_e2e_04_semantic_diff_reports_scoped_changes_only`**（adopt materialize が scoped change のみ報告・raw-material kind 非出現） | `compare_snapshots` 独立 oracle | ✅ T0 headless Pass |
| 実 PCM/RT/GUI での同 lifecycle | — | 実信号・実 RT・実 window | ⛔ NotRun（`scarecrow173/ruviform#628`/`#631`・synvelis#114/#176） |

### CB-E2E-02（Instrument/FX/Track/Bus の plugin state・PDC・
sidechain/send rejected scope・1 DSP owner）

| 範囲 | 被覆 fixture | oracle | 判定 |
| --- | --- | --- | --- |
| Instrument slot（plugin state・shared MIDI・missing plugin 分岐） | `cb04_instrument_slot.rs`（`InstrumentStateAssessment`・per-candidate plugin honesty）・**`cb11_cross_gate.rs`**（InstrumentSlot adopt が他 domain と atomic に共存） | assessment + 独立 oracle | ✅ T0 headless Pass |
| FX range（境界・sidechain fail-closed） | `cb05_fx_range.rs`・**`cb11_cross_gate.rs`** | 独立 oracle | ✅ T0 headless Pass |
| Track/Bus strip（mask・routing 参照保護・plugin honesty） | `cb06_strip_fields.rs`・**`cb11_cross_gate.rs`** | 独立 oracle | ✅ T0 headless Pass |
| PDC・実 plugin state・sidechain 実信号 | — | 実 plugin ABI・実信号 | ⛔ NotRun（`scarecrow173/ruviform#630`・synvelis#176・`#628`） |

### CB-E2E-03（Arrangement 2+ Track / 4+ Clip の単一 Composite
Adopt + 1 Undo/Redo・Scope 外不変）

| 範囲 | 被覆 fixture | oracle | 判定 |
| --- | --- | --- | --- |
| Arrangement adapter・Composite CAS・1 undo/redo・scope 外不変 | **`cb11_e2e_03_arrangement_composite_adopt_undo_scope_isolation`**（`cb11_cross_gate.rs` — merge 済み `ArrangementAdapter` 経由。2 Track / 4 in-scope Clip + out-of-range/absolute-anchored witness・mute+move+note-remove composite → 単一 adopt=1 CAS=1 undo、undo/redo bitwise、scope 外 clip・他 4 domain・raw material 不変）・既存被覆 `cb07_arrangement.rs` | 独立 oracle（`Project` bitwise 復帰・`AdoptReceipt`・candidate payload 比較） | ✅ T0 headless Pass |
| gateway 経路の arrangement dispatch | `cb11_ai_vcs_off.rs::cb11_e2e_07_arrangement_scope_fails_closed_until_gateway_wiring` | typed `unsupported` fail-closed | ◐ 部分的 — project 軸は Pass。`VariantPorts::standard` が `ArrangementAdapter` を未配線のため gateway fork は fail-closed（src 変更を要する follow-up、下記「判明した gap」参照） |
| 実 GUI/実信号での arrangement composite | — | 実 window・実 PCM/RT | ⛔ NotRun（`scarecrow173/ruviform#631`/`#628`・synvelis#286） |

### CB-E2E-04（Shared Context change / Unaligned / Probe loss /
Graph stale / 非決定 plugin で差分証拠を誤表示しない）

| 範囲 | 被覆 fixture | oracle | 判定 |
| --- | --- | --- | --- |
| shared-context pin 分離（scope 編集 vs shared 編集） | `cb04_instrument_slot.rs`（shared MIDI stale）・**`cb11_x_rcp_01_render_pin_scope_vs_shared_context`**（`shared_context_digest` が全 branch で動き `scope_digest`/payload は不変） | `BranchRenderPin` 独立 oracle | ✅ T0 headless Pass（pin レベル） |
| unaligned comparison・stale derived・非決定 plugin・probe loss の orchestrated session | **`cb11_e2e_04_orchestrated_compare_stale_session_never_poses_as_current`**・**`cb11_e2e_04_orchestrated_compare_unaligned_probe_loss_provider_honesty`**（`ComparisonOrchestrator` session — scoped edit は自 session の Right-side dims + `ProjectRevision` のみ、sibling は `ProjectRevision` のみ、shared-context edit は全 session `SharedContext` stale。stale session は pin-scoped semantic evidence を保ち aux layer を stale flag。`Unaligned`→`AlignmentPolicy`、opaque plugin→`PluginStateUnknown`、非有限 metric→`EvidenceQuality`、probe loss→`Partial`/`Unknown`）・既存被覆 `cb08_compare_orchestrator.rs` | `StaleDimension`/`EvidenceReason`/`LayerState` 独立 oracle | ✅ T0 headless Pass（orchestration 契約レベル） |
| graph stale・非決定 plugin の実挙動 | — | 実 RT/plugin | ⛔ NotRun（外部 gate） |

### CB-E2E-05（Stale revision / ID collision / Missing Plugin /
QueueFull / Recording Active → 半端 Commit・録音劣化なし）

| 範囲 | 被覆 fixture | oracle | 判定 |
| --- | --- | --- | --- |
| stale scope/revision・missing plugin・recording-active の typed reject | `cb01`〜`cb06` 各 test（stale digest・`RecordingActive`・missing plugin・plugin state failure） | typed `VariantError` + `Project` 不変 assert | ✅ T0 headless Pass（domain 単位） |
| cross-domain 非汚染（ある domain の reject が他 domain を汚さない） | **`cb11_e2e_05_stale_reject_is_atomic_and_domain_isolated`** | `Project` bitwise + sibling candidate digest + 事後 adopt 成功 | ✅ T0 headless Pass |
| ID collision・QueueFull・実 RT publish 失敗 | — | 実 RT queue・id generator 実負荷 | ⛔ NotRun（外部 gate・`scarecrow173/ruviform#628`） |

### CB-E2E-06（GUI GPUI 実 OS / 実外部 Plugin Dock —
Headless と別記録・NotRun≠Pass）

| 範囲 | 被覆 fixture | oracle | 判定 |
| --- | --- | --- | --- |
| Difference Explorer UI・plugin dock | — | 実 window・実 plugin | ⛔ **Blocked: synvelis#286（CB-09）** + NotRun（`scarecrow173/ruviform#631`/`#630`・synvelis#114/#176）。CB-10（synvelis#287）は PR #483（`696efa73`）で解消済み |

### CB-E2E-07（AI/VCS OFF・通常 DAW Gate A の
Record/Playback/Mixer/Save/Export 非回帰）

| 範囲 | 被覆 fixture | oracle | 判定 |
| --- | --- | --- | --- |
| 既存 suite 非回帰（CB なし Gate A 相当を含む headless） | `mise run test synvelis-project`・`mise run test synvelis-gateway` 全 target green（本 slice で実測 — `verification.md`） | 既存 test suite | ✅ T0 headless Pass（headless レベルのみ） |
| AI OFF 条件の CB 統合（grantless agent fail-closed） | **`cb11_ai_vcs_off.rs::cb11_e2e_07_grantless_agent_ops_fail_closed_ai_off`**（agent principal の task/grant 未提示 variant op が prepare `Denied`("no applicable authorization")・proposal 非残存・apply `Denied`・human は同 proposal を apply/commit 可能）・既存被覆 `cb10_variant_actions.rs` | typed `GatewayError::Denied` + proposal store 独立 oracle | ✅ T0 headless Pass（headless 契約レベル） |
| VCS OFF での variant ops 独立動作 | **`cb11_ai_vcs_off.rs::cb11_e2e_07_vcs_off_variant_lifecycle_and_granted_agent`**（`VariantPorts::standard(None)` = in-memory store + `NoopRuntimePublish` + `SnapshotResolver` のみ — durable/VCS backend 非搭載で fork→edit→adopt(ApproveToApply+human receipt)→query→discard 完走） | `CommitGate` receipt + query response 独立 oracle | ✅ T0 headless Pass（headless 契約レベル） |
| 実機 Record/Playback/Mixer/Export | — | 実 device・実出力 | ⛔ NotRun（synvelis#23/#114 系・外部 gate） |

### CB-E2E-08（Ruviform candidate SHA に対し AudioLens
f64/長尺/CLI/解析非回帰 と Synvelis f32/RT を別 oracle で
検証・失敗時は導入拒否）

| 範囲 | 被覆 fixture | oracle | 判定 |
| --- | --- | --- | --- |
| consumer 側 precision/RT/perf regression | — | AudioLens + Synvelis resolved SHA matrix | ⛔ NotRun（`scarecrow173/ruviform#294`/`#308`/`#309`・synvelis#296）— consumer gate の責務 |

### CB-E2E-09（新 CI workflow なし・既存 mise/Daily/手動
実機の証拠を #23/#114 へ link）

| 範囲 | 被覆 fixture | oracle | 判定 |
| --- | --- | --- | --- |
| 新規 workflow 非追加・証拠 link | 本 slice は `.github/workflows/`・`mise.toml` を変更しない（diff で確認可能）。証拠 link は PR 本文の NotRun mapping | diff + ledger | ✅ T0（宣言レベル — 本 slice 範囲） |

### CB-X-RCP-01（各 Scope で Render Recipe 共用・変更失効・
Adopt/Undo・MissingPlugin 正負 fixture → RCP-GATE Case 紐付け）

| 範囲 | 被覆 fixture | oracle | 判定 |
| --- | --- | --- | --- |
| recipe 共有 identity（scope pin）・scope/shared context 分離・candidate payload digest verbatim | **`cb11_x_rcp_01_render_pin_scope_vs_shared_context`**・`cb06_strip_fields.rs`（render-pin identity 独立）・`cb03_midi_clip.rs`（same-context pin staleness） | `resolve_render_project`/`BranchRenderPin` 独立 oracle | ✅ T0 headless Pass（pin/identity レベル） |
| RCP-GATE Case 紐付け（merge 済み RCP-07 headless ledger） | `evidence/sv296/acceptance-ledger.md` の RCP-GATE-01..07 case-ID（`rcp07_gate0N_*` — 参照のみ、`crates/synvelis-render/tests/rcp07_gate.rs` は sv#518 占有で改変不可）。下記 mapping 表参照 | RCP-07 ledger case-ID | ✅ 紐付け実施（本 slice） |
| recipe cache/invalidation・MissingPlugin 正負の recipe 経路 | `synvelis-render/rcp01_recipe.rs`・`rcp02_artifact.rs`・`synvelis-player/rcp_04.rs`（merge 済み artifact 側）・RCP-07 `rcp07_gate01..07` | recipe/artifact oracle | ◐ 部分的 — CB scope × recipe identity は project 層 pin で被覆。cache/invalidation 本体・2人協業/非RT renderer/Undo/Reload 完全 fixture は RCP-07/#296 側所有（RCP-GATE-05 は同 ledger で Partial/Blocked 記録済み） |

#### CB-X-RCP-01 → RCP-GATE case-ID mapping（slice 2 で実施）

| CB-X-RCP-01 要求要素 | 対応 RCP-GATE case（`evidence/sv296/acceptance-ledger.md`） | 本 slice の被覆 |
| --- | --- | --- |
| 各 Scope で Render Recipe 共用（scope pin identity） | RCP-GATE-01（recipe key = cache identity・false-valid 0）・RCP-GATE-05（branch edit で key 分離・adopt pin） | `cb11_x_rcp_01_*` の `scope_digest`/`shared_context_digest`/payload digest pin + `rcp07_gate01_*`/`rcp07_gate05_*` 参照 |
| 変更失効（invalidation） | RCP-GATE-02（closure invalidation sound/complete）・RCP-GATE-07（stale R1→R3 誤 activation 0） | `cb11_x_rcp_01_*` の shared-context 変更で全 branch pin が stale 化する契約 + `rcp07_gate02_*`/`rcp07_gate07_*` 参照 |
| Adopt/Undo | RCP-GATE-05（adopt 単一 CAS・LiveBranch GC root） | `cb11_e2e_01_*`/`cb11_e2e_03_*` の adopt=1 CAS=1 undo + `cb01_variant.rs` adopt/fork 既存被覆 |
| MissingPlugin 正負 fixture | RCP-GATE-03（verified artifact fault 耐性）・RCP-GATE-06（store eligibility→resolver） | project 層は `cb04`/`cb11` の plugin honesty fixture、recipe 経路の正負は RCP-07 ledger 側所有（本 slice で再実装しない） |

RCP-GATE-04（OnDemand/Manual 認可）・RCP-GATE-06（5 mode
投影）は CB scope × recipe の直接対応が薄く、RCP-07 ledger
の case-ID 参照に留める。

---

## 領域別まとめ（7 domain × axis）

| Domain | Project 軸（CAS/Undo/Save/diff） | PCM 軸 | RT 軸 |
| --- | --- | --- | --- |
| Audio Clip/Take/Comp | ✅ cb02+cb11 | ⛔ 実 PCM NotRun（#628/#296） | ⛔ 実 RT NotRun |
| MIDI Clip | ✅ cb03+cb11 | ⛔ 実 MIDI device/PCM NotRun | ⛔ 実 RT NotRun |
| Instrument | ✅ cb04+cb11 | ⛔ 実 plugin PCM NotRun（#630） | ⛔ 実 RT/PDC NotRun |
| FX Range | ✅ cb05+cb11 | ⛔ 実 PCM NotRun | ⛔ 実 RT NotRun |
| Track/Bus Strip | ✅ cb06+cb11 | ⛔ 実 PCM NotRun | ⛔ 実 RT NotRun |
| Arrangement | ✅ cb07+cb11（project 軸）/ ◐ gateway dispatch は fail-closed（adapter 未配線 follow-up） | ⛔ 実 PCM NotRun（#628/#296） | ⛔ 実 RT NotRun |
| Orchestrated Compare | ✅ cb08+cb11（orchestration 契約） | ⛔ 実信号 evidence NotRun | ⛔ 実 RT NotRun |

## 本 slice の runnable subset（`cb11_cross_gate.rs`）

| test | pin する invariant | case-ID 対応 |
| --- | --- | --- |
| `cb11_e2e_01_fork_edit_isolation_across_five_domains` | 5 scope fork/edit が live raw・sibling candidate・scope digest を相互に汚染しない | CB-E2E-01 |
| `cb11_e2e_01_adopt_single_cas_undo_save_reload_across_domains` | adopt=1 CAS=1 undo、undo×5/redo×5 bitwise 復帰、save/reload provenance verbatim | CB-E2E-01 |
| `cb11_e2e_01_take_comp_adopt_undo_save_reload_raw_immutable` | TakeUsage comp adopt の CAS/undo/save と recording registry bitwise 不変 | CB-E2E-01 |
| `cb11_x_rcp_01_render_pin_scope_vs_shared_context` | scope/shared context digest 分離・candidate payload immutability | CB-X-RCP-01・CB-E2E-04(pin) |
| `cb11_e2e_04_semantic_diff_reports_scoped_changes_only` | adopt materialize が scoped semantic change のみ・raw kind 非出現 | CB-E2E-04(diff) |
| `cb11_e2e_05_stale_reject_is_atomic_and_domain_isolated` | stale reject の atomicity・cross-domain 非汚染 | CB-E2E-05(cross) |
| `cb11_e2e_03_arrangement_composite_adopt_undo_scope_isolation` | 2 Track/4 Clip composite adopt=1 CAS=1 undo、undo/redo bitwise、scope 外・他 domain・raw 不変 | CB-E2E-03 |
| `cb11_e2e_04_orchestrated_compare_stale_session_never_poses_as_current` | scoped edit の stale dims 分離（自 session Right-side+ProjectRevision、sibling ProjectRevision のみ）・shared-context 全 session stale・stale evidence が current を装わない | CB-E2E-04(orchestration) |
| `cb11_e2e_04_orchestrated_compare_unaligned_probe_loss_provider_honesty` | Unaligned 拒否・Mock origin・Approximate 非昇格・非有限→EvidenceQuality・opaque plugin→PluginStateUnknown・probe loss→Partial/Unknown | CB-E2E-04(orchestration) |

### gateway fixture（`crates/synvelis-gateway/tests/cb11_ai_vcs_off.rs`・slice 2 新規）

| test | pin する invariant | case-ID 対応 |
| --- | --- | --- |
| `cb11_e2e_07_grantless_agent_ops_fail_closed_ai_off` | agent principal の task/grant 未提示 op が prepare/apply 両段で `Denied` fail-closed・proposal 非残存・human 権限は非依存 | CB-E2E-07(AI OFF headless) |
| `cb11_e2e_07_vcs_off_variant_lifecycle_and_granted_agent` | durable/VCS backend 非搭載 rig で fork→edit→adopt(receipt)→query→discard 完走・granted agent は approval policy 通り動作 | CB-E2E-07(VCS OFF headless) |
| `cb11_e2e_07_arrangement_scope_fails_closed_until_gateway_wiring` | `VariantPorts::standard` 未配線の arrangement scope が `unsupported` fail-closed（src gap の正直な pin） | CB-E2E-03(gateway boundary) |

## 判明した gap（slice 2・src 変更を要する follow-up）

- `VariantPorts::standard`（`crates/synvelis-gateway/src/variant.rs`）
  が `ArrangementAdapter` を adapters map に登録していない
  — CB-07 merge（#482/#534）で `synvelis_project::variant`
  側の adapter は存在するが gateway 配線が未更新。
  fail-closed `unsupported` で安全側に倒れるため headless
  契約違反ではないが、CB-E2E-03 の gateway 経路 dispatch
  には src 変更（adapter 登録）が必要。本 slice は
  `crates/**/src/**` 編集禁止のため fixture で現状を pin
  し、follow-up として記録。
