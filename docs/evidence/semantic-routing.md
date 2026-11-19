# セマンティックルーティング — 実行証拠

SF-01〜SF-06（セマンティックルーティングモデル・ルーティングトランザクション・semantic→runtime lowering/activation・プローブ・Signal Flow/UI・クロスゲート oracle）と StagedFile/SFXR 系の永続化・検証にわたる headless 検証の記録である。検証は主に mise run lane（fmt-check・check・lint・test・test-rt・test-msrv・check-headless-deps 等）と cargo test の fixture 群で構成され、外部 gate（実機・device・consumer・実 plugin）は NotRun/Deferred として分離記録されている。task 文書の front matter status と IR の Validation 記録が一致しない箇所があり、その場合は「文書が一致しない」と記録してどちらかを正と断定していない。

## SF-01 セマンティックルーティングモデル（synvelis#104）

impl-ir の Validation 記録では、mise run fmt-check・check（workspace 全 target）・lint（clippy --workspace --all-targets -D warnings）・test（cargo test --workspace --locked、新規 routing_sf01 15 test 含む全 suite 緑）・build・test-msrv（rust 1.89）が全て Pass した。task 文書の仕様では、7 種の安定 ID 型・12 routing operation・順序付き v5→v6 migration・厳格 routing invariant oracle・1 CAS/1 Undo を Done When/Verification として定義した。

gateway 層の op_facts は fail-closed catch-all で routing ops を拒否する設計であり、routing op は session/controller 経路のみであった。runtime lowering・mute/solo・temp-reference transaction は実装せず SF-02/SF-03 へ scope を分離した点が IR の設計判断として記録されている。

## SF-02 ルーティングトランザクション（synvelis#105）

本書の対象文書群には plan と 2 件の task（staging model・validation acceptance tests）のみが含まれ、対応する impl-ir/exp は存在しない。task は temp 参照・preflight・graph validation・atomic RoutingEditIntent・stale/dangling/invalid/cycle/undo/save-load の受入テストを規定するが、この文書集合内では完了の実装記録を確認できない。

設計上の確定事項として、複合編集の 1 revision/1 undo 集約・拒否時の live state 不変・mute/solo の strip-owned control target 権威・セマンティック+汎用グラフの二層検証が plan/task に明記されている。

## SF-03 semantic→runtime lowering・activation（synvelis#106）

slice4（NoRebuild Equivalence・processor control 払い出し）では sf03_l4.rs を新規作成し、equivalence・非 equivalence・reconcile revision-drift 抑制・take_processor_control one-shot・SF-ACT-05 headless 固定・SF-ACT-07/08 skipped_fault 両 mode fail-closed を実測した。SF-ACT-05 headless 固定の内容は、rapid R1/R2/R3 の receipt 系列・Superseded publish 拒否・PublishQueueFull・stale counter→EventStale reconcile・DeviceEpochChanged・fresh retry である。既存 sf03_l1/l2/l3 は新署名で green 回帰なし。cargo test -p synvelis-player --test sf03_l4 と mise run test が green。

slice5（session engine 接続）では synvelis-app tests/sf03_session_engine.rs 5 件を追加した。session_apply_publishes_and_activates（apply→receipt Published→run→Active+last_good_generation）・failed_build_keeps_last_good（unresolved binding で Failed+last_good、旧 generation 継続）・semantic_noop_is_equivalent・stale_revision_rejected（Err(StaleRevision)・graph_builds 不変・reconcile が revision mismatch 報告）・last_prepare_error 経路である。mise run test/test-rt/lint/fmt-check/check-headless-deps 全て exit 0。

slice6（RoutingControlDispatch）では sf03_l6.rs 6 fixture で automation delivery・bounded queue・catalog refresh revalidate・re-lowering を実測し 6/6 pass。mise run test（workspace 全 green・sf03_l6 含む）・lint exit 0（toolchain 1.99.0）。実 OS/実 plugin format/実 GUI・AudioLens f64/RT 非回帰は NotRun であり Ruviform#294 の責務とされた。

