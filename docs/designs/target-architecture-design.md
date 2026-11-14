---
id: "DESIGN-s3oIq2dqNieQuUHD6G89a7"
type: "design"
status: "approved"
title: "Target Architecture 設計レビュー — TA-001〜012 契約マップ・Owner 境界・DAG/型漏出/GUI 証拠評価"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "docs/specs/target-architecture.md"
  changes:
    added:
      - type: "file"
        path: "docs/designs/target-architecture-design.md"
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/target-architecture.md"
  implemented-by: []
  depends-on:
    - "docs/specs/target-architecture.md"
    - "docs/designs/synvelis-driven-ruviform-target-architecture-v07.md"
    - "docs/designs/playback-mixer-generic-execution-graph-architecture.md"
    - "docs/designs/playback-graph-design.md"
    - "docs/designs/configurable-ppqn-musical-time-architecture.md"
    - "docs/designs/musical-time-design.md"
    - "docs/designs/project-asset-persistence-architecture.md"
    - "docs/designs/time-transport-midi-automation-plugin-integration-architecture.md"
    - "docs/designs/media-import-export-offline-render-ui-diagnostics-architecture.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/plans/target-architecture.md"
    - "docs/tasks/target-architecture-task.md"
    - "docs/test-specs/target-architecture.md"
    - "docs/impl/ir/target-architecture.md"
    - "docs/impl/exp/target-architecture.md"
    - "evidence/sv22/case-ledger.md"
    - "evidence/sv22/dependency-dag.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/synvelis-driven-ruviform-target-architecture-v07.md"
    - "docs/specs/target-architecture.md"
  derived-by:
    - "docs/plans/target-architecture.md"
    - "docs/test-specs/target-architecture.md"
  verifies: []
  verified-by:
    - "docs/test-specs/target-architecture.md"
  references:
    - "evidence/sv22/case-ledger.md"
    - "evidence/sv22/dependency-dag.md"
    - "docs/adr/sv175-native-shell-driver-neutral-gpui.md"
    - "docs/adr/sv-gui-01-app-crate-webview-feature-vendored-ui-pipeline.md"
    - "docs/specs/sv-gui-04-02-close-veto.md"
    - "docs/specs/sv-gpui-01-native-app-shell.md"
    - "docs/specs/sv-gui-03-05-gpui-scenario.md"
  defers: []
  deferred-by: []
---

# Target Architecture 設計レビュー — TA-001〜012 契約マップ・Owner 境界・DAG/型漏出/GUI 証拠評価

## 概要

本書は spec `docs/specs/target-architecture.md`（synvelis#22）の
設計レビュー正本である。承認済み Target Architecture v0.7.1
（以下 v0.7.1）の TA-001〜012 受入条件と GUI 採用方針 2 条件を、
**現行コード（Synvelis workspace @ branch HEAD・Ruviform resolved
rev `fd1d4d5f4de57092b96ad1cd309b85fe47cbef38`）と突合** し、
各条件に対する契約・fixture・Issue Owner・証拠分類を確定する。

これは実装レビューではなく**設計レビューと証拠台帳の正本化**で
ある。コード変更は行わず、実装ギャップは各 Owner Issue へ委譲
する。証拠の実行結果は `evidence/sv22/case-ledger.md`、
依存 DAG の全辺は `evidence/sv22/dependency-dag.md` を正本とする。

### レビュー対象の固定

| 対象 | 値 |
| --- | --- |
| Synvelis branch | `scarecrow173/sv22-target-architecture`（worktree HEAD、PR 前 merge 時点で再固定） |
| Ruviform resolved rev | `fd1d4d5f4de57092b96ad1cd309b85fe47cbef38`（`Cargo.lock` resolved、全 ruviform git dep が単一 SHA に一致 — `check-ruviform-rev` Pass） |
| Ruviform checkout | `~/.cargo/git/checkouts/ruviform-8f0ffdb08acc57df/fd1d4d5`（cargo metadata 解決済みソース） |
| 検証 OS/toolchain | Windows（Git Bash）、Rust toolchain は mise 管理（MSRV 1.89 / CI pin 1.99.0） |

## 1. Owner/境界契約の現行整合評価

v0.7.1 の責務分担を現行 crate 構成に写像した結果:

