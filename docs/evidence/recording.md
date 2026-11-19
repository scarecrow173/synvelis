# 録音基盤 — 実行証拠

この文書は、Synvelis の録音・Take・Comping 系検証で実際に実行されたテスト・probe・lane の記録をまとめる。対象は synvelis#12 の受入条件管理と、RCD-01〜04（synvelis#115〜#118）および RCD-F02（synvelis#120、docs-only）の各 slice である。canonical lane は `mise run <task>` であり、直接 `cargo test` は補助証拠として区別される。外部 gate（実 GUI・実 device・実 OS FS・実 RT audio・AudioLens native f64・MIDI 永続化）は文書側で NotRun/Deferred/Unsupported と明示されており、headless/fake の成功を実機 Pass に昇格させていない。

## synvelis#12 受入条件の棚卸し

69 AC の Owner Matrix を evidence として固定した。検証構造は AC を Owner Issue・fixture・独立 oracle・OS Gate・状態に対応付ける docs-only の棚卸しであり、Rust build/test/lint は対象外（NotRun/NotApplicable）である。

- 集計: Pass (headless) 44・Partial 22・Deferred 2・Pass (design) 1・NotRun 0・Unsupported 0。
- AC-28/AC-29（Take Lane/Comp Editor UI）は UI 未実装のため Deferred（GUI Gate）。
- AC-69（API 適合チェック process AC）は本 chain が証拠のため Pass (design) と分類。
- graph route は `blocked`（repository-wide pre-existing broken relation、slice 起因でない）。
- Monitor/Punch headless fixture（PR #387）の統合済みを前提とし、実 Device Capture/同 Clock・Raw Fidelity は synvelis#115 が Owner。（synvelis#12）

## RCD-01 Device Recording・実入力（synvelis#115）

- fake device→real writer 統合: `DeviceRig` 系 16 device-recording test + app recording projection で callback→adapter→writer→staging/journal→finalize/recovery を検証。gate clamp bug を修正し Windows canonical lane で Pass。無関係な webdav test の一過性 flake 1 件は再実行で pass。
- real backend readiness: stamp fallback を累積 frame-count 配置へ改訂後、実 WASAPI device（Audient iD4）で 2 real-device test が Pass（ruviform#565 の `IAudioClock::GetPosition` frame position 欠落が関連）。
- real-input coverage: lane を 10 test に拡張し、記録された 1 run で 10/10 pass（交渉 44.1 kHz stereo F32、30 秒 capture で 1,323,000 frames・0 fault の clean run）。別 run では長時間 `FAULT_EPOCH_DISCONTINUITY`・device disappearance・continuity fault を観測し、抑制せず NotRun/blocked として verified-prefix 証拠付きで分類（ruviform#251 依存、ruviform#629 の virtual endpoint EXT-L3 未 provision、ruviform#628 の fixture variant 未 pin が関連）。
- virtual L1 oracle: `cargo test -p synvelis-recording --test rcd01_oracle` — 7 test pass（stereo・multichannel・2 mono tap・same-clock devices・discontinuity prefix・missing-finalized mixed-session）。production source 無変更。fault semantics と canonical publish scope を明確化。

## RCD-02 Durability・Recovery・Publish（synvelis#116）

- durability/recovery/commit receipt: media/journal/dir sync・durable frontier・明示 review・fail-closed discard・冪等 reconcile を実装し evidence ledger に記録（synvelis#84 HEAD CAS gate 関連）。
- project/library adapter: verified SHA-256 propagation・`MediaReferenceProbe`/`ProjectMediaProbe`/`CompositeProbe`・saved-HEAD probing を実装、adapter 12 test pass（synvelis#86 ALIB blob 連携）。
- blob SHA-256: `cargo test -p synvelis-player --test rcd02_blob_digest` — 10 test pass。namespaced canonical blob + observation-reconcile + legacy CRC32 互換。render layout 互換のため flat CRC32 前提 test を SHA-256 namespaced blob 検証へ更新し legacy-read 互換 test を残した。
- StagedFile publish migration: `PublishMode::CreateNew` で manifest publication TOCTOU を閉塞、fault seam 挙動を検証（synvelis#84）。
- unsynced-loss real-FS probe: `mise run test-recording-real-fs` — 2 test pass（Windows/NTFS 実 FS）。truncate-based deterministic fixture で対称・非対称 unsynced loss が Complete 昇格を防ぐことを確認。真の power-loss は NotRun。
- residual closeout: 既存 ledger + test 名存在確認で AC 照合。canonical lane 実測を記録。test-spec index 重複行 anomaly を発見・記録（unmanaged tail の stale `proposed` 行は generator verbatim 保持のため手編集せず）。AC-3 は RCD-02 所有分のみ Pass、cross-cutting は gate 所有 NotRun/Deferred。