slice8（revision fixture）では app tests 4 件（same_committed_revision_across_receipt_map_probe_and_view・viewstate_and_semantic_noop_keep_revision_axes_honest・controller_bypass_surfaces_as_reconcile_drift_then_converges・save_is_persistence_evidence_not_activation）と workspace tests 3 件（session_engine_projection_and_evidence_share_one_revision・revision_transitions_stale_prior_generation_evidence・save_is_persistence_evidence_on_the_same_revision_axis）が green。fixture 作成中に bypass 用 AddTrack が strip 無しで rebuild PluginUnavailable 失敗する問題を検出し、既存 lane/source の AddClip bypass に修正した。production src 変更なし。

slice9（capability firing）では sf03_l9.rs 5 fixture を追加した。host_generation_tracks_registrations（boundary/catalog/factory 全登録軸で決定的変化・mutation 無しで安定）・publish_rejects_factory_drift（prepare→publish 間 drift で PluginCapabilityChanged・last-good 保持・drift 後再 build 成功）・reconcile_reports_host_capability_drift（CapabilityGeneration mismatch+fault・plan は live）・host_drift_defeats_equivalence（drift 下 semantic 同一 revision が Built・drift 無しは Equivalent）・host_swap_is_capability_drift である。canonical 9 lane 全 exit 0。NotRun 表には ActivationNotObserved・RetireOverflow・reconcile DeviceEpoch・実 VST3 host drift・実機/GUI/RT・SF-ACT-07 preview policy が全て理由付きで明示された。

slice1〜4 の task 文書は status done・Done When/Verification 全チェック済みで、sf03_l1〜l4・sf03_session_engine 等の headless fixture green を記録した。ただし slice1/2/3 の対応 impl-ir は対象文書集合に含まれず、IR レベルの最終検証記録は slice5/6/8/9 のみである。

## SF-04 プローブ（synvelis#107、ruviform#308/#309）

slice1（semantic probe adapter）では ruviform pin を cc7d5296→0f5b5737 に bump した（ruviform#308 RebindReceipt/CapturePolicy・#309 ProbeClockMark/ProbeTimeContext/ObservationCoverage 含む main HEAD）。project contract 型 + resolve_probe_point fail-closed + player ProbeSession（Requested→QueuedForBinding→BoundActive lifecycle・generation-guarded stale reject・RebindReceipt segment cut・remove→Stopped）を実装し、headless test 15 件（project 8 + player 7）green を IR に 8/8・7/7 pass として記録した。task front matter は in-progress のままである。

slice2（ControlSession probe trace）では ProbeTracePort/ProbeObservationUpdate/ProbePortError（project）+ ProbeTraceBridge（player）+ ProbeTraceSession（workspace ControlSession field）を実装し、workspace fake-port E2E 9 件 + player bridge 3 件 green。check-ruviform-rev exit 1 は pre-existing failure（vendor REV 20a24a66 vs Cargo.lock 0f5b5737、両ファイル diff 未変更・`ui/`・`synvelis-app` は編集禁止範囲のため未対処で PR 記録、synvelis#230）。

slice3（clock epoch discontinuity）では CaptureSegmentEnd::ClockEpoch + ProbeSession::bump_clock_epoch/time_context + drain epoch 照合（lazy cut・triggering record 破棄・closing kind 除外・unwired clock no-op）+ RoutingEngine::seek epoch bump + probe_time_context accessor を実装した。sf04_s3_probe_discontinuity 5 件（seek epoch・capture segment cut・DSP invariance・capacity・rebind channel-full atomicity/recovery）5/5 pass。fmt-check/check/lint/test/test-rt/test-msrv/check-headless-deps exit 0、check-ruviform-rev は slice2 と同じ pre-existing failure を記録。