| 層 | Owner | 現行 crate/実体 | 整合評価 |
| --- | --- | --- | --- |
| DAW Project/Asset/Source/Track/Lane/Clip/Take/Comp | Synvelis | `synvelis-project`（Ruviform 依存: audio-core/graph/io-file/media-core/plugin-core） | 整合 — Ruviform に DAW 型なし（§3） |
| Typed Graph IR/Validator/Compiler/Runtime | Ruviform | `ruviform-graph`・`ruviform-audio::typed`・`ruviform-audio-plugin-bridge` | 整合 — 通常依存は audio-core のみ（§4） |
| Media Time/MIDI/Parameter/Event | Ruviform | `ruviform-media-core`（music/transport_schedule）、`ruviform-music` | 整合 — Synvelis は consumer として import |
| Plugin Host/Adapter/format | Ruviform | `ruviform-plugin-host`・`ruviform-plugin-core`・`ruviform-plugin-{clap,vst3,ofx,export}`・`ruviform-audio-plugin-bridge` | 整合 — §5 の境界確認 |
| Generic Media Read/Write・Offline Job | Ruviform | `ruviform-io-{file,wav,...}`・`ruviform-media-*`・`ruviform-exec` | 整合 — §8 TA-010/TA-011 |
| GUI Driver-neutral/Observability/Gateway | Ruviform | `ruviform-ui`・`ruviform-ui-{gpui,webview,theme,test}`・`ruviform-observability`・`ruviform-gateway` | 整合 — §9 GUI 方針 |
| Playback/Mixer/Render consumer | Synvelis | `synvelis-player`・`synvelis-render`・`synvelis-recording` | 整合 — DAG §2 |
| Recording UX/Take/Comp product semantics | Synvelis | `synvelis-recording`・`synvelis-project` | 整合 — sv17 chain |
| f64 解析/PTS/CLI/Job | AudioLens | 別 repo `F:/repositry/audio-products/audio-lens` | 本 slice 範囲外 — #219/#294 で接続 |

### 1.1 v0.7.1 記述と現行コードの差異（観測事項）

- **crate 数の変化**: v0.7.1（2026-11-01 時点）は「現行 28
  crate」と記述するが、resolved rev `fd1d4d5` の Ruviform
  workspace は **42 crate** に増加している（M1〜M7 実装で
  `ruviform-music`・`ruviform-ui-{gpui,webview,theme,test}`・
  `ruviform-plugin-export`・`ruviform-poc-native-dock` 等が
  追加）。これは設計の否定ではなく**文書ドリフト**であり、
  v0.7.1 の責務分担自体は現行構造と整合する。v0.7.1 の
  更新は Ruviform #193 の migration snapshot 管理で扱う。
- **plugin-host → audio の向き**: v0.7.1 §2 の訂正済み事実
  （`ruviform-audio` は plugin-host の dev-dep のみ）を
  resolved rev の `crates/ruviform-plugin-host/Cargo.toml`
  で再確認した。通常依存は `ruviform-audio-core`・
  `ruviform-media-core`・`ruviform-plugin-core`・
  `ruviform-observability`・`ruviform-rt`・
  `ruviform-video-core`・platform support・`shared_memory`。
  `tests/plugin_graph_e2e.rs` の AC3 コメントが
  「endpoint→node→Graph 接着は host 側からのみ試験する」
  契約を明示しており、依存方向は設計どおり。

## 2. Production Dependency DAG（TA-001/SV22-C01）

### 2.1 実行した検査

`cargo metadata --format-version 1 --locked` の resolve
グラフを対象に、**通常依存（dev/build 以外）辺のみ**で DFS
cycle 検査を実施した（コマンド・生データは
`evidence/sv22/dependency-dag.md`）。

| 検査対象 | 範囲 | 結果 |
| --- | --- | --- |
| Synvelis workspace 通常依存 | 12 member crate + resolved ruviform 依存 | **循環なし** |
| Ruviform workspace 全体（fd1d4d5 checkout で metadata 再取得） | 42 crate | **循環なし** |

- dev-dependency・build-dependency・feature/optional・
  target-specific の各辺は `dependency-dag.md` で分類済み。
  Cargo が dev-dep 循環を許容するため、plugin-host→audio
  の dev-dep 辺は「通常依存の非循環」判定に影響しない。
- `synvelis-*` → `ruviform-*` は一方向であり、Ruviform の
  いずれの crate も Synvelis crate に依存しない（resolve
  グラフに synvelis package node が存在しない）。

### 2.2 Ruviform→Consumer 方向の独立性（TA-002 の DAG 側）

`ruviform-graph`・`ruviform-media-core`・`ruviform-rt`・
`ruviform-exec`・`ruviform-io-*` の通常依存集合に Consumer
（synvelis/audio-lens）crate は含まれない。Consumer 互換は
Ruviform #219/#294 の独立 Oracle 責務であり、本 slice は
Synvelis 側の一方向依存のみを検査する。