## RCD-03 Monitoring・Punch・Latency（synvelis#117）

- crate slice（PR #214 merge 済みと記録。本 IR は PR #214 で impl record が未作成だったため merge 済み実装を正本として後追い補完したもの）:
  - 検証した内容: `decide` 行列・`MonitorProbe` block 内 Punch 分割・Loop pass ごと gate open・Raw File Byte/span/PCM が Monitor Off/Auto/On で一致・`MonitorAdapter` native placement bit 一致・gain ramp・fail-closed・`LatencyReport` 二重加算禁止・low-latency Ready/Degraded/Unavailable を Fixed PCM/Frame fixture で検証。
  - 結果: `monitor_gate.rs`・`monitor.rs` 新規 test を含む `mise run test` Pass。
- app arm monitoring UX:
  - 検証した内容: 6 monitoring op・bounded session-state projection。`monitor_arm.rs` 11→14 test。
  - 結果: canonical Pass。実 device monitor・実 GUI・#114 Scenario・MonitorAdapter 実 RT graph・count-in UX・AudioLens は NotRun/対象外。
- monitor engine:
  - 検証した内容: `MonitorEngine`/`MonitorSession`/`EngineBlocks` を実装し、`monitor_engine.rs` 10 test・`monitor_gate.rs` 20 test で route/PCM pin（Punch open 区間 Input/Playback 切替 bit 一致・input missing fail-closed・Off/On/Auto×arm×mute/solo×phase matrix・PunchInLoop pass・Loop 連続 open・tempo 境界 punch・monitor bus が engine playback を変更しないこと plain driver と PCM bit 一致・session 位置ずれ fail-closed・sample rate mismatch 拒否）。
  - 結果: `mise run test` Pass（全 workspace 緑）。実 device monitor・実 GUI・count-in UX・AudioLens・RT graph monitor node は外部 gate/別 slice。
- monitor persistence:
  - 検証した内容: schema v11→v12、`monitoring_persist.rs` 10 test・`monitor_arm.rs` 14 test。write-through + undo・save/open rehydrate・session active 中 punch 拒否・観測値非永続を検証。
  - 結果: `mise run test`/`test-rt`/`test-msrv`/`check-app-native`/`lint-app-native`/`build` 全 Pass。review-protocol で CONFIRMED P2（session_json phase live punch 参照→plan.gate 凍結値修正）・P2（literal schema pin 2 件）・P3（session active punch 拒否 fixture 追加）を修正。実 Device/Capture/Software Monitor 可聴計測・実 GPUI GUI・multi-window/focus・#114 Scenario・MonitorAdapter 実 RT graph・low-latency route 実 Prepared Route・VCS diff 分類は NotRun/follow-up。
- monitor session UX:
  - 検証した内容: `syn.monitoring.session` start params に loop/count-in/pre-roll 宣言追加、`GateSpec::Loop`/`PunchInLoop` 構築と `TransportPhase::CountIn`/`PreRoll` 投影を実現。`monitor_arm.rs` 全 20 test（新規 E8 6 件: loop/punch_in_loop gate 投影・punch outside loop 拒否・count_in/pre_roll gate 閉鎖・宣言境界拒否・session-scope 宣言が project を消費しない）。
  - 結果: `mise run fmt-check`/`check`/`lint`/`test`/`build`/`test-rt`/`check-app-native`/`lint-app-native` Pass、`test-msrv` はユーザー指定で Daily CI 委譲 NotRun。experiment log は未作成（先行 slice で確立済みのため optional 判断）。