slice5（subscriber/window ownership）では ProbePrincipal/ProbeReleaseReport/NotSubscriber/NotCaptureOwner + leases/by_view/capture_owners を実装し、11 unit test 追加で全件 pass。test lane 初回は synvelis-library webdav 2 件の既存 flaky（os error 10054）で fail したが、単独再実行・全量再実行・MSRV（webdav 42/42 pass）で既存 flake と分類した。test-msrv rust 1.89.0 exit 0。設計段階で invariant 矛盾 2 件・unsubscribe atomicity を検出・修正した。

slice6（probe compare）では ProbeComparisonRequest/AcousticObservation 契約型（#[non_exhaustive]・runtime id 非含有・clock id evidence 型は serde 対象外）+ ProbeCoverage（PROBE_COVERAGE_MAX_EVENTS=2048・loss floor）+ probe_compare free function を実装した。sf04_s7_probe_compare 10 test 全 pass（Aligned/Conditional/Incomparable 全到達分岐を実 engine fixture で固定）。task の canonical 9 lane・commit/push/PR checkbox は未チェックだが、IR に 10 passed/0 failed（2026-11-12）記録があり文書間で状態が一致しない。

slice8（faults + declare）では RoutingEngine::declare_probe_discontinuity（bump_clock_epoch delegate）+ SessionRoutingEngine passthrough を実装した。sf04_s8_probe_faults 5 件（F1–F5）+ sf04_s9_probe_discontinuity_declare 5 件（F6–F10）全 pass、既存 sf04_* 25/25 green。PendingClose による drain batch 末尾 close 修正を行い、BudgetExhausted+CaptureStopped 同 batch で close 前 chunk orphan する実ギャップを fixture が露呈させた。canonical 9 lane 全 green。test 初回は os error 112 disk-full（synvelis-library cache test のみ）で失敗したが、容量回復後再実行で全 pass・コード起因でない。外部 gate（実機・#109 Oracle・#108 UI・syn.probe.compare・SF-PROBE-08/09・consumer 照合）は NotRun。

