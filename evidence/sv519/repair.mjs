// sv519 route_graph broken-relation 修復マニフェスト生成 + 適用検証
// findings_parsed.json (route_graph --json 由来) を入力に、
// file -> relation -> 旧値 -> { action: 'set', values: [...] } | { action: 'remove' }
// の修復表を repair-manifest.json に出力する。dry-run で検証のみも可能。
import fs from 'node:fs';
import path from 'node:path';

const S = 'https://github.com/scarecrow173/synvelis/issues/';
const R = 'https://github.com/scarecrow173/ruviform/issues/';
const A = 'https://github.com/scarecrow173/audio-lens/issues/';
const RUV_BLOB = 'https://github.com/scarecrow173/ruviform/blob/0f5b57372e9a43f84415f7964b2f9636774174f0/crates/ruviform-audio-core/src/device.rs';

// file|relation|oldValue -> new values (array) or null=remove
const MAP = new Map();
function put(file, relation, oldValue, values) {
  const key = `${file}|${relation}|${oldValue}`;
  if (MAP.has(key)) throw new Error(`dup key ${key}`);
  MAP.set(key, values);
}
function urls(base, nums) { return nums.map((n) => base + n); }
function s(...n) { return urls(S, n); }
function r(...n) { return urls(R, n); }
function a(...n) { return urls(A, n); }
const D = (p) => [p]; // doc path helper

// ============ depends-on (8) ============
put('docs/designs/issue-515-os-credential.md', 'depends-on', 'docs/impl/ir/sv261-headless-session-host.md', D('docs/impl/ir/autx-02-cli-adapter.md'));
put('docs/designs/rcd-01-input-adapter-binding-arm-monitor-fake-port.md', 'depends-on', 'docs/specs/vs-b-deterministic-mock-capture-engine.md', D('docs/specs/vs-b-rt-capture-mock-input-buffer-pool-spsc.md'));
put('docs/designs/rcd-01-input-adapter-binding-arm-monitor-fake-port.md', 'depends-on', 'docs/specs/vs-d-multi-track-recording-session.md', D('docs/specs/vs-d-recording-commit-e2e-idempotency-undo-reload.md'));
put('docs/discovery/issue-515-os-credential.md', 'depends-on', 'docs/impl/ir/sv261-headless-session-host.md', D('docs/impl/ir/autx-02-cli-adapter.md'));
put('docs/specs/issue-274-cli-script-analyze.md', 'depends-on', 'docs/specs/autx-03-headless-cli.md', D('docs/impl/ir/autx-03-headless-cli.md'));
put('docs/specs/issue-515-os-credential.md', 'depends-on', 'docs/impl/ir/sv261-headless-session-host.md', D('docs/impl/ir/autx-02-cli-adapter.md'));
put('docs/specs/pc-c4-drain-composition.md', 'depends-on', 'docs/specs/pc-c4-07-transport-observe.md', D('docs/specs/pc-c4-07-transport-mirror-observe.md'));
put('docs/specs/rcd-03-monitoring-punch-latency.md', 'depends-on', 'docs/specs/vs-b-deterministic-mock-capture-engine.md', D('docs/specs/vs-b-rt-capture-mock-input-buffer-pool-spsc.md'));

// ============ references (9) ============
for (const f of [
  'docs/impl/ir/issue-511-cb07-materialization.md',
  'docs/specs/issue-511-cb07-materialization.md',
  'docs/tasks/issue-511-cb07-materialization.md',
  'docs/test-specs/issue-511-cb07-materialization.md',
]) put(f, 'references', 'evidence/sv511/', D('evidence/sv511/verification.md'));
put('docs/specs/issue-517-render-branch-wiring.md', 'references', 'evidence/sv517/', D('evidence/sv517/verification.md'));
for (const f of ['docs/specs/issue-518-job-system.md', 'docs/test-specs/issue-518-job-system.md'])
  put(f, 'references', 'evidence/sv518/', D('evidence/sv518/verification.md'));
put('docs/specs/pc-c4-06-control-op-surface.md', 'references', 'docs/specs/pc-c4-03-param-observation-slice.md', D('docs/specs/pc-c4-03-param-observation.md'));
put('docs/specs/sf-04-slice2-controlsession-probe-trace.md', 'references', 'docs/designs/sf-03-slice5-session-engine-connect.md', D('docs/designs/sf-03-slice5-session-engine-connect-design.md'));