- monitor punch fixture:
  - 検証した内容: `monitor_punch_session.rs` 4 test（route 観測と raw capture 独立性・device fault が monitor 状態に依らない・punch device loss fail-stop + clean recovery・fault 非漏洩）。production `src/` 無変更。
  - 結果: `mise run test synvelis-recording` Pass。slice 別 evidence で Pass/NotRun を分離記録。
- VCS diff monitor classification:
  - 検証した内容: `vcs02_diff.rs` 28 test（decl add/remove・4 field edit・punch add/modify/remove・partial declaration range 非捏造・Recording domain filter・AA equality・AmbiguousIdentity regression）。`DIFF_POLICY_VERSION` 1→2。
  - 結果: review で CONFIRMED P1（`punch_range` が `Removed` で before 宣言 range を落とす誤り）を修正。`mise run fmt-check`/`check`/`lint`/`test`/`test-rt`/`test-msrv`/`build` Pass。実 device monitor・RT graph・Loop/PunchInLoop gate・count-in UX・実 GUI は後続 slice。

## RCD-04 Comp・MIDI・Recovery・E2E（synvelis#118）

- crate fixture（PR #219 merge commit `9798be5b` と記録）:
  - 検証した内容: `compile_midi_comp` + `rcd_04_comp_cas` 5 test・`rcd_04_midi_phrase` 11 test・`rcd_04_recovery_partial` 4 test・`rcd_04_take_comp_pcm` 1 test、計 21 test。Group Comp stale-revision CAS 拒否・invalid op all-or-nothing・Take/Source/Asset 非破壊・Undo/Redo・Save/Reload・stable ID・`project_comp` 決定性・partial stream verified prefix のみ commit・Hold/cancel で media/journal bytes 不変・Adopt/retry 冪等・corrupt journal fail-closed・Discard clear reference probe 要件・group commit partial/held 分離。
  - 結果: 21 test 全 Pass。`mise run fmt-check`/`check`/`lint`/`test`/`test-rt`/`build` Pass。実 OS/Device/FS durability・実 GUI・2-window stale gesture/history view・AudioLens native-f64/raw-recording 非回帰（ruviform#294 consumer gate）・実 RT engine/player rendering comp PCM 一致・MIDI selection 永続化（synvelis#126）は NotRun/Unsupported。
- product E2E（PR #300 merge commit `a030e1f` と記録）:
  - 検証した内容: `rcd_04_product_e2e.rs` 5 scenario（Comp Transition・MIDI Phrase・Recovery Review・Recovery guards・Stale+read-only）。`cargo test -p synvelis-app --test rcd_04_product_e2e` — 5 tests pass。canonical mise lanes 全 exit 0。
  - 結果: `mise run test` 初回で `synvelis-library` webdav test が os error 10053 で 5 件 flake → 単独再実行 42/42 pass・全体再実行 pass。実 Window/OS GUI/device/OS FS/実 RT sample/AudioLens f64 は NotRun/Unsupported（synvelis#114・synvelis#126・ruviform#294 deferred）。
- RT engine comp PCM（PR #319 merge commit `ff33ffb` と記録）:
  - 検証した内容: `rcd_04_rt_comp_pcm.rs` 2 scenario で legacy `HeadlessDriver`/semantic `RoutingEngine`/offline `export` の 3 実装経路が全 sample bit-exact 一致・`PlayReport.crc` 一致・fresh lower+prepare+run で crc/pcm 再現（determinism）。`cargo test -p synvelis-render --test rcd_04_rt_comp_pcm` — 2 tests pass。stereo 48 kHz fixture・6 region・3 transition・tempo event（2880→90BPM、tick→frame 非一様 100/3 f/t）で `[0,104000)` 照合、plan-derived anchor で 13 probe frame 固定。Loop/SimpleStretch で legacy explicit-skip 沈黙再生 vs semantic/export build 全体拒否の意図された fail-closed 挙動差を記録。
  - 結果: `synvelis-player/src` への補修不要（gap は実装欠陥ではなく検証不在）。