slice10（syn.probe.compare op）では service.rs に OpInfo+dispatch、ui_ops.rs に op_probe_compare+probe_observation_json mapper（&self read-only・engine 未接続は engine_attached:false+observation:null・malformed は INVALID_PARAMS）を追加した。sf04_probe_compare_op.rs 6 test pass（Aligned/Conditional/Incomparable・fail-closed・read-only 不変条件）。loop-discontinuity では ui/src/ipc.ts OpName に syn.probe.compare 追加、RoutingControlDispatch に last_observed_loop_discontinuity+observed_loop_discontinuity()、SessionRoutingEngine::run を per-block run_observed(1)+PumpPolicy emulation+PlayReport merge に再構成した。sf04_loop_discontinuity.rs 4 test pass（wrap/apply 検出・宣言発火・cursor/PCM 非移動・跨ぎ compare fail-closed・CommandChannelFull fail-closed）。test-ui vitest 56 tests pass。*-msrv/*-full・3OS は NotRun。

probe-adapter 集約監査（Issue #107 SF-PROBE-01..08）では、監査で未被覆分岐 2 件を特定した。bound probe の entity 消失（RemoveProcessor 等による SourceMap 失効→Unresolved unbind）と per-key subscription dedup 境界（session_id 違いが独立 subscription になる境界）である。sf04_probe_acceptance.rs 2 test を RT-observed binding で追加した。evidence/sv107/ に被覆表・検証結果・外部 gate NotRun/Unsupported を記録。fmt-check/check/lint/test（両 crate）/test-rt/check-ruviform-rev 全 exit 0。check-ruviform-rev は sync-ui-deps で vendor 再同期後 Passed と記録する一方、Risks 節は sync-ui-deps 実行自体を evidence/PR に NotRun と記録としており軽い緊張がある。

## SF-05 Signal Flow・probe 配線・UI view model（synvelis#108）

slice1（headless）では workspace signal_flow_ui read model（scope/collapse/role filter/dedup/stale/unsupported を verbatim に運ぶ）+ syn.signal_flow.view/syn.signal_flow.visual op（visual op が session/project/engine 不変）+ Signal Flow tab が main view spec に additive 表示を実装した。sf_05 13 test + sf05_signal_flow 5 test 含む mise run test 全 pass。

slice2（gesture/inspection/persist）では routing_gesture（fixation/hover/drop/confirm/cancel・stale revision/capability/window・prohibited・consent）+ inspection API（Follow/Explore/Pinned・session-only 履歴・FollowSource filter）+ ViewState::signal_flow schema v2→v3 per-view 永続化を実装した。sf_05_s2 18 件 + sf05_signal_flow 10 件 pass（task 記録）。link delivery は明示変換成功時のみ Signal Flow Follow focus 更新・selection/arrangement focus 不変。

sf05-editor IR（被覆監査）では SF-UX-03/04/07 の headless 検証可能な欠落 6 件を特定し、sf_05_s2.rs に 7 test を追加した（既存 18 件不変→計 25 件全 pass）。mise run test synvelis-workspace exit 0。

slice3（probe subscriber wiring）では syn.probe.control の view-scoped action（view_id→ViewInstanceId 解決・session authority と分離）+ op_view_close/close_vcs_view の lease release hook + syn.probe.state の holders/capture_owner/subscribers 投影 + Signal Flow tab probe table（read-only）+ OpName 追加を実装した。sf05_probe_share.rs 6 test — engine 無し fail-closed・2 view subscribe で provider request 1 回 dedup・owner-only abort・非 lease view の INVALID_PARAMS。mise run fmt-check/check/lint/test/test-rt/test-msrv/check-app-native/check-ui/test-ui 全 exit 0。

ui-viewmodel では ui/src/viewmodel/ に signal-flow-view-model.ts・probe-view-model.ts を新規作成し、OpName union に 7 op（syn.signal_flow.view/visual/inspection/routing・syn.probe.control/state/compare）を追加した。vitest 8 files/73 tests（新規 2 spec 13 tests）pass・tsc --noEmit pass。check-ui（vendor sync check 含む）pass。build-ui は views/app 未変更のため NotRun/不要、#109/#103 oracle join・SF-UX-08/Windows L2 は範囲外 NotRun。

task/IR front matter は in-progress のままで検証は Pass 記録である。wire 語彙の相違が fixture 修正の原因になった。Signal Flow は Debug 整形データ・probe serde は kebab-case・entity tag は PascalCase（"Track"）で、lowercase 前提の初期 test fixture を契約に合わせて修正した。

## SF-06 cross-gate oracle・SFXR-01（synvelis#109）

sf06-cross-gate-headless-oracle では crates/synvelis-project/tests/sf06_oracle.rs 6 test を追加した。AudibilityOracle+routing_invariant_violations の独立 persisted-domain oracle・mix/chain fixture topology・exhaustive mute×solo sweep（mix 2^14）・fault matrix（reject→state unchanged）・v5→v6 migration oracle である。cargo test 6/6 pass、mise run fmt-check/check/lint/test/test-rt exit 0、workspace 全 test binary Pass・0 failed（task 記録は 159 test binary）。evidence/reports/sf-06-cross-gate-ledger.md に Case ID registry・lane 別結果（headless Pass・実機/consumer NotRun）を記録した。

sf06-cross-gate IR では crates/synvelis-player/tests/sf06_send_path_pcm.rs を新規作成した（send/bus/pre/post tap・mute/solo propagation の PCM oracle）。evidence/sv109/ に SF-ACT-01..09・SF-PROBE-01..09 の被覆写像を新設した。先行 ledger（evidence/reports/sf-06-cross-gate-ledger.md）が SF-03/04 merge 前の stale な T2 NotRun を抱えていたため、現行 tests/ 全件実測 green に基づく被覆の正本とした。check/lint/test synvelis-player・test synvelis-project・test synvelis-render 全 exit 0。merge 後対象 SHA の ci.yml covering run は NotRun pending。

sfxr01（CTL-01/CTL-02 cross-gate fixture）では crates/synvelis-player/tests/sf_xr_01.rs に sfxr01_01_*/sfxr01_02_* の 2 test を追加した。strip/send 権威・serialization・undo・lowering・channel-map consent・atomic rejection を persisted+runtime 横断で検証する。既存 coverage 棚卸し（ctl_01.rs 21 test・ctl_02.rs project 9+・sf_02・sf06_oracle・sf03_l1）で重複を回避した。cargo test 2 passed。fixture commit 43511f0（merge commit、test file は af1cac1 で追加・review 修正の tautology 除去を merge に fold）。src 変更なし・他 crate 非接触。ledger SF-XR-01 行更新+headless evidence row 追加。task の merge 後最終検証・push・PR checkbox は未チェック。

