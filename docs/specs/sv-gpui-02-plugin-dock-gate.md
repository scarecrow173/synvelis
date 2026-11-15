---
id: "SPEC-034XPGS17BBlFOHnchrKxE"
type: "spec"
status: "approved"
title: "SV-GPUI-02 Plugin Dock Gate — PD-01〜07 case freeze and acceptance ledger (synvelis#176)"
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
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/pd-01-07-plugin-dock.md"
    - "docs/specs/sv-gpui-01-native-app-shell.md"
    - "docs/specs/sv-gui-05-real-os-e2e.md"
    - "docs/test-specs/svt-real-os-e2e.md"
    - "evidence/sv176/pd-01-07-case-definitions.md"
    - "evidence/sv176/acceptance-ledger.md"
    - "evidence/sv176/pd-headless-contract.md"
    - "docs/impl/ir/sv-gpui-02-plugin-dock-gate.md"
    - "docs/impl/exp/sv-gpui-02-plugin-dock-gate.md"
    - "crates/synvelis-app/tests/plugin_dock_contract.rs"
    - "crates/synvelis-app/tests/plugin_dock_contract_gpui.rs"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/pd-01-07-plugin-dock.md"
  references:
    - "evidence/sv176/pd-01-07-case-definitions.md"
    - "evidence/sv176/acceptance-ledger.md"
    - "evidence/sv176/pd-headless-contract.md"
    - "evidence/sv114/sv-os-case-definitions.md"
    - "evidence/sv114/acceptance-ledger.md"
    - "evidence/sv114/l1-contract-comparison.md"
    - "crates/synvelis-app/tests/plugin_dock_contract.rs"
    - "crates/synvelis-app/tests/plugin_dock_contract_gpui.rs"
    - "docs/impl/ir/sv-gpui-02-plugin-dock-gate.md"
    - "docs/impl/exp/sv-gpui-02-plugin-dock-gate.md"
  defers: []
  deferred-by: []
---

# SV-GPUI-02 Plugin Dock Gate — PD-01〜07 case freeze and acceptance ledger (synvelis#176)

## What Should Be Built

Issue #176（[ui/plugin-dock][SV-GPUI-02] 外部VST3/CLAP/AU実GUIの
GPUI Dock統合・UX・OS別受入Gate）の残件 docs/evidence
スライスとして、**PD-01〜07 の case skeleton freeze** と
**Issue #176 現行受入条件 9 行の acceptance ledger 化** を行う。
具体的には:

- Issue #176 本文「2026-11-07 横断レビュー確定仕様 v2」の
  実機製品ケース **PD-01〜07** を、証拠スキーマ
  `Case × OS × plugin format × driver/backend/GPU ×
  plugin artifact identity（PluginArtifactEvidence 6 dims:
  path・SHA-256・descriptor plugin_id・name・vendor・version）×
  host/repo SHA × Expected / Observed /
  Result(Pass/Fail/NotRun/Unsupported) / Artifact / Owner`
  で凍結し、`evidence/sv176/pd-01-07-case-definitions.md` と
  `docs/test-specs/pd-01-07-plugin-dock.md` に固定する。