// ============ related (37, cb-09 は lease 外でスキップ) ============
put('docs/adr/pc-c3-control-trace-stage-evidence-freshness.md', 'related', 'docs/adr/pc-c1-recording-transaction-prepare-apply-finalize.md', D('docs/adr/pc-c1-commit-session-cas-orchestration-prepared-commit.md'));
put('docs/designs/issue-262-run-file-real-vm.md', 'related', 'docs/designs/autx-03-headless-cli.md', D('docs/impl/ir/autx-03-headless-cli.md'));
put('docs/designs/issue-274-cli-script-analyze.md', 'related', 'docs/designs/autx-03-headless-cli.md', D('docs/impl/ir/autx-03-headless-cli.md'));
put('docs/designs/rcd-01-input-adapter-binding-arm-monitor-fake-port.md', 'related', 'docs/designs/vs-b-capture-engine-design.md', D('docs/designs/vs-b-clock-gate-rt-capture-mock-spsc.md'));
for (const f of [
  'docs/discovery/autx-08-live-adapter-leg.md',
  'docs/impl/exp/autx-08-live-adapter-leg.md',
  'docs/impl/ir/autx-08-live-adapter-leg.md',
  'docs/specs/autx-08-live-adapter-leg.md',
  'docs/test-specs/autx-08-live-adapter-leg.md',
]) put(f, 'related', 'evidence/sv267/', D('evidence/sv267/live-adapter-leg.md'));
put('docs/discovery/issue-515-os-credential.md', 'related', 'docs/impl/ir/sv261-headless-session-host.md', D('docs/impl/ir/autx-02-cli-adapter.md'));
for (const f of [
  'docs/discovery/sv114-gui-option-b-recorder-wire.md',
  'docs/impl/exp/sv114-gui-option-b-recorder-wire.md',
  'docs/impl/ir/sv114-gui-option-b-recorder-wire.md',
  'docs/plans/sv114-gui-option-b-recorder-wire.md',
  'docs/specs/sv114-gui-option-b-recorder-wire.md',
  'docs/tasks/sv114-gui-option-b-recorder-wire.md',
  'docs/test-specs/sv114-gui-option-b-recorder-wire.md',
]) put(f, 'related', 'evidence/sv114/', D('evidence/sv114/recorder-wire-option-b-headless.md'));
for (const f of [
  'docs/discovery/sv488-lint-app-native.md',
  'docs/impl/exp/sv488-lint-app-native.md',
  'docs/impl/ir/sv488-lint-app-native.md',
]) put(f, 'related', 'evidence/sv488/', D('evidence/sv488/lint-app-native-verification.md'));
put('docs/impl/ir/alib-05-browser-import-ux.md', 'related', 'evidence/sv89/', D('evidence/sv89/acceptance-ledger.md'));
// docs/impl/ir/cb-09-diff-explorer.md の evidence/sv286/ は synvelis#286 占有のため未修復（例外として文書化）
put('docs/impl/ir/tim-04-plugin-state-snapshot.md', 'related', 'docs/specs/tim-03-process-boundary.md', D('docs/specs/tim03-process-boundary.md'));
put('docs/plans/signal-flow-design-plan.md', 'related', 'docs/plans/issue-365-impl-index-generation.md', D('docs/plans/issue-365-impl-index-generation-plan.md'));
put('docs/specs/issue-262-run-file-real-vm.md', 'related', 'docs/specs/autx-03-headless-cli.md', D('docs/impl/ir/autx-03-headless-cli.md'));
put('docs/specs/pc-c4-06-control-op-surface.md', 'related', 'docs/specs/pc-c4-03-param-observation-slice.md', D('docs/specs/pc-c4-03-param-observation.md'));
for (const f of ['docs/specs/pc-c4-drain-composition.md', 'docs/specs/pc-c4-observe-engine-wiring.md'])
  put(f, 'related', 'docs/specs/pc-c4-plugin-parameter-binding-frame-base-capability-gate.md', D('docs/specs/pc-c4-plugin-param-binding.md'));
const rcd01spec = 'docs/specs/rcd-01-recording-input-adapter-channel-binding-arm-monitor-fake-port-fail-stop-s.md';
put(rcd01spec, 'related', 'docs/specs/vs-b-deterministic-mock-capture-engine.md', D('docs/specs/vs-b-rt-capture-mock-input-buffer-pool-spsc.md'));
put(rcd01spec, 'related', 'docs/specs/vs-c-staging-writer-journal-recovery.md', D('docs/specs/vs-c-staging-writer-journal-crash-recovery-verified-media-range.md'));
put(rcd01spec, 'related', 'docs/specs/vs-d-multi-track-recording-session.md', D('docs/specs/vs-d-recording-commit-e2e-idempotency-undo-reload.md'));
put('docs/tasks/sv293-semantic-selection-integration.md', 'related', 'docs/tasks/rcp-04-missing-plugin-resolver.md', D('docs/impl/ir/rcp-04-missing-plugin-resolver.md'));
// rcd-03-monitor-gate / rcd-03-monitoring-punch-latency は実在しない名 → 実在の gate fixture matrix test-spec へ
put('docs/test-specs/rcd-03-monitor-engine.md', 'related', 'docs/test-specs/rcd-03-monitor-gate.md', D('docs/test-specs/rcd-03-monitor-punch-latency.md'));
put('docs/test-specs/rcd-03-monitor-engine.md', 'related', 'docs/test-specs/rcd-03-monitoring-punch-latency.md', D('docs/test-specs/rcd-03-monitor-punch-latency.md'));
put('docs/test-specs/rcd-03-monitor-punch.md', 'related', 'docs/test-specs/rcd-03-monitor-gate.md', D('docs/test-specs/rcd-03-monitor-punch-latency.md'));
put('docs/test-specs/signal-flow-design.md', 'related', 'docs/test-specs/sf-05-signal-flow-headless-hierarchy.md', D('docs/test-specs/sf-05-slice1-signal-flow-headless.md'));
put('docs/test-specs/signal-flow-design.md', 'related', 'docs/test-specs/sf-06-cross-gate-oracle-case-ledger.md', D('docs/test-specs/sf-06-cross-gate-oracle.md'));

// ============ source (37) ============
for (const f of [
  'docs/designs/rcd-01-input-adapter-binding-arm-monitor-fake-port.md',
  'docs/specs/rcd-01-recording-input-adapter-channel-binding-arm-monitor-fake-port-fail-stop-s.md',
]) put(f, 'source', 'ruviform crates/ruviform-audio-core/src/device.rs @ 0f5b573', D(RUV_BLOB));
put('docs/specs/vcs-02-semantic-diff-changeset.md', 'source', 'ISSUE-7', s(7));
put('docs/specs/vcs-02-semantic-diff-changeset.md', 'source', 'ISSUE-92', s(92));
put('docs/specs/vcs-02-semantic-diff-changeset.md', 'source', 'ISSUE-93', s(93));
for (const [rel, num] of [['ISSUE-7', 7], ['ISSUE-88', 88], ['ISSUE-92', 92], ['ISSUE-93', 93], ['ISSUE-94', 94]])
  put('docs/specs/vcs-03-git-read-provider.md', 'source', rel, s(num));