## 3. 型漏出禁止（TA-002/SV22-C02）

resolved rev `fd1d4d5` の Ruviform `src/` 全域を
`synvelis` で走査した結果:

- **DAW 意味論型の侵入なし**: `synvelis` crate への依存・
  `Project`/`Track`/`Take`/`Comp`/`Automation`/`Undo`/
  `RenderRecipe` 等の Synvelis 専用型名は Ruviform 公開 API
  に存在しない。
- **観測された出現（DAW 型ではない）**:
  - `crates/ruviform-ui-theme/src/generated.rs` に
    `pub mod synvelis_dark`（consumer 名を冠した theme
    fixture データ — design token 値であり DAW 意味論ではない）。
  - `crates/ruviform-ui/src/headless.rs` 等の doc 例示に
    `synvelis::...` パス言及（doc comment）。
- **評価**: TA-002 の受入条件（「Synvelis 専用
  Project/Track/Take 等の型が侵入しない」）は**充足**。
  consumer 名 theme fixture は共通契約の汚染ではないが、
  「consumer 名を Ruviform ソースに持つ」点は Ruviform Owner
  （#193/#219 系）が棚卸しする観測事項として P2 に記録する
  （本 slice で修正しない）。
- **独立利用の型検査**: 共通契約（graph/media-core/rt/
  exec/io）の通常依存は他 Consumer・上位 engine に依存せず、
  AudioLens が同じ契約を独立に利用できる構造。Synvelis 側は
  `synvelis-project` が `ruviform-plugin-core`・
  `ruviform-media-core` を直接 import しており、DAW 型を
  Ruviform 側に置かず consumer 側で組み立てている。

## 4. Mock/Headless Graph 検証（TA-003/SV22-C03）

- `ruviform-graph` の通常依存は **`ruviform-audio-core` のみ**
  （OS device・実 plugin host・上位 engine への依存なし）。
  `src/` は `graph.rs`/`ids.rs`/`lib.rs`/`mapping.rs`/
  `port.rs`/`project.rs`/`validate.rs` で構成され、device/host
  を import しない。typed compiler/runtime は上位の
  `ruviform-audio/src/typed/`（`compile.rs`/`runtime.rs` 等）
  と `ruviform-audio/tests/typed_*.rs` が担う。
- fixture `tests/poc_ex.rs`（`poc_ex_001`〜`006`）は OS
  Device・実 Plugin Host を起動せずに Graph 構築・検証・
  失敗（`poc_ex_006_failure_preserves_published_and_main_only_golden`）・
  revision mapping（`poc_ex_005`）を検証する。
- **証拠分類**: fixture 存在はソース pin で確認（Pass=
  契約実在）。fixture の実走は本 slice では未実施のため
  NotRun とし、先行 slice/CI の記録を SHA 付きで参照する。

## 5. Typed Fan-in/Fan-out・Sidechain・PDC・無効拒否（TA-004/SV22-C04）

`ruviform-graph` の port/validate 契約と `ruviform-audio`
typed compiler（`src/typed/compile.rs`・
`tests/typed_compiler.rs`）の fixtures に以下が実在する:

| 契約要素 | 実体 |
| --- | --- |
| Fan-in（merge-point scratch slot） | `g2_01_fan_in_gets_scratch_slot`（typed compiler fixture） |
| Channel map 結合 | `g2_02_channel_mapped_edge_binds_map` |
| 宣言 delay の拒否 | `g2_02_declared_edge_delay_rejected` |
| PDC（latency 差の merge 点補償） | `g2_03_pdc_compensates_latency_difference_at_merge` |
| Sidechain port role | `PortRole::Sidechain`（`port.rs` — role は方向・DAW 意味論を持たない generic 表現）+ `ruviform-audio-plugin-bridge` の `BusRole::Sidechain`/`aux_sidechain` capability fixture |
| 無効 Port/Channel 拒否 | `ValidationError`/`ValidationErrors` の typed variants（`InvalidDirection`/`DomainMismatch`/`FormatMismatch`/`ImplicitConversionRequired`/`InvalidChannelMap`/`MissingNode`/`MissingPort`/`PortNotOnNode`/Multiplicity/Cycle 系） |