- Issue #176 現行「受入条件」9 行を `evidence/sv176/
  acceptance-ledger.md` に写し取り、各行を merged-evidence
  pointer・`NotRun`・外部 Owner のいずれかへ正直に分類する。
- Headless/Simulated（OS-L1）・Real OS（GUI-L2）・Real Plugin
  ABI / Native Dock（EXT-L3）を別評価列として分離し、先行
  スライスの証拠（`evidence/sv114/` 各 file・
  `l1-contract-comparison.md` §3 の plugin surface
  Unsupported 記録等）への参照 pointer のみを張る。
- `rf631.plugin-dock` scenario 語彙（`ruviform_ui_test` が
  pin `ac10a2f7` で export する `PLUGIN_DOCK_SCENARIO`・
  `PLUGIN_DOCK_SLOT_KEY`・`plugin_dock_view_spec`・
  `plugin_dock_spec`・`PluginArtifactEvidence`）を PD case
  証拠行の共有語彙として採用し、synvelis 側 runnability を
  正直に記録する。
- optional impl leg として、`crates/synvelis-app/tests/
  plugin_dock_contract.rs` に headless view-mount contract
  test を追加する（条件を満たす場合のみ — 下記 Acceptance
  Criteria 参照）。

本 spec は **SV-GPUI-02 全体の spec ではなく本 freeze slice
の spec** である。Issue #176 全体の完了・いかなる case/AC の
`Pass`/`accepted` も宣言しない。

## Why Now

Issue #176 は Plugin Dock 製品受入 Gate の Issue であり、
PD-01〜07 の実 OS/実 Plugin 受入（GUI-L2/EXT-L3）は
`scarecrow173/ruviform#630`（正規ABI Test Native Editor・
Windows child HWND）・`scarecrow173/ruviform#631`（実 GPUI
Window Runner）・既知 DComp 問題 `scarecrow173/ruviform#532`
の解消待ちで未実施のまま残っている。一方で、Ruviform pin
`ac10a2f7` では `rf631.plugin-dock` の scenario 語彙
（8 case: `view-mount`/`artifact`/`host-lifecycle`/
`editor-attach`/`session-lifecycle`/`host-terminate`/
`open-or-degrade`/`dock-pixels`）と `PluginArtifactEvidence`
identity schema が `ruviform-ui-test` から export 済みであり、
case freeze の語彙基盤が利用可能になった。

実行環境・依存 crate・製品 PluginDock view が未整備のまま
実機受入が走ると case ID・証拠 schema・判定語彙がばらつく
ため、**実行可能な状態になる前にケース定義と受入分類を
凍結**して後続スライスの判定基準を固定する
（`docs/specs/sv-gui-05-real-os-e2e.md` と同じ freeze
様式）。

## Users and Value

- Synvelis の Acceptance Owner（Issue #176）と Coordinator:
  PD-01〜07 の実施時に「何を・どの OS/format/plugin
  artifact で・どの証拠 schema で判定するか」が一意に
  定まる。
- Ruviform 側 Owner（`scarecrow173/ruviform#630`/#631/
  #388/#389/#532）と Consumer 横断 Gate（#294）: Synvelis
  側の要求分類と外部依存が明示され、Fake/Headless/Generic/
  Floating の Pass の誤昇格を防げる。
- 後続 slice の実装者: headless contract test が「共有
  `plugin_dock_view_spec` を production composition で
  mount できる」ことの最小証拠を提供する。

## Scope

### In Scope

- PD-01〜07 のケース定義 freeze（Issue #176 v2 節の定義を
  正本として、証拠 schema・評価列・lane status・
  `rf631.plugin-dock` 語彙との対応・Expected の骨格）。
- 証拠スキーマと評価列（Headless/Simulated・Real OS・
  Real Plugin ABI/Native Dock）の明示。plugin-identity
  dims に `PluginArtifactEvidence` 6 項目を必須採用。
- Issue #176 受入条件 9 行の acceptance ledger 化
  （merged-evidence pointer / `NotRun` / 外部 Owner の
  分類）。
- optional impl leg: `crates/synvelis-app/tests/
  plugin_dock_contract.rs` + `plugin_dock_contract_gpui.rs`
  （`rf631.plugin-dock` の `view-mount` semantic
  contract + `plugin_dock_spec` helper の capability
  gate pre-flight `Unsupported` 検証。実施可否は
  下記 AC-4）。
- spec anchor → test-spec → impl-ir → impl-exp の文書鎖と
  管理 index の再生成。

### Out of Scope

- PD-01〜07 の実 OS/実 Plugin 実行（GUI-L2/EXT-L3 —
  `scarecrow173/ruviform#630`/#631 実環境 + #532 DComp
  解消待ち）。
- `PluginHostProbe` 系 case（`artifact`/`host-lifecycle`/
  `session-lifecycle`/`host-terminate`/`open-or-degrade`）
  の synvelis 側実走 — `ruviform-plugin-test-vst3`・
  `ruviform-plugin-dock-test` の依存追加が必要であり
  Ask-first 事項（本 slice では追加しない）。