for (const [rel, num] of [['ISSUE-7', 7], ['ISSUE-92', 92], ['ISSUE-94', 94], ['ISSUE-95', 95]])
  put('docs/specs/vcs-04-backend-wiring.md', 'source', rel, s(num));
for (const [rel, num] of [['ISSUE-114', 114], ['ISSUE-7', 7], ['ISSUE-92', 92], ['ISSUE-93', 93], ['ISSUE-94', 94], ['ISSUE-95', 95], ['ISSUE-97', 97]])
  put('docs/specs/vcs-04-history-visual-diff-ui.md', 'source', rel, s(num));
for (const [rel, num] of [['ISSUE-114', 114], ['ISSUE-7', 7], ['ISSUE-95', 95]])
  put('docs/specs/vcs-04-native-mount.md', 'source', rel, s(num));
for (const [rel, num] of [['ISSUE-7', 7], ['ISSUE-92', 92], ['ISSUE-93', 93], ['ISSUE-94', 94], ['ISSUE-96', 96]])
  put('docs/specs/vcs-05-apply-restore-headless.md', 'source', rel, s(num));
put('docs/specs/vcs-05-monitoring-apply-surface.md', 'source', 'ISSUE-96', s(96));
put('docs/specs/vcs-05-monitoring-apply-surface.md', 'source', 'SPEC-vcs06-monitoring-matrix', D('docs/specs/vcs-06-monitoring-matrix.md'));
put('docs/specs/vcs-05-plugin-instance-apply.md', 'source', 'ISSUE-96', s(96));
put('docs/specs/vcs-06-fixture-fault-matrix.md', 'source', 'SPEC-vcs01-immutable-snapshot-resolver', D('docs/specs/vcs-01-immutable-snapshot-resolver.md'));
put('docs/specs/vcs-06-fixture-fault-matrix.md', 'source', 'SPEC-vcs02-semantic-diff-changeset', D('docs/specs/vcs-02-semantic-diff-changeset.md'));
put('docs/specs/vcs-06-fixture-fault-matrix.md', 'source', 'SPEC-vcs03-git-read-provider', D('docs/specs/vcs-03-git-read-provider.md'));
for (const f of ['docs/tasks/vcs-05-monitoring-apply-surface-task.md', 'docs/tasks/vcs-05-plugin-instance-apply-task.md'])
  put(f, 'source', 'ISSUE-96', s(96));