評価: TA-004 の要素は**契約・fixture として実在**し、
v0.7.1 の「既存 Ruviform Audio Graph は Pull-ordered であり
typed multi-port graph・PDC compiler は将来/additional 要件」
との関係を明確化する — **typed multi-port graph + PDC は
実装済み fixture を持つ**（`ruviform-graph` crate +
`ruviform-audio::typed`）。v0.7.1 の Pull-ordered 記述は
旧 `ruviform-audio` の graph であり、typed 系は別層として
存在する。この区別を台帳に固定する。

## 6. PluginHost↔AudioEngine 境界（TA-005/SV22-C05）

cargo metadata の resolve グラフで確認:

- `ruviform-plugin-host` → `ruviform-audio`: **通常依存なし**
  （dev-dep のみ。`tests/plugin_graph_e2e.rs` が AC3 契約を
  明示）。
- `ruviform-audio` → `ruviform-plugin-host` /
  `ruviform-audio-plugin-bridge`: **依存なし**。
- `ruviform-audio-plugin-bridge` → `ruviform-audio` +
  `ruviform-plugin-host`: 正方向（bridge が接着層）。
- 結論: Host→Engine・Engine→Host の通常依存循環は存在せず、
  TA-005 の「不要な通常依存がない」は**充足**。接着は
  bridge crate に隔離されている。

## 7. 単一 RT Owner（TA-006/SV22-C06）

- `ruviform-audio::typed::runtime`（`crates/ruviform-audio/src/typed/runtime.rs`）:
  generation-tagged SPSC command channel・`active_generation`
  AtomicU64・install/remove は plan generation 一致時のみ
  適用・`g2_04_rt_runtime_is_send_not_sync`（`Send` 非
  `Sync` で single-owner を型で強制）。
- `typed_activation.rs` fixture: `g2_04_stable_slot_preserves_state`・
  `g2_04_plan_owned_replaces_state`・
  `g2_04_install_remove_order_and_deferred_destruction`・
  `g2_05_*` retirement 系（deferred destruction・collector
  drain 観測・stale generation event 拒否）。
- 評価: Stateful Processor/Graph の RT 実行 Owner は
  runtime が単一所有し、旧/新 Plan の二重 Process・二重
  所有は generation 契約と fixture で拒否される構造。
  **契約実在 = Pass（pin）、実走 = NotRun（先行参照）**。

## 8. RT 禁止事項（TA-007/SV22-C07）

- `rt_*` fixture 群（ruviform: `rt_typed_process_no_alloc`・
  `rt_run_callback_no_alloc`・`rt_device_restart_no_alloc`・
  `rt_queries_no_alloc`、media-core `transport_schedule`/
  `music` 内、Synvelis 側各 crate の `rt_*` suite —
  `mise run test-rt` = `cargo test --workspace --locked rt_`）
  と `assert_no_alloc` 計測 dev-dep が存在する。
- `ruviform-rt` の `retire.rs`/`spsc.rs`/`alloc_counter.rs`・
  `g2_05_retire_queue_saturation_flagged_leak_safe` は
  RT callback 内の危険 Drop を deferred destruction +
  collector drain で回避する設計を示す。
- **検証不能経路を PASS にしない原則**: 本 slice は fixture
  存在を pin するのみで、alloc/block の実測は先行 slice/CI
  の記録を SHA・環境付きで参照する。本 slice で実走して
  いない項目は `NotRun`。Mock/Headless の成功を実 device
  受入と混同しない。

## 9. Generation/時点契約（TA-008/SV22-C08）

`ruviform-media-core::transport_schedule` が以下の型付き
ID/receipt を提供する（resolved rev で実在確認）:

- Identity: `ClockEpochId`・`TransportGeneration`・
  `PlanGeneration`・`TempoMapRevision`・`LoopOccurrence`・
  `CommandId`
- Frame: `EngineFrame`（device/sample frame の区別）
- Receipt/状態: `AppliedReceipt`・`ApplyStatus`・
  `RejectReason`・`ApplyDegradation`・`DeliveryPolicy`・
  `ScheduleBudget`・`InBlockTransportSupport`
- fixture: `play_then_seek_receipt_lifecycle`・
  `stale_transport_generation_event_is_faulted_not_delivered`・
  `device_restart_vs_seek_distinct_epoch_generation`・
  `install_rejects_same_generation_different_plan`・
  `loop_wrap_splits_at_end`・`loop_events_fire_once_per_occurrence`・
  `same_frame_events_keep_source_order`・
  `min_frames_violation_exact_wrap_undeliverable`・
  `min_frames_violation_degraded_defers_first_wrap_then_faults`・
  no-allocation callback。

