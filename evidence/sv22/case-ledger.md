# SV22 Case Ledger — Target Architecture 設計レビュー（synvelis#22）

対象 Issue: https://github.com/scarecrow173/synvelis/issues/22
対象 Revision:

- Synvelis: `scarecrow173/sv22-target-architecture` worktree、
  base HEAD `7802ac7ae3e88eac1e67f8b734b68dca22bd2e7f`
- Ruviform: `Cargo.lock` resolved
  `fd1d4d5f4de57092b96ad1cd309b85fe47cbef38`
  （`check-ruviform-rev` = Pass、27 lockfile sources 単一 SHA）

## Environment / 実行した検証

| 項目 | 値 |
| --- | --- |
| OS | Windows（MINGW64_NT-10.0-26300, Git Bash） |
| toolchain | rustc/cargo 1.99.1（mise 管理）・node v26.12.1 |
| 実行日 | 2026-11-14 |

| コマンド | exit | 結果 | 分類 |
| --- | --- | --- | --- |
| `cargo metadata --format-version 1 --locked --offline` + node DFS cycle 検査 | 0 | normal-dep cycle = 0（Synvelis resolve 全体 + Ruviform 42 member） | Pass（本 slice 実走） |
| `mise run fmt-check`（`cargo fmt --all -- --check`） | 0 | フォーマット差分なし | Pass（docs 追加前に実行。docs は Rust 非対象） |
| `mise run check-ruviform-rev` | 0 | `resolved fd1d4d5… (27 lockfile sources, lockfile-primary, REV match)` | Pass（初回 stale vendor 検出→`mise run sync-ui-deps` で復旧後に再実行） |
| `mise run check-headless-deps` | 0 | `no GUI framework deps in 10 headless crates` | Pass |
| `mise run check`/`test`/`test-rt`/`lint` | — | package 引数必須・docs slice のため対象 crate 変更なし | NotRun（下記 C07/C12 に根拠記録） |
| ruviform fixture 実走（`cargo test -p ruviform-*`） | — | 別 repo・build 対象外 | NotRun（先行 slice/CI 記録を SHA 付き参照） |
| 実 OS Device/実 Plugin/実 Native Dock/実 GUI | — | EXT-L3/GUI-L2 相当・別 Owner Issue | Deferred |

## Case 一覧

凡例: **Pass** = 本 slice で実行した検証の成功、または先行
slice が SHA/環境付きで記録した実測の参照（明記）。
**NotRun** = fixture/契約は実在するが本 slice で未実行。
**Deferred** = 別 Owner/将来 lane。**Unsupported** = 環境上
不可または明示的対象外。

### SV22-C01 — TA-001: Production Dependency DAG 非循環

- **要求状態**: Required
- **結果**: **Pass**（本 slice 実走）
- **証拠**: `evidence/sv22/dependency-dag.md`
- **内容**: `cargo metadata --locked --offline` の resolve
  グラフに対し normal-dep 辺のみで DFS cycle 検査。
  Synvelis resolve 全体・Ruviform workspace（fd1d4d5・42
  member）とも cycle = 0。dev/build/optional/feature/
  target 辺を分類記録。`check-ruviform-rev`・
  `check-headless-deps` とも exit 0。
- **対象 Revision**: Cargo.lock `fd1d4d5…` + worktree HEAD。

### SV22-C02 — TA-002: 型漏出禁止

- **要求状態**: Required
- **結果**: **Pass**（契約実在を pin。実走 = ソース走査）
- **証拠**: design §3、`dependency-dag.md`（DAG 方向）
- **内容**: resolved rev `fd1d4d5` の Ruviform `src/` を
  `synvelis`・DAW 意味論型名で走査。Synvelis crate 依存・
  Project/Track/Take/Comp/Automation/Undo/RenderRecipe 型の
  公開 API 侵入なし。`ruviform-ui-theme/src/generated.rs` の
  `synvelis_dark` module と doc 例示の `synvelis::` 言及は
  consumer 名 theme データ/doc であり DAW 型ではない —
  P2 観測として Ruviform Owner（#193/#219 系）へ記録。