- MIDI undo stable-ID（PR #374 merge commit `4946a59` と記録）:
  - 検証した内容: `rcd_04_midi_phrase_boundary` 2 test（wire 列手導出完全一致 + ledger 走査）・`rcd_04_comp_cas` 6 test（+1: comp 切替 CAS/undo/redo + raw 不変）・`rcd_04_vcs_stable_id` 4 test（save→saved-HEAD snapshot→comp_key_sets 一致・compare_snapshots Equal・transition diff が `CompTransition` Added/Removed のみ `comp-transition-<32hex>` stable ID・apply_selected fail-closed・restore_as_copy receipt 計上）。
  - 結果: `cargo test` 各 binary pass。canonical mise lanes 全 exit 0（test-msrv は rust 1.89.0 で workspace 全件）。review-protocol で P0–P2 なし、P3 指摘の実害あるものを修正。実 device/OS GUI・AudioLens f64・app GUI E2E・MIDI comp 永続化は範囲外（synvelis#126 待ち）。
- residual closeout: docs+evidence のみで AC 4 行を merge 済み実装（PR #219/#300/#319/#374）に 1:1 対応付け。`mise run fmt-check`/`prek run --all-files`/`test synvelis-project`/`test synvelis-recording`/`test synvelis-player`/`test synvelis-render`/`test-rt`/`test synvelis-app`（opt-in、355 tests・`rcd_04_product_e2e` 5 tests 全 pass）全 Pass。簿記 drift 6 file 修正（3 task status in-progress→done、3 impl status in-progress→completed、stale checkbox を事実整合）。外部 gate は NotRun/Deferred のまま。

## RCD-F02 Overdub・Pre-Punch Handle（synvelis#120、docs-only）

canonical 文書チェーン + slice 別 evidence の ADR 決定・検証記録を固定する docs-only slice。5 領域 ADR D1〜D5（`MonitorRoute::Overdub`・notice 再利用+発火条件拡張・journal kind=4 `HandleGrant`+`JournalSpan` 分類・既存 undo/verified prefix・session-scoped consent）を MADR 4.0.0 で記録。

- 実行した検証: `mise run fmt-check`・`prek run --all-files`・doc index `index plan`/`apply`・`sync_impl_index`・doc-status audit・type 別 lint・impl audit・ADR audit。
- 未実行の検証: cargo test/build/lint・実 GUI/実 OS/実 Device/実 Plugin 検証は実行していない（docs-only・POLICIES §5 比例原則）— `NotRun`/`NotApplicable`。
- graph router は `status: blocked`（pre-existing broken relation、slice 起因でない）。（synvelis#120）

## 横断メモ

- flake 観測: `synvelis-library/tests/webdav.rs::browse_cursor_pages_listing` の `os error 10053`（socket abort）と `synvelis-player/tests/sf03_l2.rs` の `memory allocation failed`/`STATUS_STACK_BUFFER_OVERRUN` は大規模並列実行時の一過性で、単独・workspace 再実行で緑。`test-msrv` の `crate synvelis_gateway required to be available in rlib format` は rust 1.89 build と 1.99 fixture test の並行 artifact race で単独再実行 Pass。
- 実 device run の分散: RCD-01 の clean run 成功（10/10 pass・1,323,000 frames・0 fault）と fault 観測 run は矛盾ではなく、実機の非決定性を分離記録したもの。観測された fault は抑制・再分類されていない。
- plan/task の status=in-progress vs IR completed の齟齬は残存しており、実装記録（IR）の実績を優先して読むのが正しい扱い。RCD-04 の merge 済み slice では簿記 drift が事実整合で修正済み。
- monitor-session-ux slice の experiment log は未作成だが、先行 slice で確立済みのため optional と判断した意図的な省略であり、検証の欠落ではない。

## 出処

synvelis#12、synvelis#115、synvelis#116、synvelis#117、synvelis#118、synvelis#120。関連: synvelis#84、synvelis#86、synvelis#114、synvelis#126、ruviform#251、ruviform#294、ruviform#565、ruviform#628、ruviform#629。