`ruviform-graph::GraphProject`（`src/project.rs`）は
requested vs published revision を区別し、
`ruviform-audio::typed::runtime` は plan generation の SPSC
適用で Commit/Applied の時点を型で分離する。

評価: Plan/Target/Event の Generation と
Commit/Applied/Observed の時点は**型付き契約として実在**。
Synvelis 側は sv18（playback-graph）・sv19（musical-time）
chain が consumer 契約を被覆する。

## 10. Musical Time → Sample Event（TA-009/SV22-C09）

- `ruviform-media-core::music`: `Ppqn`（validated
  `1..=u16::MAX`）・`PPQN=960` 互換定数・`BeatPosition`
  （正規化有理数・負 preroll 対応・明示 overflow error）・
  `TickPosition`（resolution 結合）・`TempoMap`。
- fixture: `variable_ppqn_media_roundtrip`・
  `beat_position_normalizes`・`beat_position_arithmetic`・
  `ppqn_validates`・`tick_position_binds_resolution`・
  `tick_position_projection`・`beat_position_off_grid`・
  `conversion_overflow_is_explicit`・`preroll_negative_position`・
  `ppqn_960_golden_compat`・`rt_queries_no_alloc`。
- `transport_schedule` の span scheduling が Loop/Seek を
  `LoopOccurrence`・epoch/generation で区別し、量子化誤差は
  `Error::TimeOverflow`/明示 policy で顕在化する。
- **Synvelis 側の差異観測**: `synvelis-project` の PPQN
  validation は `1..=32767`（SMF 15-bit 準拠）で、Ruviform の
  `1..=u16::MAX`（65535）の**部分集合**。consumer 側がより
  厳しい制約を課す方向は整合的であり、矛盾ではない（P3
  記録）。
- Synvelis consumer 契約は sv19 chain（`tim_01_ppqn`・
  `tim_02_smf`・`tim02_midi_playback` fixture）が被覆。

## 11. WAV 実 Encode/Finalize/Reopen（TA-010/SV22-C10）

`ruviform-io-wav` の fixture が以下をカバーする:

- **roundtrip（Encode→Decode 同一 reader 系）**:
  `roundtrip_stereo_s16_48k`・`roundtrip_mono_s24_44k1`・
  `roundtrip_five_point_one_f32_96k_extensible`・
  `roundtrip_stereo_s32_48k`・`roundtrip_short_file`・
  `zero_frame_file_reopens_empty`・`nan_inf_and_clip_values_follow_declared_policy`・
  `drain_then_receive_then_none`・`send_after_drain_is_invalid_data`。
- **Finalize/Reopen/公開契約（contract.rs）**:
  `staged_seal_readback_publish_e2e`（seal→readback→publish）・
  `sink_error_at_finalize_is_io_not_completion`・
  `write_after_finish_and_double_finish_error`・
  `staged_abort_preserves_existing_destination`・
  `staged_drop_after_write_preserves_destination`・
  overwrite conflict・disk-full protection。

評価: 「実 WAV 出力を Encode→Finalize→Reopen→別 Reader
Decode で検証」は `roundtrip_*` + `staged_seal_readback_publish_e2e`
で契約実在。**Drop 時の best-effort（`staged_drop_*`）は
「既存 destination を保全する」契約であり、完了証拠ではない**
— Finalize の明示的成功と Reopen 検証が完了条件、という
v0.7.1/Issue の要求と一致する。

## 12. Window Close ≠ Job Cancel（TA-011/SV22-C11）

### 12.1 Ruviform 側（driver-neutral multi-window）

`crates/ruviform-ui/tests/multi_window.rs`・`runtime.rs`:

- `close_requested_does_not_cancel_jobs`・
  `subwindow_close_leaves_jobs_and_loop_running`・
  `job_events_route_to_owner_window` — Window Close は
  Window 所有 Job のみを対象とし、Project/Application 所有
  Job を暗黙 Cancel しない。
- `UiRuntime::cancel_window_jobs(window)` は **明示 opt-in**
  であり自動呼出ではない（`driver.rs`/`runtime.rs` の
  WindowLifecycle::CloseRequested/Destroyed は application
  teardown を自動定義しない）。
- `close_request_can_be_vetoed`・`destroy_is_distinct_from_close_request`・
  `deferred_close_stays_alive_then_destroyed` が Close/Destroy
  の区別を固定する。

### 12.2 Synvelis 側