- **独立利用**: ruviform 共通契約 crate の通常依存に
  Consumer crate なし。Synvelis→Ruviform 一方向を DAG で確認。

### SV22-C03 — TA-003: Device/Host 非依存 Graph 検証

- **要求状態**: Required
- **結果**: **Pass（契約実在 pin）/ fixture 実走 = NotRun**
- **証拠**: `ruviform-graph` Cargo.toml（通常依存 =
  `ruviform-audio-core` のみ）・`tests/poc_ex.rs`
- **内容**: `poc_ex_001`〜`006` が OS Device・実 Plugin
  Host なしで構築・検証・失敗（failure preserves published/
  main-only golden）・revision mapping をカバー。
  device/host import なしを依存集合で確認。
- **NotRun 根拠**: 別 repo の crate であり本 docs slice の
  build 対象外。実走結果は先行 slice/CI の SHA 付き記録を
  参照（本 slice では新規実行していない）。

### SV22-C04 — TA-004: Fan-in/Fan-out・Sidechain・PDC・無効拒否

- **要求状態**: Required
- **結果**: **Pass（契約実在 pin）/ fixture 実走 = NotRun**
- **証拠**: `ruviform-audio/tests/typed_compiler.rs` fixtures・
  `ruviform-audio-plugin-bridge` bus fixture・`port.rs`
  `PortRole::Sidechain`・`validate.rs` `ValidationError` variants
- **内容**: Fan-in scratch slot（`g2_01`）・channel map
  （`g2_02`）・宣言 delay 拒否（`g2_02`）・PDC latency 差
  補償（`g2_03`）・Sidechain port role・無効 Port/Channel
  の typed 拒否（InvalidDirection/DomainMismatch/
  FormatMismatch/ImplicitConversionRequired/InvalidChannelMap/
  MissingNode/MissingPort/PortNotOnNode/Multiplicity/Cycle）。
- **注記**: v0.7.1 の「既存 Audio Graph は Pull-ordered・
  typed multi-port/PDC は将来要件」は旧 `ruviform-audio`
  graph を指し、typed 系（`ruviform-graph` +
  `ruviform-audio::typed`）は別層として実装済み fixture
  を持つ — 台帳で区別を固定。
- **NotRun 根拠**: C03 と同じ。

### SV22-C05 — TA-005: PluginHost↔AudioEngine 境界

- **要求状態**: Required
- **結果**: **Pass**（DAG 実測）
- **証拠**: `dependency-dag.md` TA-005 注目辺表
- **内容**: `ruviform-plugin-host`→`ruviform-audio` は
  **dev-dep のみ**（`tests/plugin_graph_e2e.rs` AC3 契約）。
  `ruviform-audio`→`plugin-host`/`bridge` なし。
  `ruviform-audio-plugin-bridge`→両者が正方向の接着層。
  通常依存の不要依存・循環なし。

### SV22-C06 — TA-006: 単一 RT Owner

- **要求状態**: Required
- **結果**: **Pass（契約実在 pin）/ fixture 実走 = NotRun**
- **証拠**: `ruviform-audio/src/typed/runtime.rs`・
  `ruviform-audio/tests/typed_activation.rs`（`g2_04_*`/`g2_05_*`）
- **内容**: generation-tagged SPSC command・
  `active_generation` AtomicU64・install/remove は plan
  generation 一致時のみ・`Send` 非 `Sync` で single-owner
  を型強制。stable slot vs plan-owned・deferred destruction・
  stale generation 拒否 fixture が実在。
- **NotRun 根拠**: C03 と同じ。

### SV22-C07 — TA-007: RT alloc/block/危険 Drop 禁止

- **要求状態**: Required
- **結果**: **Pass（契約実在 pin）/ 実測 = 先行参照 + NotRun**
- **証拠**: `rt_*` fixture 群（ruviform:
  `rt_typed_process_no_alloc`/`rt_run_callback_no_alloc`/
  `rt_device_restart_no_alloc`/`rt_queries_no_alloc`、
  Synvelis 各 crate の `rt_*` suite）・`assert_no_alloc`・
  `ruviform-rt` retire/spsc/alloc_counter・
  `g2_05_retire_queue_saturation_flagged_leak_safe`
