# PD-01〜07 Plugin Dock Case Definitions (synvelis#176 / SV-GPUI-02)

Date: 2026-11-15. Status: **frozen at definition level — no real-OS,
real-plugin, or native-dock lane was executed by this slice.** Every
Real OS / Real Plugin ABI / Native Dock row below is `NotRun`; the only
executed evidence is the headless semantic contract recorded in
`evidence/sv176/pd-headless-contract.md` (impl leg).

Source of truth for case identity: synvelis Issue #176, section
「2026-11-07 横断レビュー確定仕様 v2 — Plugin Native Dock acceptance
product cases」(実機製品ケース PD-01〜07) and the same section's
Matrix (OS/backend × plugin format × L0/L1 × Plugin UI Toolkit/Version
× Capability・実行コマンド・観測・Result). Verification intent:
`docs/test-specs/pd-01-07-plugin-dock.md` (Rules PD-FRZ-01〜07). Spec
anchor: `docs/specs/sv-gpui-02-plugin-dock-gate.md`.

Ruviform vocabulary source: `ruviform-ui-test` at Cargo.lock resolved
rev `ac10a2f7761c08db73aebd861c0a5c1f53f269b7` — exports
`PLUGIN_DOCK_SCENARIO` (`"rf631.plugin-dock"`), `PLUGIN_DOCK_SLOT_KEY`
(`"plugin.editor"`), `PluginArtifactEvidence`, `plugin_dock_view_spec`,
`plugin_dock_spec`. The `rf631.plugin-dock` 8-case vocabulary itself is
defined in `ruviform-plugin-dock-test` (upstream; synvelis references it
only, it is not a dependency).

## Evidence schema (frozen)

Each executed case row MUST carry:

| Field | Meaning |
| --- | --- |
| Case | `PD-01` … `PD-07` (product case id) + referenced `rf631.plugin-dock` case vocabulary |
| OS | `windows` / `macos` / `linux` (+ arch, X11/XWayland/Wayland where relevant) |
| plugin format | `vst3` / `clap` / `au` (per Issue #176 Matrix) |
| driver/backend/GPU | OS input driver, GPUI backend, GPU/adapter used |
| plugin artifact identity | `PluginArtifactEvidence` 6 dims — `path`, SHA-256 `sha256`, descriptor `plugin_id`, `name`, `vendor`, `version` (required whenever a real plugin binary ran; `none` while NotRun) |
| host/repo SHA | synvelis repo SHA + ruviform resolved SHA under test |
| Expected | observable expectation before the run |
| Observed | what was actually observed (empty while NotRun) |
| Result | `Pass` / `Fail` / `NotRun` / `Unsupported` (POLICIES.md §3 vocabulary) |
| Artifact | screenshot/video/log/receipt paths or run URLs |
| Owner | accepting owner (synvelis#176 / ruviform issue / CI) |

Evaluation lanes are recorded separately and never merged:

- **Headless/Simulated** — `FakeGuiTest` / `GpuiGuiTest`
  (`GpuiSimulated`) semantic evidence (OS-L1 semantic layer). This
  slice's only executed lane: `view-mount` contract conformance.
- **Real OS** — real GPUI window, real OS user input, focus, IME, DPI,
  pixel (GUI-L2; Windows-first per Issue #176 / POLICIES.md §2).
- **Real Plugin ABI / Native Dock** — real VST3/CLAP/AU binary through
  the production host into a real docked native editor surface
  (EXT-L3; `scarecrow173/ruviform#630` test native editor + `#631`
  real GPUI window runner).

macOS/Linux Real OS / Real Plugin lanes are `NotRun` by policy
(POLICIES.md §2: L2/L3 Deferred until use is required). Windows lanes
for features actually provided are Required; features not offered are
NotApplicable and are never demanded uniformly (Issue #176 AC
「OS/機能別Evidence」).

## `rf631.plugin-dock` shared vocabulary and synvelis runnability

The 8 upstream cases are the shared vocabulary PD evidence rows
reference. Synvelis-side runnability after the S1 headless-probe slice
(2026-11-25, pin `20dc41f7`): `synvelis-app` dev-deps now include
`ruviform-plugin-dock-test` + the ruviform#630 test-plugin crates, so
the probe cases execute headless against real plugin binaries — see
`s1-headless-probe.md`/`s1-headless-probe.jsonl` (at freeze time —
pin `ac10a2f7`, no new dependencies — the probe cases were
`NotRun`/Deferred pending that addition):

| rf631 case | Driver (upstream) | synvelis runnability / recorded verdict |
| --- | --- | --- |
| `view-mount` | `Fake` | **Runnable headless** — `plugin_dock_view_spec()` mounts on `FakeGuiTest`/`GpuiGuiTest` via the production composition shape; contract conformance only (see `pd-headless-contract.md`). Re-recorded `Pass` in the S1 `run_all` document |
| `artifact` | `PluginHostProbe` | `Pass` (S1) — real VST3 `test_gain` cdylib resolved + sha256/descriptor recorded in `plugin_artifact` |
| `host-lifecycle` | `PluginHostProbe` | `Pass` (S1) — Scan→Instantiate→Initialize→Activate→Destroy on the real binary |
| `editor-attach` | `PluginHostProbe` | `Unsupported` — `createView` returns null (test_gain ships no editor surface); observed live in the S1 run, recorded as technical non-support, not environment absence |
| `session-lifecycle` | `PluginHostProbe` | `Pass` (S1) — begin/busy/refused-open/`Failed{Unsupported}`/close/end/rebegin |
| `host-terminate` | `PluginHostProbe` | `Pass` (S1) — `Terminated{HostLost}` exactly once, `closed-concluded` |
| `open-or-degrade` | `PluginHostProbe` | `Pass` (S1) — double refusal, no `Degraded` event |
| `dock-pixels` | `PluginHostProbe` | `NotRun` — no `VisualCapture` capability on any synvelis adapter (also no docked surface exists) |

The probe cases are built with upstream `probe_spec`, which declares
`required = {}` — the probe honestly claims no capability, and
artifact presence gates inside the case body. The bodies drive
`InProcessHost`/`PluginEditorSessions` directly and do not consume
the semantic adapter for dock evidence; the `PluginHostProbe`
driver reports its own honest capability set. `plugin_dock_spec`
(required `{NativePluginDock, VisualCapture}`) is a `ruviform-ui-test`
helper for consumer-declared specs — upstream's own contract test
exercises it, but the `rf631.plugin-dock` probe cases do not use it.
Synvelis's contract test exercises `plugin_dock_spec` as a
spec-level gate only: it proves a spec that *does* require those
capabilities cannot run on semantic adapters — it does not model
the upstream probe-case mechanism.

## Lane status summary (freeze snapshot)

| Case | Headless/Simulated | Real OS | Real Plugin ABI / Native Dock | Primary Owner |
| --- | --- | --- | --- | --- |
| PD-01 VST3 dock/tab/resize | pointer → `pd-headless-contract.md` (`view-mount` semantic only) + `s1-headless-probe.md` (in-process real-ABI host/session legs Pass; `editor-attach` `Unsupported`; product `PluginDock` legs honest `Failed` — not dock evidence) | NotRun | NotRun — no real OS window/child HWND/pixel oracle; `dock-pixels` = `NotRun` (no `VisualCapture`) | synvelis#176 / ruviform#630/#631/#532 |
| PD-02 CLAP dock/tab/resize | same pointers (format dim = `clap`; S1 `clap-artifact`/`clap-host-lifecycle` Pass, product open `Unsupported`) | NotRun | NotRun (same — real-window lane absent) | synvelis#176 / ruviform#630/#631/#532 |
| PD-03 macOS AU dock | same pointer (format dim = `au`, OS = `macos`) | NotRun (要求状態 Deferred — POLICIES.md §2) | NotRun (同左・L2/L3 until use required) | synvelis#176 (activation on use) |
| PD-04 2Window move/float/reDock/DPI | pointer → `evidence/sv114/sv-os-case-definitions.md` SV-OS-05 (2Window/Dock/IME/Focus headless pointer only) | NotRun | NotRun | synvelis#176 / synvelis#113 / ruviform#313 |
| PD-05 Widget Focus/Keyboard/IME/Popup vs shortcut | pointer → `svt-headless-l1.md` SVT-09 (IPC/stale contract only; IME = unclaimed capability) | NotRun | NotRun | synvelis#176 / ruviform#313/#314 |
| PD-06 hang/exit/destroyed/stale; RT/Project survive | pointer → `l1-contract-comparison.md` §1–2 (stale generation/epoch L1 contract) + `s1-headless-probe.md` (`host-terminate`/`session-lifecycle` Pass on the real binary — in-process, not a crashed-host/OS case) | NotRun | NotRun (real-host-termination lane absent) | synvelis#176 / ruviform#389/#630/#631 |
| PD-07 Save→Reload logical state restore | pointer → `svt-headless-l1.md` SVT-08 (Save/Reload oracle, project-domain only — no plugin session) | NotRun | NotRun (plugin-session restore over a real window absent) | synvelis#176 / synvelis#114 |

## Case definitions

### PD-01 VST3 の GUI を Mixer から Dock へ開き Tab と Resize

- Scope: Synvelis Mixer/Plugin Panel から実 VST3 plugin の GUI を
  開き、製品 Dock Panel 内で Tab 化・Resize できることを実
  Window で検証する。Generic/Floating Editor の成功は Native
  Dock Pass にしない。
- Related: rf631 `view-mount`/`artifact`/`host-lifecycle`/
  `editor-attach`/`dock-pixels`、synvelis#176、synvelis#175
  (native app shell composition root — Plugin Panel の host)、
  synvelis#113 (Dock/Float)、ruviform#388/#389/#630/#631/#532。
- Expected (Real Plugin): 実 VST3 binary（`PluginArtifactEvidence`
  6 dims 記録）が production Host 経由で実 child surface として
  Dock され、Tab・Resize・Clip が製品 Oracle と一致する。
- Result: Real OS = `NotRun`、Real Plugin = `NotRun`。
  Headless/Simulated = `view-mount` semantic Pass のみ
  （contract conformance — dock 証拠ではない）。

### PD-02 CLAP 同様

- Scope: PD-01 と同一操作を CLAP format の実 plugin で検証する。
- Related: PD-01 と同じ rf631 語彙（format dim = `clap`）、
  synvelis#176、ruviform#630/#631。
- Expected (Real Plugin): PD-01 と同じ期待を CLAP artifact で。
- Result: Real OS = `NotRun`、Real Plugin = `NotRun`。

### PD-03 macOS AU（実規格 GUI）

- Scope: macOS で AU format の実 plugin GUI を Dock する。
  macOS L2/L3 は利用要件発生まで Deferred（POLICIES.md §2）
  のため、activation 時に OS/Owner/Case を再定義する。
- Related: PD-01/02 と同じ rf631 語彙（format dim = `au`、
  OS = `macos`）、synvelis#176。
- Expected (Real Plugin): AU の実規格 GUI が macOS 実 Window で
  Dock・Tab・Resize できる。
- Result: Real OS = `NotRun`、Real Plugin = `NotRun`
  （要求状態は Deferred — POLICIES.md §2。Result 語彙に
  `Deferred` は存在しない）。

### PD-04 2Window 間 move/float/reDock・DPI

- Scope: plugin editor surface の 2 Window 間移動・float・
  reDock・DPI 変更時の再投影を実 Window で検証する。
- Related: rf631 `session-lifecycle`/`open-or-degrade`/
  `dock-pixels`、synvelis#176、synvelis#113（Multiwindow/
  Dock/Float/ReDock/Workspace Restore）、synvelis#114
  （SV-OS-05 の 2Window/Dock/IME/Focus case）、
  ruviform#313。
- Expected (Real OS/Plugin): Window Manager 上で surface の
  move/float/reDock が失敗時 rollback を含めて製品契約どおりで、
  DPI 変更で clip/位置が壊れない。
- Result: Real OS = `NotRun`、Real Plugin = `NotRun`。

### PD-05 Widget Focus/Keyboard/IME/Plugin Popup vs DAW shortcut

- Scope: plugin GUI widget の Focus・Keyboard・IME 入力と、
  plugin popup/menu と DAW shortcut の競合を実入力で検証する。
- Related: rf631 `editor-attach`/`dock-pixels`（surface が
  存在して初めて意味を持つ）、synvelis#176、synvelis#114
  （SV-OS-05 IME/Focus・shortcut conflict）、
  ruviform#313/#314。IME capability は現行 adapter が一切
  申告しない（`HONESTY_ABSENT` で検証済み）。
- Expected (Real OS): 実入力で widget focus・IME・popup が
  DAW shortcut と衝突なく動作する。
- Result: Real OS = `NotRun`、Real Plugin = `NotRun`。

### PD-06 Plugin GUI hang/Host exit/Window destroyed/stale parent; Plugin Instance と RT/Project 存続

- Scope: plugin GUI の hang・Host exit・Window destroyed・
  stale parent 発生時に、Plugin Instance・DSP/Project/RT が
  非阻害で存続し、二重 Open/Close がないことを検証する。
- Related: rf631 `host-terminate`/`session-lifecycle`/
  `host-lifecycle`、synvelis#176、ruviform#389
  （PluginEditorSession/L0-L1 Host/parent lease/Recovery）、
  synvelis#114（SV-OS-04 transport/RT oracle 接続）、
  ruviform#630/#631。
- Expected (Real Plugin): crash/hang/timeout/stale/reconnect/
  window disappearance で RT/DAW/Project が阻害されず、
  release と session 回復が独立 Oracle と一致する。
- Result: Real OS = `NotRun`、Real Plugin = `NotRun`。

### PD-07 Session/Workspace Save→Reload 時の logical state restore（OS Native Handle を永続化しない）

- Scope: Session/Workspace の Save→Reload で plugin dock の
  logical state（dock 位置・tab・pin 等の製品状態）が復元
  され、OS Native Handle（HWND/NSView 等）が永続化・復元
  されないことを検証する。
- Related: rf631 `session-lifecycle`、synvelis#176、
  synvelis#113（Workspace Restore）、synvelis#114
  （SVT-08/SV-OS-04 の Save/Reload oracle — project domain
  のみで plugin session は含まない）、synvelis#175。
- Expected (Real Plugin): reload 後に logical state が復元
  され、native handle の永続化なしに editor が再 attach
  される（または正直に degrade する）。
- Result: Real OS = `NotRun`、Real Plugin = `NotRun`。

## Non-claims

- 本 freeze はいかなる Case の実 OS/実 Plugin 実行・合格も
  主張しない。Headless `view-mount` の semantic Pass は
  contract conformance のみであり、dock/window/pixel/plugin
  証拠ではない。
- `evidence/sv114/` の先行証拠は pointer 参照のみであり、
  本書がその内容を再検証・再主張・昇格するものではない
  （`l1-contract-comparison.md` §3 の plugin surface
  `Unsupported` 記録を含む）。
- `PluginHostProbe` 系 case の synvelis 側実走・実 OS
  Scenario Runner・opt-in mise 入口・CI Run URL 照合・
  製品 PluginDock view 実装は後続工程に残る。
- macOS/Linux の L2/L3・物理機材・市販 Plugin は本 freeze
  の対象外（POLICIES.md §2 仮想優先）。