- `crates/synvelis-app/tests/close_veto.rs`: `syn.window.close_guard`
  decision query・dirty last-window veto・clean approve・
  non-last-window・partial quit veto・
  `tests/two_window.rs` の `both_windows_share_one_project_session`。
- `docs/specs/sv-gui-04-02-close-veto.md`: app-global
  Project/Recording guard は last live window のみ veto、
  quit は全 window close、**Project/Session/Graph/Recording
  の暗黙 Cancel なし**。
- `crates/synvelis-app/src/shell.rs`: `Destroyed` →
  `cancel_window_jobs` の**明示呼出**（Window 所有 Job の
  scoped cleanup）。

### 12.3 legacy 経路の区別

`ruviform-ui` の legacy 単一 window 経路は
`HostEvent::CloseRequested` → `close_requested_cancels_jobs_and_emits_lifecycle`
で close=アプリ終了の意味論を持つ。これは multi-window
driver 経路（CloseRequested は veto/defer 可能・Job 非連動）
とは**別契約**であり、混同すると TA-011 を誤判定する。
本レビューは両経路を区別して記録する。

評価: GUI（multi-window driver 経路 + synvelis close_veto）
と Headless（`ruviform-ui-test` FakeHost）の両側で契約が
実在。実走は NotRun（先行 slice/CI 参照）。

## 13. Consumer 互換（TA-012/SV22-C12）

各 Consumer を別責務・別回帰 Suite・別 NotRun 根拠で記録する
（詳細は `evidence/sv22/case-ledger.md` の C12 表）:

| Consumer | Ruviform 側実体 | 回帰 Suite | 接続 Issue |
| --- | --- | --- | --- |
| Audio | `ruviform-audio`・`ruviform-rt`・`ruviform-audio-core`・device adapter | `rt_*`・graph/audio tests | #184〜#191 |
| Video | `ruviform-video*`・`ruviform-video-core` | video crates tests | #187 系 |
| Plugin | `ruviform-plugin-{core,host,clap,vst3,ofx,export}`・`ruviform-audio-plugin-bridge` | plugin_*・plugin_graph_e2e | #188〜#191 |
| CLI | `ruviform-cli-sample` | cli-sample tests | — |
| UI | `ruviform-ui`・`ruviform-ui-{gpui,webview,theme,test}`・`ruviform-viz`・`ruviform-poc-native-dock` | ui tests・gpui-test | #384〜#390 |
| NLE/edit | `ruviform-edit` | edit tests | — |
| Synvelis(DAW) | `synvelis-*` 全 crate | `mise run test <pkg>` | #17〜#23 |
| AudioLens | 別 repo | 別 repo CI | #219/#294（独立 Oracle + 3repo SHA tuple） |

`check-headless-deps`（10 headless crate に GPUI/WebView/
wgpu/winit/tao/wry・Ruviform GUI adapter 非含有を機械検査）
が **Pass**。`synvelis-app` は GUI 依存を feature
（`webview`/`native`）で分離するため対象外 — これは
意図的設計であり、headless 契約の漏出はない。

## 14. GUI 採用方針（SV22-C13）

### 14.1 文書照合

| 文書 | status | 役割 |
| --- | --- | --- |
| `docs/adr/sv175-native-shell-driver-neutral-gpui.md`（Synvelis） | **accepted** | #175 後継 ADR — Driver-neutral shell + GPUI 標準 Driver。`supersedes` で sv-gui-01 を明示 |
| `docs/adr/sv-gui-01-app-crate-webview-feature-vendored-ui-pipeline.md` | **superseded** | 旧 WebView/SolidJS 決定 — 歴史保持・再承認しない |
| Ruviform `docs/adr/driver-neutral-gui-gpui-driver-webview-first-issue-384.md` | **proposed（仮決定）** | #384 — Driver-neutral 契約層 + GPUI 標準 Driver。**最終採用は #262/#388/#389 実機証拠後** |
| Ruviform `docs/adr/plugin-gui-embedding.md` | accepted（WebView 時代） | wry `build_as_child` の plugin GUI embed — #389 で再設計予定の旧経路 |

整合評価: Synvelis #175（accepted）と Ruviform #384
（proposed 仮決定）は**方向が一致**する（Driver-neutral
契約層を正本・GPUI を標準 Driver・Consumer 通常画面に
GPUI 型を出さない）。#384 が proposed である点は
「GPUI 最終採用を本レビューで PASS 宣言しない」根拠と
整合する — 本レビューは**採用方針の整合を確認**し、
最終採否は #262/#388/#389 に委譲する。

### 14.2 GPUI 型の非露出

