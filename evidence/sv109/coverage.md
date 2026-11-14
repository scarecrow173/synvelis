# SF-06 cross-gate headless fixture 被覆棚卸し (Issue #109 / synvelis#109)

SF-MODEL/VALID/ACT/PROBE の headless fixture 被覆を
`synvelis-project`/`synvelis-player`/`synvelis-render` の
`tests/` 全件へ写像した棚卸し結果。先行 ledger
（`evidence/reports/sf-06-cross-gate-ledger.md`、2026-11-10
計測）は SF-03/SF-04 slice 群の merge 前に書かれており T2
lane が `NotRun` だらけだったため、本ファイルが現行被覆の
正本となる。

凡例: ✅ headless 証拠あり（本 slice で `mise run test` 実測
green）/ ◐ 部分的 / ⛔ 外部 Gate（NotRun/Unsupported —
`external-gates.md` 参照）

## 計測環境（2026-11-15）

| 項目 | 値 |
| --- | --- |
| Synvelis SHA | `86715f9ac2761f9f275e885e0bbd7ee775d9633e`（origin/main base）+ 本 slice の test/docs commit（ledger 自体は同 commit 内。CI covering run は merge 後 SHA） |
| Ruviform resolved SHA | `fd1d4d5f4de57092b96ad1cd309b85fe47cbef38`（`Cargo.lock` git dep） |
| OS / CPU / FS | Windows 11 Pro 10.0.26300 / Intel Core i9-9900K / NTFS（C:） |
| Device + Plugin | なし（headless — audio device・native plugin host 未接続） |
| Toolchain | Rust `1.99.1`（mise 管理。CI pin は `1.99.0`）、edition 2024 |
| Project schema | v7 envelope（routing = schema v6 由来） |

## Lane 定義（先行 ledger と同一）

| Lane | 環境 | Pass にできるもの |
| --- | --- | --- |
| T0 headless (synvelis-project) | deterministic unit/integration test | ID/ref/schema migration/CAS/Undo/Diff・persisted 契約 |
| T1 generic graph (ruviform) | Ruviform offline fixture | Domain/Port/Channel/PDC/Publish queue — linked evidence のみ・本 slice では再計測しない |
| T2 headless E2E (synvelis-player/render) | lowering/activation/offline render/probe integration | PCM・source map・fault recovery・offline export |
| T3 real OS GUI/Device | Windows/macOS/Linux + device/plugin | 実 Render/Focus/latency — **全て NotRun** |
| T4 both consumers | Synvelis + AudioLens resolved SHA matrix | public API/precision/RT/perf — **全て NotRun** |

---

## SF-MODEL（Project & Legacy）

| caseId | 被覆 fixture | 判定 |
| --- | --- | --- |
| SF-MODEL-01 | `routing_sf01.rs`（master bootstrap・stable ID・save/reload・undo）・`sf06_oracle.rs`（invariant oracle） | ✅ T0 |
| SF-MODEL-02 | `sf06_oracle.rs`（chain/processor order invariant）・`routing_sf01.rs`（reload 後 ordering） | ✅ T0 |
| SF-MODEL-03 | `sf06_oracle.rs`（send↔connection 1:1・map key==id・corruption 検出）・`routing_sf01.rs`（orphan send reject）・`sf_02.rs`（dup temp ID） | ✅ T0 |
| SF-MODEL-04 | `routing_sf01.rs`（unresolved processor port — persisted 層の Unresolved 保持）・`sf03_l2.rs`（`MissingSource`/`PortUnresolved` fault） | ✅ T0+T2 headless / ⛔ runtime misroute 実測は T3 |
| SF-MODEL-05 | `sf06_oracle.rs`（v5→v6 migration oracle・決定性・HEAD 保護）・`routing_sf01.rs`（v5 load/reject） | ✅ T0（PCM golden は SF-COMPAT-01 側・⛔ T3/T4） |
| SF-MODEL-06 | `sf06_oracle.rs`（single DSP owner・master 一意 invariant）・`sf_05.rs`/`sf05_signal_flow.rs`（visual-only 操作の Project 不変） | ✅ T0+T2 headless / ⛔ probe 操作の Project 不変は SF-PROBE lane |
| SF-MODEL-07 | `routing_sf01.rs`・`sf06_oracle.rs` fault（referenced-track delete reject・状態不変） | ✅ T0（composite delete UX は SF-UX lane・⛔ T3） |

## SF-VALID（Semantic Validation）