- **内容**: RT callback の allocation/blocking/危険 Drop
  禁止が deferred destruction + collector drain +
  saturation leak-safe flag で構造化されている。
  `mise run test-rt`（`cargo test --workspace --locked rt_`）
  が rt_* suite の canonical 入口。
- **NotRun 根拠**: docs slice で crate 変更なし・workspace
  全体 test は範囲外。alloc/block の実測は先行 slice/CI
  記録（SHA 付き）を参照し、本 slice では新規実行しない。
  **検証不能な経路を Pass にしない**原則に従い、fixture
  存在のみを Pass とし実測は NotRun/先行参照とする。

### SV22-C08 — TA-008: Generation/時点契約

- **要求状態**: Required
- **結果**: **Pass（契約実在 pin）/ fixture 実走 = NotRun**
- **証拠**: `ruviform-media-core/src/transport_schedule.rs`・
  `ruviform-graph/src/project.rs`・`ruviform-audio/src/typed/runtime.rs`
- **内容**: `ClockEpochId`/`TransportGeneration`/
  `PlanGeneration`/`TempoMapRevision`/`LoopOccurrence`/
  `CommandId`/`EngineFrame`/`AppliedReceipt`/`ApplyStatus`/
  `RejectReason`/`ApplyDegradation`/`DeliveryPolicy`/
  `ScheduleBudget`/`InBlockTransportSupport` が実在。
  `play_then_seek_receipt_lifecycle`・
  `stale_transport_generation_event_is_faulted_not_delivered`・
  `device_restart_vs_seek_distinct_epoch_generation`・
  `install_rejects_same_generation_different_plan`・
  `loop_wrap_splits_at_end`・`loop_events_fire_once_per_occurrence`・
  `same_frame_events_keep_source_order`・no-allocation callback
  fixture が Commit/Applied/Observed・Loop/Seek を区別。
  GraphProject は requested vs published revision を区別。
- **NotRun 根拠**: C03 と同じ。

### SV22-C09 — TA-009: Musical Time→Sample Event

- **要求状態**: Required
- **結果**: **Pass（契約実在 pin）/ fixture 実走 = NotRun**
- **証拠**: `ruviform-media-core/src/music.rs`・
  `transport_schedule.rs`・Synvelis sv19 chain
  （`tim_01_ppqn`/`tim_02_smf`/`tim02_midi_playback`）
- **内容**: `Ppqn`（`1..=u16::MAX`）・`PPQN=960`・
  `BeatPosition`（正規化有理数・負 preroll・明示 overflow）・
  `TickPosition`（resolution 結合）・`TempoMap`。
  `variable_ppqn_media_roundtrip`/`beat_position_normalizes`/
  `beat_position_arithmetic`/`ppqn_validates`/
  `tick_position_binds_resolution`/`tick_position_projection`/
  `beat_position_off_grid`/`conversion_overflow_is_explicit`/
  `preroll_negative_position`/`ppqn_960_golden_compat`/
  `rt_queries_no_alloc` fixture が精密変換・量子化誤差・
  Loop/Seek を区別。
- **観測**: Synvelis `synvelis-project` の PPQN validation は
  `1..=32767`（SMF 15-bit）で Ruviform `1..=65535` の
  **部分集合** — consumer 側が厳しい制約を課す方向で整合
  （P3 記録、矛盾ではない）。
- **NotRun 根拠**: C03 と同じ + Synvelis sv19 実測は
  sv19 case-ledger を SHA 付きで参照。

### SV22-C10 — TA-010: WAV 実 Encode/Finalize/Reopen

- **要求状態**: Required
- **結果**: **Pass（契約実在 pin）/ fixture 実走 = NotRun**
- **証拠**: `ruviform-io-wav/tests/roundtrip.rs`・
  `tests/contract.rs`