spec/ADR の規則として、headless Pass を実機・device・consumer 受入へ昇格しない。外部 gate は external-gates.md で NotRun/Deferred を明示する。

## StagedFile 移行（synvelis#84、ruviform#294/#305/#306、AudioLens#143）

impl-ir Validation では mise run test synvelis-project exit 0 — 41 test binaries 全て test result: ok・FAILED 0。store_publish 16/16・head_cas 9/9（2-process e2e 含む）・controller_store 11/11・Doc-tests 2/2。fmt-check/check synvelis-project/lint synvelis-project 全 exit 0。

nearest-SHA CI 証拠（run 43457068626・head_sha 285e1c45）では、ubuntu-latest job 132006742268・windows-latest job 132006670018・macos-latest job 132006666935 で 3 fixture 全 Pass。msrv-verify job 132006754106（rust 1.89）で 3 fixture Pass。rt-verify job 132006741582 は rt_ filter で対象 fixture 0 selected のため RT 証拠としては不適と分類した。

対象コード SHA（e0fbbe8/1947623）の ci.yml run は両 commit とも 0 件（dispatch-only CI）で NotRun。統合受入は merge 後の対象 SHA CI 証拠に委ねる。nearest-SHA 採用の byte 同一範囲は fixture binary 入力（store.rs+3 fixture+crates/synvelis-project manifest+Cargo.lock）で限定し、lib.rs 等の store 無関係差分で fixture binary が変わり得る点を Risks に記録した。

Windows 実機検証は L2 相当の通常 FS（実 NTFS temp dir）で Snapshot/HEAD CAS/Recovery の状態遷移・Readback/再 Open・衝突/失敗復旧を記録した。Plugin は NotRun/Deferred/別 Owner に分類。「Publish ≠ Project Commit」の責務境界と両 consumer（Synvelis・AudioLens）の独立 Golden を design/test-spec/IR に記録した。

実装本体は先行 task（store StagedFile 移行・HEAD CAS・HEAD CAS e2e）で main 統合済み（PR #178/#342/#382）であり、本 slice は証拠統合・受入検証のみ。監査で fixture gap は発見されず追加 fixture 不要と判断し、production code 追加なし。defers として ruviform#305/#294/#306・synvelis#28/#294・audio-lens#143 は着手を止めない関連 Issue として記録した。

## 文書状態の矛盾と未解決事項

task front matter の status と IR の完了記録が一致しない文書が複数ある。sf-04 slice1/2/3/4/5 task は in-progress/in_progress、sf-04-slice6 task は canonical 検証・commit/push/PR checkbox 未チェック、sf-05 slice3・ui-viewmodel task は in-progress、sfxr01 task は merge 後検証・push・PR 未チェック。対応 IR は検証 Pass・fixture commit を記録しており、文書間で状態記述が一致しない。本記録は「文書が一致しない」と記録し、どちらかを正と断定しない。

check-ruviform-rev の pre-existing failure が複数 slice で記録された。vendor REV 20a24a66/b02942c2 vs Cargo.lock 解決 0f5b5737/fd1d4d5f。編集禁止範囲の slice では未対処のまま PR/evidence に記録し、許可範囲では sync-ui-deps で再同期した。probe-adapter IR は check-ruviform-rev Passed（sync-ui-deps 後）と記録する一方、Risks 節で sync-ui-deps 実行を NotRun と記しており軽い緊張がある。

