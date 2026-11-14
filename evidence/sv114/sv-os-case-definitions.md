# SV-OS-01〜09 Real OS Case Definitions (synvelis#114 / SV-GUI-05)

Date: 2026-11-14. Status: **frozen at definition level — no real-OS,
real-GUI, or native-plugin lane was executed by this slice.** Every
Real OS / Native Plugin row below is `NotRun`; Headless/Simulated
rows are reference pointers to already-merged evidence only.

Source of truth for case identity: synvelis Issue #114, section
「2026-11-10 実GUI自動入口・ケースとOwner補足」(Windows 先行
ケース). Verification intent: `docs/test-specs/svt-real-os-e2e.md`
(Rules SVT-FRZ-01〜05). Spec anchor:
`docs/specs/sv-gui-05-real-os-e2e.md`.

## Evidence schema (frozen)

Each executed case row MUST carry:

| Field | Meaning |
| --- | --- |
| Case | `SV-OS-01` … `SV-OS-09` |
| OS | `windows` / `macos` / `linux` (+ arch) |
| driver/backend/GPU | OS input driver, GPUI backend, GPU/adapter used |
| plugin binary/format/SHA | required only for SV-OS-09 (VST3/CLAP/AU binary, format, plugin & host SHA) |
| Expected | observable expectation before the run |
| Observed | what was actually observed (empty while NotRun) |
| Result | `Pass` / `Fail` / `NotRun` / `Unsupported` (POLICIES.md §3 vocabulary) |
| Artifact | screenshot/video/log/receipt paths or run URLs |
| Owner | accepting owner (Synvelis issue / Ruviform issue / CI) |

Evaluation lanes are recorded separately and never merged:

- **Headless/Simulated** — Fake Driver / `GpuiGuiTest` /
  `GpuiSimulated` evidence (OS-L1 semantic layer).