- `synvelis-app` の `native` feature のみが
  `ruviform-ui-gpui`・`synvelis-theme` を有効化し、
  GPUI 進入は composition root（app 起動部）に限定。
- `check-headless-deps` が 10 headless crate
  （synvelis-project/library/recording/player/render/
  workspace/gateway・mcp-poc・recording-poc）に
  GPUI/WebView/wgpu/winit/tao/wry・Ruviform GUI adapter
  が含まれないことを機械検査 → **Pass**。
- `sv-gui-03-05-gpui-scenario` は GPUI-simulated scenario
  lane であり、実 OS window/pixel の証拠ではない
  （POLICIES GUI-L2 と区別）。

## 15. Native Plugin GUI Dock Gate（SV22-C14）

- #384 Native Dock 採用 Gate: **PASS = GPUI メイン Dock の
  実 layout/clip/tab/resize/input/focus 管理下で Plugin 自身の
  実 nativeEditor が動作**。Floating 位置合わせ・Generic
  Editor・fake/static screenshot・remote screenshot は
  代替にしない。
- #384 Capability Matrix の NativeSurfaceSlot 行は全 Driver
  で `PoC gate`/`契約 fixture のみ` — declared であり
  実証済みではない。
- `ruviform-poc-native-dock`（非 default `gpui` feature）は
  #388/#389 の早期 Gate harness であり、実機 PASS 判定は
  本レビューでは行わない。
- Synvelis #176 が製品 E2E で確認。#262 が OS/format/
  capability 別の採否判定。
- 本レビューはこれらを `defers` で接続し、Floating/Generic/
  Mock/GPUI-simulated の成功を最終採用 PASS に**しない**。

## 16. 設計上の決定と却下理由

| 決定 | 内容 | 却下した代替 |
| --- | --- | --- |
| D1 | 本 slice を「設計レビュー + 証拠台帳の正本化」とし、コード変更を行わない | 実装ギャップを本 PR で修正 → scope 外・各 Owner Issue の責務 |
| D2 | TA-001/005 の DAG 検査を `cargo metadata --locked` + DFS で再現可能に記録 | `cargo tree` 目視 → 非再現的・dev/optional/target 分類が不明瞭 |
| D3 | fixture 存在をソース pin（rev 固定）で Pass、実走を NotRun/先行参照に分離 | fixture を本 slice で実行 → ruviform は別 repo で build 対象外・docs slice の範囲を超える |
| D4 | GUI 方針を「整合確認」に留め、GPUI 最終採用を宣言しない | #384 proposed を accepted とみなして PASS → #262/#388/#389 の実機 Gate を迂回する誤判定 |
| D5 | legacy window 経路（close=終了）と multi-window driver 経路を区別して記録 | 単一契約として扱う → TA-011 の Job Cancel 判定を誤る |
| D6 | `synvelis_dark` theme fixture・PPQN 範囲差を P2/P3 観測として分離記録 | blocker 扱い → DAW 型漏出ではなく consumer 名データ・部分集合制約であり設計違反ではない |
| D7 | pre-existing の broken relations（route graph blocked）は範囲外として記録のみ | 修復 → 無関係な docs の relation を本 slice で変更するのは scope 違反 |

## 17. Fixture/テスト対応表（test-spec への写像）

| 要件 | 主な fixture/検証 | 層 |
| --- | --- | --- |
| R-TA-01 | cargo metadata cycle 検査 + check-headless-deps + check-ruviform-rev | DAG/tooling |
| R-TA-02 | ruviform src `synvelis` 走査 + DAG 方向 | source scan |
| R-TA-03 | `ruviform-graph` deps + `poc_ex_001`〜`006` | fixture pin |
| R-TA-04 | `g2_01`/`g2_02`/`g2_03` + PortRole::Sidechain + ValidationError | fixture pin |
| R-TA-05 | plugin-host/audio/bridge の resolve 辺分類 | DAG |
| R-TA-06 | `typed/runtime.rs` + `typed_activation.rs` `g2_04_*`/`g2_05_*` | fixture pin |
| R-TA-07 | `rt_*` suite + assert_no_alloc + ruviform-rt retire/spsc | fixture pin |
| R-TA-08 | transport_schedule 型 + `play_then_seek`/`stale_generation`/`install_rejects` | fixture pin |
| R-TA-09 | music.rs 型 + `variable_ppqn`/`beat_position`/`tick_position` + sv19 tim_* | fixture pin |
| R-TA-10 | io-wav `roundtrip_*` + `staged_seal_readback_publish_e2e` + `staged_drop_*` | fixture pin |
| R-TA-11 | ruviform-ui `multi_window`/`runtime` + synvelis `close_veto` + shell.rs | fixture pin |
| R-TA-12 | crate 棚卸し + consumer table + check-headless-deps | inventory |
| R-TA-13 | ADR status 照合 + native feature 分離 + check-headless-deps | doc/tooling |
| R-TA-14 | #384 Gate + #388/#389/#176/#262 接続記録 | gate mapping |