SF-06 の evidence ledger は 2 系統存在し内容が新旧で異なる。先行 ledger（evidence/reports/sf-06-cross-gate-ledger.md）は SF-03/04 merge 前の stale T2 NotRun を含み、新 coverage 文書（evidence/sv109/）が SF-ACT-01..09/SF-PROBE-01..09 の正本被覆写像である。片方を消さず乖離として扱う。

既存 flaky/環境起因として、synvelis-library webdav test の os error 10054（TransientIo vs InvalidRequest 分類差、単独/全量/MSRV 再実行で pass）と os error 112 disk-full（並行 build 時の C: drive 一時枯渇、容量回復後 pass）はいずれもコード起因でないと記録済み。

外部 gate の NotRun/Deferred は広範である。実 OS/GPUI lifecycle・実 2-window・実 VST3/AU/CLAP plugin・実機 RT latency・AudioLens#143 統合・consumer cross-gate・SF-03 の ActivationNotObserved/RetireOverflow/DeviceEpoch/SF-ACT-07 preview policy・StagedFile 対象 SHA CI は全て未実施または委譲先待ち。

SF-02 は対象文書集合に impl-ir/exp が存在せず plan+task のみであり、headless 実装の完了証拠はこの文書集合からは確認できない。probe-adapter 監査で特定された未被覆分岐は sf04_probe_acceptance.rs 2 test で被覆されたが、task T6（commit/push/PR/worker_done）は未チェックのままであり、PR lifecycle は文書から推定しない。

SF-03 slice9 の NotRun 表には ActivationNotObserved（engine epoch/instance は構築時 pin で遷移 API 無し）・RetireOverflow（Ruviform retire channel overflow・upstream fault-injection seam 必要・ruviform 側 Issue 候補）・reconcile DeviceEpoch（epoch 遷移 API 無し）が構造的理由付きで記録された。headless で再現不能な fault は upstream seam 追加待ちである。

UI view-model の build-ui・#109/#103 oracle join・SF-UX-08/Windows L2・3OS/`*-msrv`/`*-full` lane は各 slice で NotRun/不要として明示記録されており、headless consumer path のみが検証済みである。

StagedFile の責務境界「Publish ≠ Project Commit」は store publish と project commit を混同しない設計で、両 consumer（Synvelis・AudioLens）の独立 Golden を要求する。consumer 側統合は AudioLens#143 等の別 Issue に委譲される。Ruviform 依存の既定ブランチ追従と pin bump の運用では、SF-04 slice1 で cc7d5296→0f5b5737（ruviform#308/#309 含有）に bump し Cargo.lock 反映。StagedFile 関連の ruviform#294/#305/#306 は defers として記録され、Synvelis 側での恒久的重複実装を避ける方針が AGENTS.md と整合する。

文書作成の実務的知見として、evidence/svNN/ 配下に被覆表・検証結果・外部 gate 判定を slice 単位で記録し、docs index は doc_maintenance.js で再生成、review-protocol で blocker 0 を確認してから commit/push/PR に進む流れが全 slice で共通であった。

未解決の将来作業として文書に明示されるものは、loop/device restart semantics・self-healing unresolved probe bindings・streaming probe compare・UI mounting・session teardown cleanup・実 plugin/device 検証・consumer 統合・SF-03 の fault-injection seam（ruviform 側）・SF-ACT-07 preview policy である。

## 出処

synvelis#84・#104・#105・#106・#107・#108・#109・#230、ruviform#294・#305・#306・#308・#309、AudioLens#143。SF-01〜SF-06（セマンティックルーティング・ルーティングトランザクション・lowering/activation・プローブ・Signal Flow・クロスゲート Oracle）と StagedFile/SFXR 系の永続化・検証に関する作業群からの抽出。
