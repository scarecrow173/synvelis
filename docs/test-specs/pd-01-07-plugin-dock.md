---
id: "TSPEC-034XPHWmSBL0haKHzw1gua"
type: "test-spec"
status: "approved"
title: "SV-GPUI-02 Plugin Dock Gate — PD-01〜07 case freeze test spec (synvelis#176)"
created: "2026-11-15"
updated: "2026-11-15"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/176"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on:
    - "docs/specs/sv-gpui-02-plugin-dock-gate.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/svt-real-os-e2e.md"
    - "evidence/sv176/pd-01-07-case-definitions.md"
    - "evidence/sv176/acceptance-ledger.md"
    - "evidence/sv176/pd-headless-contract.md"
    - "evidence/sv114/l1-contract-comparison.md"
    - "crates/synvelis-app/tests/plugin_dock_contract.rs"
    - "crates/synvelis-app/tests/plugin_dock_contract_gpui.rs"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/sv-gpui-02-plugin-dock-gate.md"
  derived-by: []
  verifies:
    - "docs/specs/sv-gpui-02-plugin-dock-gate.md"
  verified-by: []
  references:
    - "evidence/sv176/pd-01-07-case-definitions.md"
    - "evidence/sv176/acceptance-ledger.md"
    - "evidence/sv176/pd-headless-contract.md"
    - "evidence/sv114/l1-contract-comparison.md"
    - "crates/synvelis-app/tests/plugin_dock_contract.rs"
    - "crates/synvelis-app/tests/plugin_dock_contract_gpui.rs"
  defers: []
  deferred-by: []
---

# SV-GPUI-02 Plugin Dock Gate — PD-01〜07 case freeze test spec (synvelis#176)

## Purpose

Issue #176 の Plugin Dock 受入ケース（PD-01〜07）と受入条件
9 行の分類が、実 OS/実 Plugin 実行の前に一意の契約として
固定されていることを保証する検証意図を記録する。実
OS/実 Plugin/実 Native Dock の実行結果そのものではなく、
**freeze された定義と ledger が後続の実行・受入判定の基準
として矛盾なく使えること**を保証する。`rf631.plugin-dock`
lane が具体化して各 case の実測証拠が別文書として積み上がっ
た時点で、本 test-spec の freeze 規則は実行証拠の schema
契約として残り、case 定義の改訂が必要になれば新しい
test-spec または明示的な supersede で扱う。

## Feature