// ============ defers (143) ============
// --- issue-514 ADR ---
put('docs/adr/issue-514-session-engine-selection.md', 'defers', 'ExecutionSelection の live consumer（sv#295 表示面）', s(295));
put('docs/adr/issue-514-session-engine-selection.md', 'defers', 'SelectionInputs の production 供給経路（plugin env scan / TIM-04 state decl / artifact coverage）', s(293));
put('docs/adr/issue-514-session-engine-selection.md', 'defers', 'stateless PluginStateDecl の型表現（synvelis-player 契約 — 別 Issue）', s(530));
// --- automation-modulation-design ---
put('docs/designs/automation-modulation-design.md', 'defers', 'AUT-F01〜F05（#135〜139）— Multi-pass Takes・Envelope/Random Source・外部 MIDI CC Mapping・Audio-rate Modulation・明示 Bake', s(135, 136, 137, 138, 139));
put('docs/designs/automation-modulation-design.md', 'defers', 'Touch Return Time/Shape の最終既定値・曲線 Decimation 許容誤差・TimeBase/Plugin 精度の数値閾値（Issue #6 §H/§9 の PoC 後決定事項）', s(6));
put('docs/designs/automation-modulation-design.md', 'defers', '実 GUI/実 OS/実 Plugin/実機の受入（#102/#114/#103 の別受入 Gate）', s(102, 114, 103));
// --- rcd-04 designs ---
put('docs/designs/rcd-04-comp-transition-midi-phrase-fixture.md', 'defers', 'MIDI comp selection の project 永続化 model（#126 event model 確定後）', s(126));
put('docs/designs/rcd-04-comp-transition-midi-phrase-fixture.md', 'defers', '実 RT engine（ruviform graph）との sample 一致（後続 integration slice）', r(294));
put('docs/designs/rcd-04-product-e2e-design.md', 'defers', 'MIDI comp selection の project 永続化 model（synvelis#126 event model 確定後）', s(126));
put('docs/designs/rcd-04-product-e2e-design.md', 'defers', '実 RT engine sample 一致・AudioLens native-f64 consumer gate（Ruviform #294）', r(294));
put('docs/designs/rcd-04-product-e2e-design.md', 'defers', '実 Window/OS GUI・実 device・実 plugin を要する product E2E（#114 系 GUI slice）', s(114));
put('docs/designs/rcd-04-rt-engine-comp-pcm-design.md', 'defers', '実 device/OS GUI・AudioLens native-f64 consumer 経路の PCM 照合（#114・Ruviform #294 待ち）', [...s(114), ...r(294)]);
// --- tim-02 designs ---
put('docs/designs/tim-02-player-midi-rt-offline.md', 'defers', 'MidiNote への channel 追加（synvelis-project 側 model 変更・別 slice）', s(126));
put('docs/designs/tim-02-player-midi-rt-offline.md', 'defers', 'Panic-and-continue（transport 継続しつつ発音のみ停止）— upstream に専用 command がなく、Stopped 経由の Stop のみが release-without-retrigger を表現できる', null);
put('docs/designs/tim-02-player-midi-rt-offline.md', 'defers', 'Seek 時の controller state chase — upstream `chase_span_events` の deferred contract', null);
put('docs/designs/tim-02-player-midi-rt-offline.md', 'defers', '実 MIDI Backend OS 別・実 Device/Plugin 検証（Ruviform #253 の責務）', r(253));
put('docs/designs/tim-02-smf-format01-core.md', 'defers', 'SMPTE Division・MIDI 2.0/MPE（#129/#130 future・明示 Unsupported）', s(129, 130));
put('docs/designs/tim-02-smf-format01-core.md', 'defers', 'synvelis-player の MIDI RT/Offline 結合・Loop/Seek/Stop・RT callback（後続 slice）', D('docs/specs/tim-02-player-midi-rt-offline.md'));
put('docs/designs/tim-02-smf-format01-core.md', 'defers', '実 MIDI Backend OS 別・実 Device/Plugin 検証（Ruviform #253 の責務）', r(253));
// --- vcs-01 design ---
put('docs/designs/vcs-01-immutable-snapshot-resolver-design.md', 'defers', 'Plugin opaque state bytes の実 probe（plugin host 未接続）', s(97));
put('docs/designs/vcs-01-immutable-snapshot-resolver-design.md', 'defers', 'VCS-02 Semantic Diff ChangeSet（synvelis#93）', s(93));
put('docs/designs/vcs-01-immutable-snapshot-resolver-design.md', 'defers', 'VCS-03 Git Read Provider 実装（synvelis#94）', s(94));
put('docs/designs/vcs-01-immutable-snapshot-resolver-design.md', 'defers', '実 Git/実 OS/実 plugin host での検証（VCS-06 #97）', s(97));
// --- autx impl/exp ---
const AUTX_LANE = [...s(114), ...s(267)];
put('docs/impl/exp/autx-01-action-catalog.md', 'defers', '実 ScriptApiModel（synvelis#271）照合・Script Action（synvelis#272）・CLI surface（synvelis#262）・実 GPUI/OS lane（#114/#267）', [...s(271), ...s(272), ...s(262), ...AUTX_LANE]);
put('docs/impl/exp/autx-01-f01-has-executor.md', 'defers', 'automation_ui::ActionRow.executable への executor verdict 反映（#266 view 契約は別 Owner の Deferred 判断）', s(266));
put('docs/impl/exp/autx-01-f01-has-executor.md', 'defers', '実 ScriptApiModel（synvelis#271）照合・Script Action（synvelis#272）・CLI surface（synvelis#262）・実 GPUI/OS lane（#114/#267）', [...s(271), ...s(272), ...s(262), ...AUTX_LANE]);
put('docs/impl/exp/autx-07-gui-views.md', 'defers', 'assets/themes への status-* token 値追加（範囲外）', null);
put('docs/impl/exp/autx-07-gui-views.md', 'defers', 'Script Editor 同一 source 照合（synvelis#274）', s(274));
put('docs/impl/exp/autx-07-gui-views.md', 'defers', 'synvelis-automation live adapter 接続（#264）', s(264));
put('docs/impl/exp/autx-07-gui-views.md', 'defers', '実 GPUI/実 OS lane（synvelis#114/#267）', AUTX_LANE);
put('docs/impl/exp/autx-07-theme-parity.md', 'defers', 'Script Editor surface 本体・Analyzer 実装（synvelis#274 Owner）', s(274));
put('docs/impl/exp/autx-07-theme-parity.md', 'defers', 'synvelis-automation live adapter 接続（#264）', s(264));
put('docs/impl/exp/autx-07-theme-parity.md', 'defers', '実 GPUI/実 OS lane（synvelis#114/#267）', AUTX_LANE);
// --- autx impl/ir ---
put('docs/impl/ir/automation-modulation.md', 'defers', '実 GUI/実 OS/実 Plugin/実機の受入 — #102/#114/#103 の別受入 Gate', s(102, 114, 103));
put('docs/impl/ir/automation-modulation.md', 'defers', '製品 Automation/Modulation/Parameter Control の実装 — PC-C1〜06 #98〜103・CTL-01/02 #121/#122・AUT-01〜04 #131〜134', s(98, 99, 100, 101, 102, 103, 121, 122, 131, 132, 133, 134));
put('docs/impl/ir/autx-01-action-catalog.md', 'defers', 'CLI surface 本体（synvelis#262 のリース）— 本スライスは consumer-neutral fixture のみ供給', s(262));
put('docs/impl/ir/autx-01-action-catalog.md', 'defers', 'Script Action 登録・script→catalog 投影（synvelis#272）', s(272));
put('docs/impl/ir/autx-01-action-catalog.md', 'defers', '実 GPUI render・実 OS・live Gateway adapter 経由の実行（synvelis#114/#267 別 Gate）', AUTX_LANE);
put('docs/impl/ir/autx-01-action-catalog.md', 'defers', '実 ScriptApiModel（synvelis#271）との Capability ID/Version/Schema/Availability 同一性照合', s(271));
put('docs/impl/ir/autx-01-f01-has-executor.md', 'defers', 'automation_ui::ActionRow.executable への executor verdict 反映（#266 view 契約は別 Owner — catalog 側の executor field を正すまでが本 slice）', s(266));
put('docs/impl/ir/autx-01-f01-has-executor.md', 'defers', 'GUI/実 OS/実機 lane・CLI surface 本体（#262）・実 ScriptApiModel 照合（#271）', [...AUTX_LANE, ...s(262), ...s(271)]);
put('docs/impl/ir/autx-07-gui-views.md', 'defers', 'assets/themes/*.dtcg.json への status-* token 値の追加（assets は本スライスの編集範囲外 — vocabulary は synvelis-theme::status が正本）', null);
put('docs/impl/ir/autx-07-gui-views.md', 'defers', 'Script Editor/Diagnostics/Completion との同一 Capability/Schema/Availability/Target source 照合（synvelis#274 Owner）', s(274));
put('docs/impl/ir/autx-07-gui-views.md', 'defers', 'synvelis-automation crate 統合後の live adapter 接続（#264 別 Worker リース）', s(264));
put('docs/impl/ir/autx-07-gui-views.md', 'defers', '実 GPUI/実 OS 入力・Pixel・Native Plugin Dock・IME/DPI/2Window/WindowClose/Undo/Save/Reload の実機受入（synvelis#114/#267 別 Gate）', AUTX_LANE);
put('docs/impl/ir/autx-07-theme-parity.md', 'defers', 'Script Editor surface 本体の実装変更（synvelis#274 Owner — 本 slice は表示投影の parity 照合のみ）', s(274));
put('docs/impl/ir/autx-07-theme-parity.md', 'defers', 'synvelis-automation live adapter 接続（SV266-NR-04・#264）・composition mount 配線（SV266-NR-05）・gpui-test scenario（SV266-NR-06）', s(264));
put('docs/impl/ir/autx-07-theme-parity.md', 'defers', '実 GPUI/実 OS 入力・Pixel・IME/DPI/2Window/WindowClose の実機受入（synvelis#114/#267 別 Gate — SV266-NR-01 NotRun）', AUTX_LANE);
// --- pc-c4 impl/ir ---
put('docs/impl/ir/pc-c4-observe-engine-wiring.md', 'defers', 'project.new/open 後の engine 再 attach（synvelis#106 の責務）', s(106));
put('docs/impl/ir/pc-c4-observe-engine-wiring.md', 'defers', '実 RT thread / OS timer への run 周期供給（caller/deterministic driver が供給する既存契約のまま）', null);
put('docs/impl/ir/pc-c4-observe-engine-wiring.md', 'defers', '実 VST3/AU/CLAP・OS backend・実 GUI・AudioLens/Ruviform consumer 照合（synvelis#101 残件・外部 gate）', s(101));
put('docs/impl/ir/pc-c4-webview-composition-synvelis-rs-engine-attach.md', 'defers', 'project.new/open 後の engine 再 attach（synvelis#106 の責務）', s(106));
put('docs/impl/ir/pc-c4-webview-composition-synvelis-rs-engine-attach.md', 'defers', 'synvelis-native bin の共有 helper への呼出側移行（後続 slice）', null);
put('docs/impl/ir/pc-c4-webview-composition-synvelis-rs-engine-attach.md', 'defers', '実 plugin factory（ruviform#253 gate）・実 GUI（synvelis#114）・wire-schema L1 結合検証（synvelis#101 残件）', [...r(253), ...s(114), ...s(101)]);
put('docs/impl/ir/pc-c5-04-recording-status.md', 'defers', 'UI-C5-05 external GUI acceptance（synvelis#114 / ruviform#313 / ruviform#314）— NotRun で後続 gate に委譲', [...s(114), ...r(313), ...r(314)]);
// --- rcd-01 impl/ir ---
put('docs/impl/ir/rcd-01-audio-device-headless-slice-fake-port-streamwriter-fixture-recording-stat.md', 'defers', 'cross-clock multi-device（#119）', s(119));
put('docs/impl/ir/rcd-01-audio-device-headless-slice-fake-port-streamwriter-fixture-recording-stat.md', 'defers', '実 Device backend・OS 別ドライバ・実機受入（ruviform#251 待ち、別 Gate）', r(251));
put('docs/impl/ir/rcd-01-audio-device-headless-slice-fake-port-streamwriter-fixture-recording-stat.md', 'defers', '録音 command/UI（#114 / SV-GUI-05）', s(114));
put('docs/impl/ir/rcd-01-real-backend-readiness-slice.md', 'defers', 'cpal WASAPI adapter の IAudioClock::GetPosition device frame position 破棄（ruviform#565 起票済み）', r(565));
put('docs/impl/ir/rcd-01-real-backend-readiness-slice.md', 'defers', '実機 gate 行（実録音受入・Raw capture fidelity 実測・multi-device 実機相関・xrun/removal/長時間・証拠リンク）— Issue #115 後続 slice', s(115));
put('docs/impl/ir/rcd-01-real-input-coverage.md', 'defers', 'PCM fidelity deterministic source（OS loopback 等）— deterministic source 不在のため NotRun 維持', null);
put('docs/impl/ir/rcd-01-real-input-coverage.md', 'defers', 'physical USB 抜去・2ch 超 MultiChannel・cross-clock multi-device（#119）・macOS/Linux lane（ruviform#251 Stage M/L）・製品 E2E（#114）— 本 slice 対象外', [...s(119), ...r(251), ...s(114)]);
// --- rcd-04 impl/ir ---
put('docs/impl/ir/rcd-04-product-e2e-slice.md', 'defers', 'MIDI comp selection の project 永続化 model（synvelis#126）', s(126));
put('docs/impl/ir/rcd-04-product-e2e-slice.md', 'defers', '実 RT engine sample 一致・AudioLens native-f64 consumer gate（Ruviform #294）', r(294));
put('docs/impl/ir/rcd-04-product-e2e-slice.md', 'defers', '実 Window/OS GUI・実 device・実 plugin を要する product E2E（#114 系 GUI slice）', s(114));
// --- sv-script-03 impl/ir ---
put('docs/impl/ir/sv-script-03-workflow-plan.md', 'defers', 'ruviform#559', r(559));
put('docs/impl/ir/sv-script-03-workflow-plan.md', 'defers', 'synvelis#262', s(262));
put('docs/impl/ir/sv-script-03-workflow-plan.md', 'defers', 'synvelis#265', s(265));
put('docs/impl/ir/sv-script-03-workflow-plan.md', 'defers', 'synvelis#267', s(267));
// --- syn-io-02 impl/ir ---
put('docs/impl/ir/syn-io-02-head-cas-e2e.md', 'defers', 'macOS/Linux での実 2-process/HEAD.lock 実走 — portable code のため CI 実走見込み、merge 後の対象 SHA の CI 証拠で確認（NotRun）', null);
put('docs/impl/ir/syn-io-02-head-cas-e2e.md', 'defers', '電源断・実 durability fault・stale break の process 間実測 — 外部 gate（NotRun）', null);
put('docs/impl/ir/syn-io-02-head-cas-e2e.md', 'defers', '非協調 writer への強制排他・kernel file lock 化 — ruviform 境界上の別 Issue 事項', null);
put('docs/impl/ir/syn-io-02-os-l1-evidence.md', 'defers', 'Ruviform #305/#294/#306・Synvelis #28/#294・AudioLens #143 の統合・受入関係事項', [...r(305), ...r(294), ...r(306), ...s(28), ...s(294), ...a(143)]);
put('docs/impl/ir/syn-io-02-os-l1-evidence.md', 'defers', 'Windows NTFS 実機 Case・物理電断・Network/Cloud FS（Required/Deferred・NotRun）', null);
put('docs/impl/ir/syn-io-02-os-l1-evidence.md', 'defers', '対象 SHA c0b000b3 の ci.yml run（NotRun pending — 次回 daily-ci/手動 dispatch）', null);
put('docs/impl/ir/syn-io-02-windows-real-fs.md', 'defers', 'Import↔Save 結合 Case — sv#86/#91 の責務（範囲外）', s(86, 91));
put('docs/impl/ir/syn-io-02-windows-real-fs.md', 'defers', 'Issue #84 の他残ゲート行（対象 SHA CI run・upstream/consumer gate）— tracker として open 継続', s(84));
put('docs/impl/ir/syn-io-02-windows-real-fs.md', 'defers', 'macOS/Linux 実機 real-FS — 各 OS 実機環境での同一 lane 実走（Windows/NTFS 観測値を一般化しない）', null);
put('docs/impl/ir/syn-io-02-windows-real-fs.md', 'defers', '真の power-loss（電源断相当）実測 — 非破壊的に自動化不能、process kill は dirty page を保持するため代替不可（NotRun）', null);
// --- tim-04 impl/ir ---
put('docs/impl/ir/tim-04-plugin-state-snapshot.md', 'defers', '実 VST3/AU/CLAP plugin での state save/restore・実 OS filesystem atomicity・GUI 統合（NotRun/Unsupported — headless mock slice の範囲外）', s(128));
// --- vcs impl/ir ---
put('docs/impl/ir/vcs-01-immutable-snapshot-resolver.md', 'defers', 'VCS-02 Semantic Diff ChangeSet（Issue #93）', s(93));
put('docs/impl/ir/vcs-01-immutable-snapshot-resolver.md', 'defers', 'VCS-03 Git Read Provider（Issue #94）', s(94));
put('docs/impl/ir/vcs-01-immutable-snapshot-resolver.md', 'defers', '実 Git remote / 実 OS filesystem atomicity / GUI / 実 plugin host probe（VCS-04/05/06, Issue #97）', s(97));
put('docs/impl/ir/vcs-02-semantic-diff-engine.md', 'defers', 'lineage schema（Issue #7 で将来の Project schema extension へ deferred）', s(7));
put('docs/impl/ir/vcs-02-semantic-diff-engine.md', 'defers', 'VCS-03 Git Read Provider（Issue #94）', s(94));
put('docs/impl/ir/vcs-02-semantic-diff-engine.md', 'defers', 'VCS-04 History/Visual Diff UI（Issue #95）', s(95));
put('docs/impl/ir/vcs-02-semantic-diff-engine.md', 'defers', 'VCS-05 Apply/Restore', s(96));
put('docs/impl/ir/vcs-02-semantic-diff-engine.md', 'defers', '実 OS filesystem atomicity / 実 Git remote/provider / GUI / 実 plugin host probe（Issue #97）', s(97));
put('docs/impl/ir/vcs-03-git-read-provider.md', 'defers', 'remote LFS fetch（local LFS object store のみ解決）', null);
put('docs/impl/ir/vcs-03-git-read-provider.md', 'defers', 'VCS-04 History/Visual Diff UI（Issue #95）', s(95));
put('docs/impl/ir/vcs-03-git-read-provider.md', 'defers', 'VCS-05 Safe Apply/Restore（Issue #96）', s(96));
put('docs/impl/ir/vcs-03-git-read-provider.md', 'defers', '実 OS permission matrix・real remote fetch・auth/credential 実経路・GC/objects 欠損の実 filesystem 条件・GUI・RT 性能保証（Issue #97）', s(97));
put('docs/impl/ir/vcs-05-safe-apply-selected-change-restore-as-copy-headless.md', 'defers', 'cross-consumer validation（Issue #294）', s(294));
put('docs/impl/ir/vcs-05-safe-apply-selected-change-restore-as-copy-headless.md', 'defers', 'VCS-04 History/Visual Diff UI（Issue #95）', s(95));
put('docs/impl/ir/vcs-05-safe-apply-selected-change-restore-as-copy-headless.md', 'defers', 'VCS-06 Windows real FS/provider/plugin E2E・fault acceptance（Issue #97）', s(97));
put('docs/impl/ir/vcs-06-evidence-completion.md', 'defers', '*-REAL-* 行・実 GUI/実 OS/実 provider/consumer/RT — 外部 gate 残存', s(97));
put('docs/impl/ir/vcs-06-evidence-completion.md', 'defers', 'docs/*/README.md index 再生成（同上・対象外）', null);
put('docs/impl/ir/vcs-06-evidence-completion.md', 'defers', 'ledger 行追加（VCS-RESTORE-08・VCS-COMPAT-03/04 — ledger 占有のため Coordinator 後続 slice で統合）', null);
put('docs/impl/ir/vcs-06-monitoring-matrix-slice.md', 'defers', 'monitoring kind の apply surface 追加（transplant payload・preflight 設計を要する別 slice）', D('docs/specs/vcs-05-monitoring-apply-surface.md'));
put('docs/impl/ir/vcs-06-monitoring-matrix-slice.md', 'defers', '実 GUI/実 OS/実 provider/実 plugin host/RT/consumer lane（`*-REAL-*`・COMPAT 行 — 外部 gate）', s(97));
put('docs/impl/ir/vcs-06-real-git-fs-fault.md', 'defers', 'POSIX 側 FS fault 行 — share-mode/readonly 意味論が異なる別 lane（NotRun）', s(97));
put('docs/impl/ir/vcs-06-real-git-fs-fault.md', 'defers', '電源断/物理 fault・実 OS crash durability・実 GUI/plugin host/RT/consumer — 外部 gate（NotRun）', s(97));
put('docs/impl/ir/vcs-06-real-git-fs-fault.md', 'defers', '認証付き http/ssh remote・実 provider E2E・OS perms matrix — 実ネットワーク/認証情報が必要（NotRun）', s(97));
put('docs/impl/ir/vcs-06-real-git-fs-fault.md', 'defers', '本 branch の新規行に対する ci.yml covering run — merge 後の次回 dispatch 待ち', null);
put('docs/impl/ir/vcs-06-ui-headless-matrix-slice.md', 'defers', '実 GPUI two-window/IME/keyboard/a11y 証拠（synvelis#114）', s(114));
put('docs/impl/ir/vcs-06-ui-headless-matrix-slice.md', 'defers', '実 OS window / native dock（OS matrix lane）', s(114));
put('docs/impl/ir/vcs-06-variant-matrix-slice.md', 'defers', 'restore_as_copy の variant_sets defect 修復（fork_project nested identity 再採番 vs validate 規則見直し — synvelis#458）', s(458));
put('docs/impl/ir/vcs-06-variant-matrix-slice.md', 'defers', 'variant diff adapter 本体（ObjectKind variant kind・raw_kind_map・DIFF_POLICY_VERSION・apply surface — synvelis#457）', s(457));
put('docs/impl/ir/vcs-06-variant-matrix-slice.md', 'defers', '実 GUI/実 OS/実 provider/実 plugin host/RT/consumer lane（*-REAL-*・COMPAT 行 — 外部 gate）', s(97));
// --- pc-c3/pc-c4/sf-03/tim-02/vcs-01 specs ---
put('docs/specs/pc-c3-05-rt-trace-adapter.md', 'defers', 'synvelis-workspace ControlSession 接続・UI 表示・2 Inspector 二重起動防止（後続 integration slice）', s(102));
put('docs/specs/pc-c3-05-rt-trace-adapter.md', 'defers', 'HeadlessDriver/AttachedNode への adapter 搭載・transport control 配線（後続 integration slice）', s(102));
put('docs/specs/pc-c3-control-trace-projection.md', 'defers', 'synvelis-workspace ControlSession 接続・single drain owner・2 Inspector 二重起動防止（後続 integration slice）', s(102));
put('docs/specs/pc-c3-control-trace-projection.md', 'defers', 'TRACE-C3-05 bounded history / no-RT alloc-block / 実計測 RT adapter（synvelis-player 後続 slice）', null);
put('docs/specs/pc-c4-drain-composition.md', 'defers', 'L1 wire 検証・実 VST3/OS backend・実 GUI・AudioLens/Ruviform consumer 照合（synvelis#101 残件・後続 slice）', s(101));
put('docs/specs/pc-c4-observe-engine-wiring.md', 'defers', 'project.new/open 後の engine 再 attach（synvelis#106 の責務）', s(106));
put('docs/specs/pc-c4-observe-engine-wiring.md', 'defers', '実 RT thread / OS timer への run 周期供給（caller/deterministic driver が供給する既存契約のまま）', null);
put('docs/specs/pc-c4-observe-engine-wiring.md', 'defers', '実 VST3/AU/CLAP・OS backend・実 GUI・AudioLens/Ruviform consumer 照合（synvelis#101 残件・外部 gate）', s(101));
put('docs/specs/pc-c4-webview-composition.md', 'defers', 'project.new/open 後の engine 再 attach（synvelis#106 の責務）', s(106));
put('docs/specs/pc-c4-webview-composition.md', 'defers', 'synvelis-native bin の共有 helper への呼出側移行（native 経路の回帰リスクを避けるため本 slice 対象外）', null);
put('docs/specs/pc-c4-webview-composition.md', 'defers', '実 plugin factory（ruviform#253 gate）・実 GUI（synvelis#114）・wire-schema L1 結合検証（synvelis#101 残件・後続 slice）', [...r(253), ...s(114), ...s(101)]);
put('docs/specs/sf-03-map-semantic-runtime-contract.md', 'defers', 'SF-ACT-05〜09 の実機部分（RT latency/underrun・実 device epoch restart・実 OS/実デバイス/VST3 検証）', s(103));
put('docs/specs/sf-03-map-semantic-runtime-contract.md', 'defers', 'SF-ACT-06 残件（automation→TimedParamEvent lowering 配線・Fast Control active-generation gate・GUI）', D('docs/specs/sf-03-slice6-act06-control-surface.md'));
put('docs/specs/sf-03-map-semantic-runtime-contract.md', 'defers', 'SF-ACT-07 Partial Preview vs Export fail-closed の mode 別 policy 分岐', null);
put('docs/specs/sf-03-map-semantic-runtime-contract.md', 'defers', 'SF-ACT-08 Ruviform RT/render/AudioLens 側の受入証拠', r(294));
put('docs/specs/sf-03-map-semantic-runtime-contract.md', 'defers', 'SF-ACT-09 device config-only change の compile 判定・性能 baseline', null);
put('docs/specs/tim-02-player-midi-rt-offline.md', 'defers', 'AudioLens Native f64 非回帰（#294 別 Gate）', r(294));
put('docs/specs/tim-02-player-midi-rt-offline.md', 'defers', 'MidiNote への channel 追加（synvelis-project 側 model 変更・別 slice）', s(126));
put('docs/specs/tim-02-player-midi-rt-offline.md', 'defers', 'SMPTE Division・MIDI 2.0/MPE（#129/#130 future・明示 Unsupported）', s(129, 130));
put('docs/specs/tim-02-player-midi-rt-offline.md', 'defers', '実 MIDI Backend OS 別・実 Device/Plugin 検証（Ruviform #253 の責務）', r(253));
put('docs/specs/tim-02-ppqn-smf-cross-golden.md', 'defers', 'AudioLens Native f64 非回帰（#294 別 Gate）', r(294));
put('docs/specs/tim-02-ppqn-smf-cross-golden.md', 'defers', 'SMPTE Division・MIDI 2.0/MPE・Format 2（#129/#130 future・明示 Unsupported）', s(129, 130));
put('docs/specs/tim-02-ppqn-smf-cross-golden.md', 'defers', 'synvelis-player の MIDI RT/Offline 結合・実機 MIDI timing（別 slice・NotRun）', D('docs/specs/tim-02-player-midi-rt-offline.md'));
put('docs/specs/tim-02-ppqn-smf-cross-golden.md', 'defers', '実 MIDI Backend OS 別・実 Device/Plugin 検証（Ruviform #253 の責務）', r(253));
put('docs/specs/tim-02-smf-format01-import-export.md', 'defers', 'AudioLens Native f64 非回帰（#294 別 Gate）', r(294));
put('docs/specs/tim-02-smf-format01-import-export.md', 'defers', 'SMPTE Division・MIDI 2.0/MPE（#129/#130 future・明示 Unsupported）', s(129, 130));
put('docs/specs/tim-02-smf-format01-import-export.md', 'defers', 'synvelis-player の MIDI RT/Offline 結合・Loop/Seek/Stop・RT callback（後続 slice）', D('docs/specs/tim-02-player-midi-rt-offline.md'));
put('docs/specs/tim-02-smf-format01-import-export.md', 'defers', '実 MIDI Backend OS 別・実 Device/Plugin 検証（Ruviform #253 の責務）', r(253));
put('docs/specs/vcs-01-immutable-snapshot-resolver.md', 'defers', 'Plugin opaque state bytes の実検証 probe（plugin host 未接続のため本 slice では Unknown 固定）', s(97));
put('docs/specs/vcs-01-immutable-snapshot-resolver.md', 'defers', 'VCS-02 Semantic Diff ChangeSet・Change Index・Comparison policy（synvelis#93）', s(93));
put('docs/specs/vcs-01-immutable-snapshot-resolver.md', 'defers', 'VCS-03 Git Read Provider の実装（synvelis#94 — 本 spec は VCS Bytes Port 境界のみ定義）', s(94));
put('docs/specs/vcs-01-immutable-snapshot-resolver.md', 'defers', '実 Git remote / 実 OS filesystem / 実 plugin host での検証（VCS-06 #97・OS 別 NotRun/Unsupported 記録）', s(97));
put('docs/test-specs/automation-modulation.md', 'defers', '実 Window/OS GUI・実 device・実 plugin を要する product E2E（#102/#114/#103 の別受入 Gate）', s(102, 114, 103));
put('docs/test-specs/vcs-01-snapshot-resolver-fixtures.md', 'defers', '実 Git/実 OS/実 plugin host での検証（VCS-06 #97）', s(97));