- `editor-attach` の `createView` — upstream が現時点
  `createView` null で `Unsupported`（freeze にその旨を
  記録するのみ）。
- `dock-pixels` — `VisualCapture` capability 非存在
  （`NotRun`）。
- 製品 PluginDock view の実装・`ui_ops` 登録・ViewSpec
  builder wiring・`bind_native_slot`/`SurfaceSlotId` 実
  binding（`crates/synvelis-app/src/**` は本 slice の
  lease 外）。
- `Cargo.toml`/`Cargo.lock`/`mise.toml` の変更、
  `tests/common/**` の改変、既存 test/doc/evidence の
  変更、CI workflow の変更。
- macOS/Linux L2/L3（POLICIES.md §2 Deferred）・merge 後
  対象 SHA の CI Run URL 照合（Coordinator の証拠工程）。
- いかなる Case/AC 行の `Pass`/`accepted` 昇格、Issue
  #176 の Close。

## Requirements

- PD-01〜07 は Issue #176「2026-11-07 横断レビュー確定
  仕様 v2」の実機製品ケース定義行を正本として freeze
  する。各 case が証拠 schema 全項目を持つ。
- Headless/Simulated・Real OS・Real Plugin ABI/Native
  Dock は別評価列とし、列間で昇格・転用・合算しない。
  Headless/Simulated 列は merged 証拠への pointer に
  留め、内容の複写・再主張をしない。
- freeze 時点の Real OS/Real Plugin 行は `NotRun`。
  `Unsupported` は根拠を伴う技術的非対応のみ
  （upstream `createView` null の `editor-attach` 等）。
  環境不在を `Unsupported` にしない（POLICIES.md §3）。
- 受入条件 9 行はそれぞれ (i) merged-evidence pointer、
  (ii) `NotRun`、(iii) 外部 Owner のいずれかに分類し、
  本 slice で `Pass` にしない。
- plugin-identity dims に `PluginArtifactEvidence` の
  6 項目（path・SHA-256 `sha256`・descriptor `plugin_id`・
  `name`・`vendor`・`version`）を証拠 schema の必須 dim
  として採用し、`rf631.plugin-dock` scenario ID/case
  語彙を証拠行の共有語彙として記録する。
- Fake/Floating/Generic/Mock を Native Dock Pass に
  せず、semantic mount pass を dock/window/pixel 証拠に
  昇格しない。未使用 format/OS/capability を一律要求
  しない（POLICIES.md §2・Issue AC「OS/機能別Evidence」）。

## Acceptance Criteria

- [ ] `docs/specs/sv-gpui-02-plugin-dock-gate.md`・
  `docs/test-specs/pd-01-07-plugin-dock.md`・
  `evidence/sv176/pd-01-07-case-definitions.md` が
  PD-01〜07 を同一証拠スキーマで凍結し、全 Real
  OS/Real Plugin 行が `NotRun` である。
- [ ] `evidence/sv176/acceptance-ledger.md` が受入条件
  9 行を merged-evidence pointer / `NotRun` / 外部
  Owner に分類し、参照先証拠の再主張・昇格をして
  いない。
- [ ] optional impl leg を実施した場合、
  `crates/synvelis-app/tests/plugin_dock_contract.rs`
  （Fake lane）と
  `crates/synvelis-app/tests/plugin_dock_contract_gpui.rs`
  （`gpui-test` feature の GpuiSimulated lane）が
  `rf631.plugin-dock` 語彙で `view-mount` の
  semantic contract（`plugin.editor` NativeSlot の
  resolve/observe → `Pass` + honest detail）と、
  `plugin_dock_spec` helper の capability gate
  （`required={NativePluginDock, VisualCapture}` が両
  adapter で pre-flight `Unsupported`・body 未実行 —
  spec-helper contract であり upstream probe case の
  仕組みではない）を検証し、`ScenarioReport` が
  adapter 実測の `Fake`/`GpuiSimulated` と
  `{SemanticAction, MultiWindow}` のみを記録する。
  依存追加が必要になった場合は docs-only に切替え、
  残件を記録する。