- **Real OS** — real GPUI window, real OS user input, focus,
  IME, DPI, pixel (GUI-L2; Windows-first per Issue #114).
- **Native Plugin** — real VST3/CLAP/AU dock/editor/device
  (EXT-L3; synvelis#176 scope).

macOS/Linux Real OS / Native Plugin lanes are `NotRun` by policy
(POLICIES.md §2: L2/L3 Deferred until use is required). Windows
lanes for features actually provided are Required; features not
offered are NotApplicable and are never demanded uniformly
(Issue #114 AC8).

## Lane status summary (freeze snapshot)

| Case | Headless/Simulated | Real OS (Windows first) | Native Plugin | Primary Owner |
| --- | --- | --- | --- | --- |
| SV-OS-01 | pointer → `svt-headless-l1.md` SVT-01/07 | NotRun | NotApplicable | synvelis#114 (GUI-L2) / ruviform#313 |
| SV-OS-02 | pointer → `svt-headless-l1.md` SVT-02 | NotRun | NotApplicable | synvelis#114 / ruviform#313 |
| SV-OS-03 | pointer → `svt-headless-l1.md` SVT-03/04 | NotRun | NotApplicable | synvelis#114 / ruviform#313 |
| SV-OS-04 | pointer → `svt-headless-l1.md` SVT-05/08 | NotRun | NotRun (real audio device = separate EXT-L3, only if offered) | synvelis#114 / #109 oracle |
| SV-OS-05 | pointer → `svt-headless-l1.md` SVT-07/09 + `l1-contract-comparison.md` §1–2 | NotRun | NotApplicable | synvelis#114 / ruviform#243/#313 |
| SV-OS-06 | partial pointer → `svt-headless-l1.md` SVT-06 (viz bounded/stale only; pixel = NotRun there too) | NotRun | NotApplicable | synvelis#114 / #177 / ruviform#314 |
| SV-OS-07 | no dedicated headless row in `svt-headless-l1.md` (asset browser UI not in SVT-01〜09 scope) | NotRun | NotApplicable | synvelis#114 / synvelis#89 |
| SV-OS-08 | pointer → `svt-headless-l1.md` SVT-06 + domain oracles (#103/#109 headless) | NotRun | NotApplicable | synvelis#114 / synvelis#108/#102 |
| SV-OS-09 | Unsupported at product layer today (`syn.app.info` reports plugin surface unavailable; `l1-contract-comparison.md` §3) | NotRun | NotRun (Deferred until #176 concrete) | synvelis#176 / ruviform#388/#389/#532 |

## Case definitions

### SV-OS-01 Project 起動・Close

- Scope: 実 OS で Synvelis GPUI アプリを起動し、Project の
  Create/Open が View に反映されること、Window Close・dirty
  veto・終了時の cleanup が製品契約どおりであることを実走する。
- Related: SVT-01/SVT-07（headless 対応済み）、SV-UI-01、
  synvelis#112/#113/#175、close veto（`tests/close_veto.rs`
  の L1 oracle）。
- Expected (Real OS): 実 Window が表示され、Project 操作が
  製品 State/Receipt を通じて View に反映され、Close 時に
  dirty guard が働き、プロセスが leak なく終了する。
- Result: Real OS = `NotRun`（実 OS Scenario Runner / opt-in
  mise 入口が未整備）。Native Plugin = NotApplicable。

### SV-OS-02 Arrangement→Inspector Follow/Pinned

- Scope: Arrangement での Selection が Inspector の Follow
  対象を駆動し、Pinned 時は Selection 変更で target が
  置き換わらないことを、実 OS の pointer/keyboard 入力で
  検証する。
- Related: SVT-02、Context Inspector（synvelis#5 系）、
  synvelis#112。
- Expected (Real OS): 実入力での selection → inspector
  追従/pin が L1 oracle（`svt02_*`・`select_row_scenario`）
  と同じ semantic 結果を返す。
- Result: Real OS = `NotRun`。

### SV-OS-03 Fader→Command/Receipt/Undo

- Scope: Mixer Fader の実 pointer drag が Command →
  Receipt → UI 状態へ反映され、Undo/Redo が製品契約どおり
  動くことを実走する。AUTX-GUI-01〜06（synvelis#267）とは
  同一 Case ID 体系で接続する対象。
- Related: SVT-03/SVT-04、SV-UI-02、synvelis#267。
- Expected (Real OS): 実 pointer gesture の commit/cancel・
  receipt の Requested/Applied/Observed 区別・Undo が
  headless oracle と一致する。
- Result: Real OS = `NotRun`。

### SV-OS-04 Transport/Save/Reload と Project/RT 独立 Oracle

- Scope: 実 OS で Transport（Play/Pause/Stop/Seek）・
  Save・Reload を実行し、Project/RT の独立 Oracle
  （`syn.project.info`・bundle 直接 parse・#109 系 domain
  oracle）と照合する。実 Audio/MIDI device への出力を伴う
  部分は EXT-L3 であり、製品が当該 capability を提供する
  場合のみ Required。
- Related: SVT-05/SVT-08、SV-UI-03、synvelis#109。
- Expected (Real OS): Requested/Applied/Observed の区別・
  Save→Reload 後の project truth が独立 Oracle と一致。
- Result: Real OS = `NotRun`。実 device 出力 = `NotRun`
  （提供時のみ Required）。

### SV-OS-05 2Window/Dock/IME/Focus・shortcut conflict

- Scope: 実 OS の 2 Window 所有権・Dock 移動・IME 入力・
  Focus 遷移・keyboard shortcut 競合を検証する。stale
  generation/attach epoch の実 OS での再現も含む。
- Related: SVT-07/SVT-09、SV-UI-04、synvelis#113、
  ruviform#243/#313、`l1-contract-comparison.md` §1–2、
  AUTX-GUI-01〜06（#267）。
- Expected (Real OS): 実 Window Manager 上での 2 window
  ownership・focus・IME・shortcut が Fake/Fixture の L1
  契約と矛盾しない。
- Result: Real OS = `NotRun`。

### SV-OS-06 Theme light/dark × 2 density/DPI/a11y Pixel

- Scope: Theme light/dark 切替・2 density・DPI scaling・
  accessibility を実 OS の pixel として検証する。Ruviform
  #314 の Pixel Golden 契約を消費する。
- Related: SV-UI-05、synvelis#177、ruviform#314、
  AUTX-GUI pixel 判定（#267）。
- Expected (Real OS): 実画面 pixel が theme/density/DPI
  ごとの期待と一致し、a11y 要件を満たす。
- Result: Real OS = `NotRun`。Headless/Simulated でも pixel
  自体は未検証（`svt-headless-l1.md` SVT-06 の NaN-gap/
  canvas paint は NotRun）。

### SV-OS-07 Asset Browser Pin/Cancel (synvelis#89)

- Scope: Asset Browser の Pin/Cancel 操作を実 OS GUI で
  検証する。metadata-first browse・pinned intent・
  progress/conflict UX は synvelis#89（ALIB-05）が製品側
  Owner。
- Related: synvelis#89、SV-UI 系 asset/library UI。
- Expected (Real OS): 実 Window 上で Pin/Cancel が製品
  State/Receipt と整合し、cancel 時に副作用が残らない。
- Result: Real OS = `NotRun`。

### SV-OS-08 Signal Flow/Automation UI (synvelis#108/#102)

- Scope: Signal Flow Editor と Automation UI の gesture・
  表示を実 OS で検証し、#103/#109 の独立 Domain/RT Oracle
  と対応づける。
- Related: SV-UI-08、synvelis#108（SF-05）・#102（PC-C5）・
  #109・#103。
- Expected (Real OS): 実入力 gesture が semantic
  relation/selection/context と一致し、表示が domain truth
  を偽らない。
- Result: Real OS = `NotRun`。

### SV-OS-09 Native Plugin Dock VST3/CLAP 実 Plugin 画素/Input/rollback

- Scope: 実 VST3/CLAP plugin の Native Dock で、実画素・
  実入力・失敗時 rollback を検証する。plugin binary/
  format/SHA・host SHA を証拠行に必須記録する。PD-01〜07
  （実 Plugin hang 等）の必要 Format/操作は synvelis#176
  が別途定義する。
- Related: SV-UI-06、synvelis#176、ruviform#388/#389/#532、
  `l1-contract-comparison.md` §3。
- Expected (Real OS / Native Plugin): Floating/Generic
  Editor の成功を Native Dock Pass にしない。実 plugin
  editor の画素・Focus・入力・popup・resize・失敗復旧・
  RT 非阻害を実測する。
- Result: Real OS = `NotRun`、Native Plugin = `NotRun`
  （現行製品は plugin surface を unavailable と正直に報告
  しており、#176 で具現化するまで Deferred。Fake/Mock の
  成功をもって `Unsupported` にもしない）。

## Non-claims

- 本 freeze はいかなる Case の実行・合格も主張しない。
- Headless/Simulated の既存証拠は pointer 参照のみであり、
  本書がその内容を再検証・再主張するものではない。
- 実 OS Scenario Runner・opt-in mise 入口・CI Run URL
  照合は後続工程（Coordinator の証拠工程を含む）に残る。