- **内容**: `roundtrip_stereo_s16_48k`/`roundtrip_mono_s24_44k1`/
  `roundtrip_five_point_one_f32_96k_extensible`/
  `roundtrip_stereo_s32_48k`/`roundtrip_short_file`/
  `zero_frame_file_reopens_empty`/`nan_inf_and_clip_values_follow_declared_policy`/
  `drain_then_receive_then_none`/`send_after_drain_is_invalid_data`
  （Encode→Decode）と `staged_seal_readback_publish_e2e`/
  `sink_error_at_finalize_is_io_not_completion`/
  `write_after_finish_and_double_finish_error`/
  `staged_abort_preserves_existing_destination`/
  `staged_drop_after_write_preserves_destination`/
  overwrite conflict・disk-full protection（Finalize/Reopen/
  公開契約）。
- **契約固定**: Drop 時の best-effort（`staged_drop_*`）は
  既存 destination 保全の契約であり**完了証拠ではない**。
  完了 = Finalize 明示成功 + Reopen 検証。
- **NotRun 根拠**: C03 と同じ。

### SV22-C11 — TA-011: Window Close ≠ Job Cancel

- **要求状態**: Required
- **結果**: **Pass（契約実在 pin）/ fixture 実走 = NotRun**
- **証拠**: `ruviform-ui/tests/multi_window.rs`・
  `runtime.rs`・`driver.rs`/`runtime.rs`（`cancel_window_jobs`）・
  `synvelis-app/tests/close_veto.rs`・`src/shell.rs`・
  `docs/specs/sv-gui-04-02-close-veto.md`
- **内容**:
  - multi-window driver 経路: `close_requested_does_not_cancel_jobs`・
    `subwindow_close_leaves_jobs_and_loop_running`・
    `job_events_route_to_owner_window`・`close_request_can_be_vetoed`・
    `destroy_is_distinct_from_close_request`・
    `deferred_close_stays_alive_then_destroyed`。
    `UiRuntime::cancel_window_jobs` は opt-in 明示呼出。
  - Synvelis: `syn.window.close_guard` decision query・
    dirty last-window veto・quit semantics・
    `tests/two_window.rs` の `both_windows_share_one_project_session`・
    `Destroyed`→`cancel_window_jobs` 明示呼出（Window 所有
    Job の scoped cleanup）。
  - Headless 側: `ruviform-ui-test` FakeHost が同契約を
    headless で被覆。
  - **区別記録**: legacy 単一 window 経路は
    `HostEvent::CloseRequested`→`close_requested_cancels_jobs_and_emits_lifecycle`
    で close=アプリ終了の意味論 — multi-window driver 経路
    （CloseRequested は veto/defer 可能・Job 非連動）とは
    別契約。
- **NotRun 根拠**: C03 と同じ + synvelis close_veto 実走は
  `mise run test synvelis-app`（package 引数必須・docs slice
  のため未実行）。先行 slice/CI 記録を参照。

### SV22-C12 — TA-012: Consumer 互換

- **要求状態**: Required
- **結果**: **Pass**（棚卸し + guard 実測）/ 個別 consumer 実走 = NotRun
- **証拠**: `dependency-dag.md`・design §13・`check-headless-deps` Pass
- **内容**: Audio/Video/Plugin/CLI/UI を別責務・別回帰
  Suite・別 NotRun 根拠で記録:

| Consumer | Ruviform 実体 | 回帰 Suite | 状態 |
| --- | --- | --- | --- |
| Audio | `ruviform-audio`/`ruviform-rt`/`ruviform-audio-core`/device adapter | `rt_*`・audio tests | 契約実在 pin・実走 NotRun |
| Video | `ruviform-video*`/`ruviform-video-core` | video tests | 契約実在 pin・実走 NotRun |
| Plugin | `ruviform-plugin-{core,host,clap,vst3,ofx,export}`/bridge | plugin_*・plugin_graph_e2e | 契約実在 pin・実走 NotRun |
| CLI | `ruviform-cli-sample` | cli-sample tests | 契約実在 pin・実走 NotRun |
| UI | `ruviform-ui`/`ui-{gpui,webview,theme,test}`/`ruviform-viz`/`poc-native-dock` | ui tests・gpui-test | 契約実在 pin・実走 NotRun |
| NLE/edit | `ruviform-edit` | edit tests | 契約実在 pin・実走 NotRun |
| Synvelis(DAW) | `synvelis-*` | `mise run test <pkg>` | docs slice のため package test NotRun |
| AudioLens | 別 repo | 別 repo CI | **Unsupported**（本 slice 範囲外）→ #219/#294 で独立 Oracle + 3repo SHA tuple に Deferred |