| caseId | 被覆 fixture | 判定 |
| --- | --- | --- |
| SF-VALID-01 | `sf06_oracle.rs`（direction/role/send 1:1 invariant）・`sf_02.rs`（sidechain・pre/post send→return・approved channel map） | ✅ T0 |
| SF-VALID-02 | `sf_02.rs`（empty channel map reject・approved conversion policy）・`routing_sf01.rs`（empty map）・`sf_xr_01.rs`（異 layout channel map consent・atomic reject） | ✅ T0 / ◐ multiplicity/capability 網羅は部分的 |
| SF-VALID-03 | `sf06_oracle.rs`（2^14 mute×solo sweep — model 層）・**`sf06_send_path_pcm.rs::sf06_mute_solo_pcm_through_sends`（PCM 層: mute が main+send 双方を測定 0 にする）**・`sf03_l1.rs`（mute prune・disabled conn） | ✅ T0+T2（PCM 実測は本 slice で追加）/ ⛔ Raw capture bytes は SF-PROBE/COMPAT lane |
| SF-VALID-04 | `sf06_oracle.rs`（solo upstream supplier ∪ master 到達 downstream sweep）・**`sf06_send_path_pcm.rs::sf06_mute_solo_pcm_through_sends`（solo が send 経路へ波及）** | ✅ T0+T2（PCM 実測は本 slice で追加） |
| SF-VALID-05 | `routing_sf01.rs`（unresolved explicit）・`sf_02.rs`（typed rejects）・`sf03_l2.rs`（offline capability gate） | ✅ T0+T2 headless / ⛔ Direct Monitor・format/d-delay policy は T3 |
| SF-VALID-06 | `sf_02.rs`（`a_feedback_cycle_rejects_as_a_typed_cycle`）・`routing_sf01.rs`（strip graph non-cycle）・`sf06_oracle.rs`（fault cycle + invariant acyclicity） | ✅ T0（zero-delay reject のみ。declared delayed feedback は ⛔ T1/T2 policy 未実装） |
| SF-VALID-07 | `sf_02.rs`（stale revision・stale capability generation）・`sf06_oracle.rs` fault matrix | ✅ T0 / ⛔ multiwindow conflict は SF-UX/T3 |
| SF-VALID-08 | `sf_02.rs`（multi-edit intent・1 CAS/1 undo・preview 非破壊・receipt 決定性・dup temp・adopted gain seal・mixed edits・rejected intent 不変）・`sf06_oracle.rs` fault matrix・**`sf06_send_path_pcm.rs::sf06_graph_edit_pcm_undo_save_reload`（intent=1 revision=1 undo を PCM で検証）** | ✅ T0+T2（PCM/undo 復元は本 slice で追加） |

## SF-ACT（Lowering/Plan/Activation）

| caseId | 被覆 fixture | 判定 |
| --- | --- | --- |
| SF-ACT-01 | `sf03_l1.rs`（semantic lowering deterministic・semantic↔legacy PCM equivalence・two-track additive mix・send gain・pre/post tap 区別・processor chain gain PCM）・`sf03_render.rs`（offline export vs RT engine vs persisted model の same-revision source map 一致） | ✅ T2 headless（PCM golden 対 consumer は SF-COMPAT-01/05 側） |
| SF-ACT-02 | `sf03_l1.rs`（routing graph lowering・source map）・`sf03_l2.rs`（prepare/lower/publish fault taxonomy・last-good・reconcile drift・pending publish retry・QueueFull・offline capability）・`sf03_l3/l4/l6/l9`（plugin binding・rapid revision/restart・automation・activation/reconcile）・**`sf06_send_path_pcm.rs`（send/bus/return の PCM 実測: pre/post tap・bus 集約・mute/solo 波及）** | ✅ T2 headless / ⛔ PDC/latency 実測は render l2 系 + T3 |
| SF-ACT-03 | `sf_02.rs`（preview non-mutating・deterministic receipt）・`sf03_l2.rs`（stale revision/generation-scoped source map） | ✅ T0+T2 headless / ⛔ speculative BuildKey 照合の case-level 判定は別 gate |
| SF-ACT-04 | `sf03_l1.rs`（publish→active receipt 区別）・`sf03_l2.rs`（pending publish） | ✅ T2 headless / ⛔ RT process 実測は T3 |
| SF-ACT-05 | `sf03_l2.rs`（`PublishQueueFull`・last-good preservation） | ✅ T2 headless |
| SF-ACT-06 | `sf03_l3.rs`/`sf03_l9.rs`（rapid revision・device/plugin restart・`Superseded`/`DeviceEpochChanged` fault） | ✅ T2 headless / ⛔ 実 device epoch は T3 |
| SF-ACT-07 | `sf03_l6.rs`（automation tick on active generation — stale event が別 instance へ届かない） | ✅ T2 headless / ⛔ plugin 実機は T3 |
| SF-ACT-08 | `sf03_l2.rs`（missing source/plugin/port typed fault・fail-closed）・`sf03_render.rs`（同一 fault class の cross-consumer 一致） | ✅ T2 headless |
| SF-ACT-09 | `sf03_l4.rs`（name-only/undo equivalence — 不要 compile なし）・`sf03_render.rs`（same-revision fingerprint） | ✅ T2 headless |