// ---- 検証: findings_parsed.json の全 234 件（cb-09 の 1 件を除く 233 件）が MAP にあること ----
const findings = JSON.parse(fs.readFileSync('evidence/sv519/findings_parsed.json', 'utf8'));
const SKIP = new Set(['docs/impl/ir/cb-09-diff-explorer.md|related|evidence/sv286/']);
const missing = [];
for (const f of findings) {
  const key = `${f.source}|${f.relation}|${f.value}`;
  if (SKIP.has(key)) continue;
  if (!MAP.has(key)) missing.push(key);
}
if (missing.length) {
  console.error('UNMAPPED findings:', missing.length);
  for (const m of missing) console.error(' ', m);
  process.exit(1);
}
// MAP に余分なエントリがないこと
const keys = new Set(findings.filter((f) => !SKIP.has(`${f.source}|${f.relation}|${f.value}`)).map((f) => `${f.source}|${f.relation}|${f.value}`));
const extra = [...MAP.keys()].filter((k) => !keys.has(k));
if (extra.length) {
  console.error('EXTRA map entries:', extra.length);
  for (const e of extra) console.error(' ', e);
  process.exit(1);
}

// manifest 出力（key は file|relation|value 形式で構築済みのため findings を再走査）
const byFile = {};
for (const f of findings) {
  const key = `${f.source}|${f.relation}|${f.value}`;
  if (SKIP.has(key)) continue;
  const values = MAP.get(key);
  (byFile[f.source] ??= {})[f.relation] ??= {};
  byFile[f.source][f.relation][f.value] = values === null ? { action: 'remove' } : { action: 'set', values };
}
fs.writeFileSync('evidence/sv519/repair-manifest.json', JSON.stringify(byFile, null, 2) + '\n');
console.log(`manifest entries: ${MAP.size} (findings ${findings.length} - skip 1)`);
console.log(`files touched: ${Object.keys(byFile).length}`);