- **check-headless-deps Pass**: 10 headless crate に
  GPUI/WebView/wgpu/winit/tao/wry・Ruviform GUI adapter
  非含有を機械検査。`synvelis-app` は GUI 依存を feature
  分離するため対象外（意図的設計）。

### SV22-C13 — GUI 採用方針整合

- **要求状態**: Required
- **結果**: **Pass**（文書照合 + guard 実測）
- **証拠**: `docs/adr/sv175-native-shell-driver-neutral-gpui.md`
  （accepted・supersedes sv-gui-01）・
  `docs/adr/sv-gui-01-...`（superseded）・
  Ruviform `docs/adr/driver-neutral-gui-gpui-driver-webview-first-issue-384.md`
  （proposed 仮決定）・`crates/synvelis-app/Cargo.toml`
  （native/webview feature 分離）・`check-headless-deps` Pass
- **内容**:
  - #175（accepted）と #384（proposed）は方向一致:
    Driver-neutral 契約層正本・GPUI 標準 Driver・Consumer
    通常画面に GPUI 型を出さない。
  - 旧 WebView/SolidJS ADR は `superseded` で歴史保持・
    再承認しない。
  - GPUI 進入は `synvelis-app` の `native` feature の
    composition root のみ。10 headless crate に GUI 非含有。
  - **#384 は proposed であり GPUI 最終採用を宣言しない** —
    最終採否は #262/#388/#389 の実機証拠後。本レビューは
    採用方針の**整合**を確認し、最終採用 PASS とはしない。

### SV22-C14 — Native Plugin GUI Dock Gate 分離

- **要求状態**: Required
- **結果**: **Deferred**（Gate 接続記録。本 slice で実機
  受入を行わない・PASS 宣言しない）
- **証拠**: #384 Native Dock 採用 Gate・#388/#389・
  Synvelis #176・#262・`ruviform-poc-native-dock`・
  `sv-gui-03-05` simulated lane
- **内容**:
  - PASS 定義: GPUI メイン Dock の実 layout/clip/tab/resize/
    input/focus 管理下で Plugin 自身の実 nativeEditor が
    動作。Floating 位置合わせ・Generic Editor・fake/static/
    remote screenshot は代替にしない。
  - #384 Capability Matrix の NativeSurfaceSlot 行は全 Driver
    で `PoC gate`/`契約 fixture のみ`（declared・未実証）。
  - `ruviform-poc-native-dock`（非 default `gpui` feature）は
    #388/#389 の早期 Gate harness。
  - 接続: NativeSurfaceSlot=#388・PluginEditorSession=#389・
    製品 E2E=#176・採否=#262。
  - **Floating/Generic/Mock/GPUI-simulated（sv-gui-03-05）の
    成功を最終採用 PASS にしない**ことを台帳に固定。

## 分類集約

- **Pass（本 slice 実走）**: C01（DAG cycle=0）・
  `check-ruviform-rev`・`check-headless-deps`・`fmt-check`・
  C02（ソース走査）・C05（DAG 分類）・C12（棚卸し+guard）・
  C13（文書照合+guard）。
- **Pass（契約実在 pin・実走 NotRun/先行参照）**: C03・C04・
  C06・C07・C08・C09・C10・C11。
- **Deferred**: C14（実 OS/Plugin/Native Dock/実 GUI 受入）
  ・AudioLens 独立 Oracle/3repo SHA tuple（#219/#294）・
  merge 後対象 SHA CI 証拠・sv#20/#21 並行 docs。
- **Unsupported**: AudioLens 側証拠（別 repo）・実 Native
  Dock（Floating/Generic/Mock は代替不可）。
- **P2 観測**: `ruviform-ui-theme` `synvelis_dark` fixture・
  v0.7.1 crate 数ドリフト（28→42）・plugin-gui-embedding ADR
  が #389 再設計予定の旧経路のまま accepted。
- **P3 記録**: legacy window close=終了意味論・Synvelis
  PPQN `1..=32767` vs Ruviform `1..=65535`（部分集合で整合）。