## SF-PROBE（Probe Adapter）

| caseId | 被覆 fixture | 判定 |
| --- | --- | --- |
| SF-PROBE-01 | `sf04_project_probe.rs`（semantic point→port/boundary resolve・send tap・explicit port agreement・unknown/unmapped/unresolved・fader-slot requirement）・`sf04_probe_acceptance.rs`（entity 消失→Unresolved unbind・re-attach fresh bind — sv107 slice） | ✅ T0+T2 headless / ⛔ published source map の ACT 接続 case-level は別 gate |
| SF-PROBE-02 | `sf04_probe_adapter.rs`（probe off = observation silence・bypass/connection 非変更は lowering 契約） | ✅ T2 headless（sv107 slice 証拠） |
| SF-PROBE-03 | `sf04_probe_adapter.rs`（lifecycle・stale generation・moved point）・`sf04_s2_probe_trace_bridge.rs`（stale/unknown 拒否） | ✅ T2 headless |
| SF-PROBE-04 | `sf04_probe_adapter.rs`（generation swap cuts capture — 再自動 capture なし）・`sf04_s8_probe_faults.rs`（capture variant・fault status）・`sf04_s9_probe_discontinuity_declare.rs`（epoch boundary・segment cut） | ✅ T2 headless / ⛔ 実機 #308 fault injection は external-gates |
| SF-PROBE-05 | `sf04_s8_probe_faults.rs`（chunk-pool exhaustion・telemetry overflow・observation state 分離）・`sf04_s7_probe_compare.rs`（bounded evidence・empty observations） | ✅ T2 headless |
| SF-PROBE-06 | slice5 workspace lease（merge 済み）・`sf04_probe_acceptance.rs`（per-key 冪等・session 境界 — sv107 slice） | ✅ T2 headless / ⛔ 2 window/Inspector 実共有は SF-UX/T3 |
| SF-PROBE-07 | `sf04_s7_probe_compare.rs`（shared cursor alignment・partial coverage・epoch mismatch・unsubscription・missing runtime probe）・`sf04_s3_probe_discontinuity.rs`・`sf04_s9`（seek/stop/run 区別・full-channel error） | ✅ T2 headless / ⛔ RF#309 実測 alignment gate は external-gates |
| SF-PROBE-08 | `sf04_s8`（Unavailable/Unobserved/Unknown/Silence の状態分離） | ◐ headless は状態語彙分離まで。first-drop vs root-cause 判定・UI 分離は SF-UX/#109 gate |
| SF-PROBE-09 | `*_rt.rs`（`pc_c4_binding_rt`・`pc_c6_oracle_pl_rt`・`tim02_midi_rt`・`tim03_process_boundary_rt`・`trace_c3_05_rt` — RT no-alloc/no-block） | ✅ T2 headless RT / ⛔ AudioLens f64 consumer は T4・⛔ 実機長時間は T3 |

## 横断・consumer lane（棚卸しのみ — 本 slice の追加対象外）

| caseId | 被覆 fixture | 判定 |
| --- | --- | --- |
| SF-COMPAT-01 | `x_e2e.rs`（recorded playback export bit-exact・identity/revision/undo/save/reload）・`l2_e2e.rs`（`l2_01_master_export_reopen_pcm_match`・clip 境界/gap/mute）・`sf03_render.rs` | ◐ headless offline PCM 部分 ✅ / ⛔ Gate A 全体判定は別 gate |
| SF-COMPAT-04 | `l2_e2e.rs`（PDC realign・delay capture・preroll state semantics・destination policy・fault injection）・`rcd_04_rt_comp_pcm.rs`（RT engine comp PCM = offline export）・`tim04_plugin_state.rs`（exact/degraded evidence） | ◐ headless 部分 ✅ / ⛔ 実 device/ADC-DAC は T3 |
| SF-XR-01 | `sf_xr_01.rs`（player test — strip/send authority・serialization・undo・lowering・channel-map consent・atomic reject） | ✅ T0+T2 headless |
| SF-XR-02..05 | — | NotRun（#119/#126/#127/#128 依存 — 本 slice 範囲外） |
| SF-UX-01..08 | `sf_05*.rs`・`sf05_signal_flow.rs`（project/player/app 層の headless 部分） | ◐ headless 部分のみ / ⛔ 実 GUI は #114/T3 |
| SF-XG-01..05 | — | NotRun（GUI cross-product — #114 系） |