SV-GPUI-02 Plugin Dock Gate — PD-01〜07 case definition
freeze and acceptance ledger (synvelis#176)

## Rules

### Rule PD-FRZ-01: Case freeze は Issue #176 v2 節の PD-01〜07 を網羅する

`evidence/sv176/pd-01-07-case-definitions.md` と本書は
PD-01（VST3 の GUI を Mixer から Dock へ開き Tab と
Resize）〜PD-07（Session/Workspace Save→Reload 時の
logical state restore — OS Native Handle を永続化しない）
を、Issue #176「2026-11-07 横断レビュー確定仕様 v2」の
実機製品ケース定義を正本として固定する。各 Case は証拠
スキーマ `Case × OS × plugin format × driver/backend/GPU
× plugin artifact identity（PluginArtifactEvidence 6 dims:
path・SHA-256 `sha256`・descriptor `plugin_id`・`name`・
`vendor`・`version`）× host/repo SHA × Expected / Observed /
Result(Pass/Fail/NotRun/Unsupported) / Artifact / Owner`
の全項目を持つ。

### Rule PD-FRZ-02: Headless/Simulated・Real OS・Real Plugin ABI/Native Dock は別評価列

各 Case の評価は少なくとも Headless/Simulated（OS-L1
相当）・Real OS（GUI-L2 相当）・Real Plugin ABI/Native
Dock（EXT-L3 相当）の列で分離する。ある列の
Pass/Fail/NotRun/Unsupported を別の列へ転用・昇格・合算
しない。Headless/Simulated 列は merged 証拠
（`evidence/sv114/l1-contract-comparison.md` §3 の plugin
surface Unsupported 記録・`svt-headless-l1.md`・
`svt_product_e2e*.rs`・本 slice の headless contract 証拠
`evidence/sv176/pd-headless-contract.md`）への参照
pointer に留め、証拠内容を複写・再主張しない。

### Rule PD-FRZ-03: freeze 時点の Real OS / Real Plugin は NotRun

本スライスは定義 freeze と headless semantic contract の
みであり、いかなる実 OS Window・実 plugin editor・実
Native Dock・実 device の実行も行わない。全 Case の Real
OS / Real Plugin ABI/Native Dock 行は `NotRun` で固定
する。`Unsupported` は根拠を伴う技術的非対応に限り
（upstream `createView` null の `editor-attach` 等）、
環境不在を `Unsupported` にしない（POLICIES.md §3）。
`Pass` は Observed・Artifact・実行環境・Run/実機 URL を
伴う実測がある場合に限る。

### Rule PD-FRZ-04: 受入条件 ledger は 3 分類のいずれかに留まる

`evidence/sv176/acceptance-ledger.md` は Issue #176 現行
受入条件 9 行を、(i) merged-evidence pointer、(ii)
`NotRun`、(iii) 外部 Owner（`scarecrow173/ruviform#630`/
`#631`/`#388`/`#389`/`#532`・synvelis#114・synvelis#28・
macOS/Linux L1 CI・merge 後 CI 証拠工程等）のいずれかに
分類する。いずれの行も本スライスで `Pass`/`accepted` に
しない。

### Rule PD-FRZ-05: 未使用 format/OS/capability を一律要求しない

Windows で未使用の plugin format・OS・capability は受入を
要求しない（POLICIES.md §2・Issue #176 AC「OS/機能別
Evidence」）。必要な実走が `NotRun`/`Fail` なら機能単位
で未受入とし、OS/Owner/CI/実機証拠を分離して記録する。
macOS/Linux の L2/L3 は利用要件発生まで Deferred、
未提供 capability は NotApplicable として case 行に反映
する。

### Rule PD-FRZ-06: plugin-identity dims と rf631 語彙を証拠 schema に採用する

証拠 schema の plugin-identity dims は
`PluginArtifactEvidence` の 6 項目（`path`・SHA-256
`sha256`・descriptor `plugin_id`・`name`・`vendor`・
`version` — `ruviform_ui_test` export・pin `ac10a2f7`）を
必須とする。`rf631.plugin-dock` の 8 case 語彙
（`view-mount`/`artifact`/`host-lifecycle`/`editor-attach`/
`session-lifecycle`/`host-terminate`/`open-or-degrade`/
`dock-pixels`）を証拠行の共有語彙として記録し、synvelis
側 runnability を正直に記す（`view-mount` = impl leg 実施
時のみ headless Runnable・probe 系 = 依存未追加で
`NotRun`/Deferred・`editor-attach` = upstream
`Unsupported`・`dock-pixels` = `NotRun`）。

### Rule PD-FRZ-07: Fake/Floating/Generic/Mock を Native Dock Pass にしない

semantic mount・Fake・Floating・Generic・Mock・画像証拠
の成功を Native Dock の Pass に読み替えない。headless
`view-mount` の `Pass` は contract conformance のみを
意味し、dock/window/pixel/plugin 証拠へ昇格しない
（POLICIES.md §2.1・Issue #176 v2 Matrix 節）。

## Examples

- Example: PD-01 を Windows 実機で後日実行する場合、
  `plugin format`・`PluginArtifactEvidence` 6 dims・
  `driver/backend/GPU`・実 Window の `Observed`・
  screenshot/log の `Artifact`・Run URL を埋めた行が
  新しい証拠文書に追加され、初めて `Result=Pass/Fail` を
  取り得る。freeze 行自体は `NotRun` のまま履歴として
  残る。
- Example: headless `view-mount` が `Pass` しても、PD-01
  の Real OS/Real Plugin 列へその結果をコピーしない。
  Headless/Simulated 列の pointer と Real Plugin 列の
  `NotRun` は独立に維持される。
- Example: `evidence/sv114/l1-contract-comparison.md` §3
  が plugin surface を `Unsupported`（製品層で未配線）
  と記録する現状をもって PD case を `Unsupported` と
  しない。製品 PluginDock view が具現化し実 plugin
  artifact が供給されるまで `NotRun`（Deferred）であり、
  `editor-attach` のみ upstream `createView` null の
  技術的 `Unsupported` を持つ。
- Example: impl leg の `plugin_dock_contract.rs`（Fake
  lane）と `plugin_dock_contract_gpui.rs`（GpuiSimulated
  lane）が `plugin_dock_spec(run, case, None)` の
  `required={NativePluginDock, VisualCapture}` に対し両
  adapter で pre-flight `Unsupported`・body 未実行を
  assert することは、「native-dock/pixel 証拠を要求する
  spec が semantic adapter で `Pass` を出せない」ことの
  検証（spec-helper contract）であり、PD case の
  `NotRun` 記録を補強するものであって `Pass` では
  ない。upstream probe case は `required={}` の
  `probe_spec` であり本 gate の対象ではない。

## Guarantees

- PD-01〜07 の定義・証拠スキーマ（`PluginArtifactEvidence`
  6 dims 含む）・評価列分離・`rf631.plugin-dock` 語彙・
  Owner が文書として一意に存在し、後続の実行者が解釈を
  要しない。
- 受入条件 9 行それぞれの現在地（merged evidence /
  NotRun / 外部 Owner / pending CI 照合）が ledger から
  読み取れる。
- Fake/Headless/Simulated/Generic/Floating の Pass が
  実 GUI・実 Native Dock・実 Plugin の Pass として記録
  される経路が、本 freeze 文書群の上では存在しない。
- impl leg 実施時、`plugin_dock_view_spec` の semantic
  mount が `Fake`/`GpuiSimulated` 両 adapter の production
  composition 形で成立することが headless で検証され、
  その `ScenarioReport` が adapter 実測の driver/
  capabilities のみを記録する。

## Non-goals

- PD-01〜07 の実 OS/実 Plugin での実行・合否判定
  （GUI-L2/EXT-L3 — `scarecrow173/ruviform#630`/#631・
  #532）。
- `PluginHostProbe` 系 case の synvelis 側実走 —
  `ruviform-plugin-test-vst3`/`ruviform-plugin-dock-test`
  依存追加が必要（Ask-first・本 slice では追加しない）。
- 製品 PluginDock view・`bind_native_slot`・
  `SurfaceSlotId` 実 binding の実装。
- `dock-pixels`（`VisualCapture` 非存在）・
  `editor-attach` の `createView`（upstream
  `Unsupported`）。
- macOS/Linux の L2/L3 — 利用要件発生まで Deferred
  （POLICIES.md §2）。
- merge 後対象 SHA の CI Run URL 照合 — Coordinator の
  証拠工程。
- Issue #176 の Close 判定。

## Risk

- freeze がないまま実 OS/実 Plugin レーンが個別に走ると、
  Case ID・証拠 schema・plugin-identity dims・判定語彙が
  ばらつき、後で台帳を再構成するコストが発生する。
- 分類を誤ると Headless/Fake の Pass が製品受入
  （Product Accepted）へ誤昇格し、POLICIES.md §2.1/§3
  の Required/Deferred・Pass/Fail/NotRun/Unsupported
  分離が崩れる。特に `view-mount` の semantic Pass を
  Native Dock Pass に読み替える誤りは PD-FRZ-07 が防ぐ。
- AC ledger が曖昧だと、外部 Owner（Ruviform #630/#631/
  #388/#389/#532・#294・synvelis#114/#28・CI）待ちの
  項目と Synvelis 側で完結する項目の境界が不明になり、
  Issue #176 の Close 判定が不可能になる。
- `PluginArtifactEvidence` dims を省略した証拠行は
  plugin binary の同一性を保証できず、再ビルドされた
  fixture を同一証拠として誤読する危険がある
  （PD-FRZ-06 が必須化する理由）。