## 18. 実装 Owner/Case ID 照合

| Case | 要件 | 実装 Owner（コード側） | 証拠 |
| --- | --- | --- | --- |
| SV22-C01 | TA-001 | Synvelis workspace manifests（変更なし）+ Ruviform crates | `evidence/sv22/dependency-dag.md` |
| SV22-C02 | TA-002 | Ruviform 公開 API（synvelis 非依存を維持） | case-ledger C02 |
| SV22-C03 | TA-003 | `ruviform-graph` | case-ledger C03 |
| SV22-C04 | TA-004 | `ruviform-graph` compiler + `ruviform-audio-plugin-bridge` | case-ledger C04 |
| SV22-C05 | TA-005 | `ruviform-plugin-host`・`ruviform-audio`・bridge | case-ledger C05 |
| SV22-C06 | TA-006 | `ruviform-audio::typed::runtime` | case-ledger C06 |
| SV22-C07 | TA-007 | `ruviform-rt`・`ruviform-audio`・Synvelis `rt_*` | case-ledger C07 |
| SV22-C08 | TA-008 | `ruviform-media-core::transport_schedule`・`ruviform-graph::project` | case-ledger C08 |
| SV22-C09 | TA-009 | `ruviform-media-core::music`・Synvelis sv19 chain | case-ledger C09 |
| SV22-C10 | TA-010 | `ruviform-io-wav` | case-ledger C10 |
| SV22-C11 | TA-011 | `ruviform-ui`・`synvelis-app` shell/close_veto | case-ledger C11 |
| SV22-C12 | TA-012 | 全 consumer crate + 別 repo AudioLens | case-ledger C12 |
| SV22-C13 | GUI 方針 | `synvelis-app` feature + `check-headless-deps` + ADRs | case-ledger C13 |
| SV22-C14 | Native Dock | Ruviform #388/#389・Synvelis #176・#262 | case-ledger C14 |

## 19. 失敗/回復セマンティクス（レビュー観点）

本 slice はコードを変更しないため、回復セマンティクスは
「証拠が取れない場合の分類」に限定する:

- fixture が見つからない/契約が実在しない → `Fail` または
  `Unsupported`（構成済みでない契約は `Unsupported`）。
- 実走していない → `NotRun`（先行実測を SHA 付きで参照する
  場合はその旨を明記し、参照元がなければ NotRun のまま）。
- 実 OS/Plugin/GUI が必要 → `Deferred`（Owner Issue 接続）。
- pre-existing の repo 全体問題（broken relations 等）→
  範囲外として記録のみ、修復しない。

## 20. Alternatives（設計レビューとしての代替案）

v0.7.1 自体の代替案は Ruviform #193・Synvelis #23 の責務。
本レビューが検討したのは**証拠分類の方法**であり、D2〜D7
で固定した（§16）。

## 21. Deferred/NotRun/Unsupported 分類の集約

- **Deferred**: 実 OS Device/実 Plugin/実 Native Dock/
  実 GUI 受入（#176/#262/#388/#389/#294）、AudioLens 独立
  Oracle/3repo SHA tuple（#219/#294）、sv#20/#21 並行
  Worker の docs、merge 後対象 SHA の CI 証拠。
- **NotRun**: ruviform fixture の本 slice 実走（別 repo・
  build 対象外）、`mise run test`/`test-rt`/`check`/`lint`
  の package 引数付き実行（docs slice のため fmt-check +
  2 つの guard のみ canonical）。
- **Unsupported**: 実 Native Dock（Floating/Generic/Mock
  は代替不可）、AudioLens 側証拠（別 repo）。
- **P2 観測**: `ruviform-ui-theme` の `synvelis_dark`
  fixture、PPQN 範囲差（部分集合で整合）、v0.7.1 crate 数
  ドリフト、plugin-gui-embedding ADR が #389 で再設計予定
  の旧経路のまま accepted。
- **P3 記録**: legacy window 経路の close=終了意味論、
  `synvelis-project` PPQN `1..=32767` vs Ruviform `1..=65535`。