## 本 slice で追加した fixture

`crates/synvelis-player/tests/sf06_send_path_pcm.rs` — 先行
slice が lowering/source-map 層で検証した send/bus 経路の
**PCM 実測が欠落**していたため追加。解析 oracle
（`fixture_sample_f32` × dyadic gain）との bit 一致を要求する。

| test | case 対応 | 検証内容 |
| --- | --- | --- |
| `sf06_send_pcm_pre_vs_post_tap` | SF-ACT-02・SF-VALID-03/04 | fader 0.5 で pre tap=post-chain/pre-fader 信号・post tap=fader 出力を識別。master PCM = 1.125·x（誤 tap なら 0.875·x で fail） |
| `sf06_graph_edit_pcm_undo_save_reload` | SF-MODEL-01/03・SF-ACT-02・SF-VALID-08 | 1 intent = 1 revision = 1 undo を revision counter + PCM/CRC で検証。save→load 往復で PCM/CRC bit-for-bit 一致 |
| `sf06_mute_solo_pcm_through_sends` | SF-VALID-03/04・SF-ACT-02 | mute が main+send 双方を測定 0 に、solo が他 strip の main+send を除去しつつ soloed strip は両経路で鳴ることを PCM で確認 |

## #114 (GPUI Signal Flow Editor) Case 対応表

| headless case | #114 での対応検証 |
| --- | --- |
| `sf06_send_pcm_pre_vs_post_tap` | Editor の pre/post send 表示と実 tap の一致（表示≠実測の確認） |
| `sf06_graph_edit_pcm_undo_save_reload` | Editor gesture → undo/save/reload の PCM 維持 |
| `sf06_mute_solo_pcm_through_sends` | Editor mute/solo が send 経路へ波及するかの GUI 確認 |
| SF-PROBE-01..09 行 | probe 状態表示（Unavailable/Unobserved/Unknown）と実観測の一致・2 window 共有 |

headless Pass は #114 の Pass を構成しない。

## 外部提出分（NotRun/Deferred 対応表）

Issue #109 が要求するが本 slice では実行しない項目。全て
`external-gates.md` と併記。

| 項目 | 状態 | 委譲先 |
| --- | --- | --- |
| SF06-RCP-01（release candidate 判定） | **NotRun** | Gate 側 — 全 lane の証拠が揃ってから |
| EX-001〜020（#18 playback graph 受入） | **NotRun** | #18 の evidence を link 参照。Gate A/B/C 判定は別 gate |
| Gate A/B/C（DAW 横断構造/診断/control） | **NotRun** | SF-COMPAT-01..03 — 実機/consumer を含む判定 |
| Windows GUI-L2 / EXT-L3 | **NotRun** | #114・#176・実 device/plugin — `POLICIES.md` の Required 状態は capability 使用時に発生 |
| macOS/Linux GUI-L2/EXT-L3 | **Deferred** | `POLICIES.md` — 使用要件が発生するまで Deferred |
| Ruviform #219/#294（consumer RT/PCM/high-precision regression） | **NotRun** | T4 lane — AudioLens resolved SHA tuple は Gate 側の責務 |
| Ruviform #308（probe capture/rebind 実機 fault） | **NotRun** | 実機 fault injection — headless は契約レベルのみ |
| Ruviform #309（frame/clock/epoch/latency 実測 alignment） | **NotRun** | 実測 precision gate — headless compare は契約レベルのみ |
| AudioLens consumer evidence | **NotRun** | T4 — 別 consumer リポジトリの受入 |

## 判定ルール

- ✅ は「当該 SHA で `mise run test <package>` が green だった」
  ことのみを意味し、case 全体の受入 Pass ではない。
- `Pass*` 相当の部分被覆は ◐ で示し、残件を明記する。
- headless の成功を GUI/device/plugin/consumer の Pass に
  昇格しない。