- [ ] impl leg 実施時は headless contract 証拠
  （実行コマンド・OS・toolchain・exit code・
  driver/capabilities/verdict 実測）を
  `evidence/sv176/` に追加する。
- [ ] `docs/specs/README.md`・`docs/test-specs/
  README.md`・`docs/impl/ir/README.md`・
  `docs/impl/exp/README.md` の managed index が
  `doc-maintenance`/`sync_impl_index` で再生成されて
  いる。
- [ ] impl leg 実施時の検証:
  `mise run check synvelis-app`・`mise run lint
  synvelis-app`・`mise run test synvelis-app`・
  `mise run check-app-gpui`・`mise run lint-app-gpui`・
  `mise run test-app-gpui`・`mise run
  check-headless-deps` が green。docs-only の場合は
  `fmt-check`・`prek run --all-files`・doc index/audit
  のみ（POLICIES.md §5）。

## Deferred Design Concerns

- 実 OS/実 Plugin 実行環境（`scarecrow173/ruviform#630`
  の実ABI Test Native Editor・`scarecrow173/ruviform#631`
  の実 GPUI Window Runner・既知 DComp `scarecrow173/
  ruviform#532`） | 実 Window/実 child HWND/pixel
  oracle の環境が未整備 | 実 OS/実 Plugin lane の具体
  化時 | 環境不在を `Unsupported`/`Pass` に誤分類する
  恐れ。
- `PluginHostProbe` 系 case の synvelis 側実走に必要な
  依存追加（`ruviform-plugin-test-vst3`・
  `ruviform-plugin-dock-test`） | Ask-first 事項・
  本 slice の lease 外 | 依存追加の承認時 | probe 系
  case が `NotRun`/Deferred のまま残る。
- 製品 PluginDock view 実装・`bind_native_slot`/
  `SurfaceSlotId` 実 binding・`ui_ops` 登録 |
  `crates/synvelis-app/src/**` は本 slice の lease 外・
  別 slice の責務 | 製品 view 実装 slice の着手時 |
  semantic mount pass を製品 dock 機能の証拠と誤読
  する恐れ。
- merge 後対象 SHA の CI Run URL・Case 別照合 | merge
  後の CI 証拠が未収集 | merge 後の対象 SHA の CI 完了
  時（Coordinator の証拠工程）| Job green のみでは
  個別 Case の Pass を示せない（POLICIES.md §3）。
- `editor-attach` の `createView` 実装 | upstream
  （Ruviform）が現時点 `createView` null で
  `Unsupported` | upstream の editor attach 実装時 |
  PD case の一部が `Unsupported` のまま残る。

## Sources and Relations

- Source: synvelis Issue #176（現行受入条件 9 行・
  2026-11-07 横断レビュー確定仕様 v2 の PD-01〜07 定義・
  Matrix 節・Owner 境界・着手条件「なし」）。
- Related: `docs/specs/sv-gpui-01-native-app-shell.md`
  （GPUI native shell の spec）、
  `docs/specs/sv-gui-05-real-os-e2e.md`（SV-GUI-05 の
  freeze slice — 同じ freeze 様式の先例）、
  `docs/test-specs/pd-01-07-plugin-dock.md`（本 spec の
  検証意図）、`evidence/sv176/` の freeze 成果物、
  `docs/impl/ir/sv-gpui-02-plugin-dock-gate.md`・
  `docs/impl/exp/sv-gpui-02-plugin-dock-gate.md`（実施記録）、
  `crates/synvelis-app/tests/plugin_dock_contract.rs`・
  `crates/synvelis-app/tests/plugin_dock_contract_gpui.rs`
  （optional impl leg の Fake / GpuiSimulated lane）。
- External vocabulary: `ruviform-ui-test`（pin
  `ac10a2f7`）の `rf631.plugin-dock` scenario・
  `PluginArtifactEvidence`・`plugin_dock_view_spec`・
  `plugin_dock_spec`。Ruviform 側 case 正本は
  `ruviform-plugin-dock-test`（synvelis からは参照のみ・
  依存ではない）。
