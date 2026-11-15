# Issue #176 受入条件 Acceptance Ledger (SV-GPUI-02 / PD-01〜07)

Date: 2026-11-15. Status: **freeze snapshot — no row upgraded to
`Pass`/`accepted` by this slice.** Case definitions:
`evidence/sv176/pd-01-07-case-definitions.md`. Headless contract
evidence (only executed lane): `evidence/sv176/pd-headless-contract.md`.

The 9 lines below are transcribed verbatim from Issue #176
「受入条件」as of 2026-11-15 (the 「仮想優先の置換DoD」 first line is
already marked 2026-11-15現行適用 upstream). Each line is classified
into exactly one of: (i) merged-evidence pointer, (ii) `NotRun`,
(iii) 外部 Owner pending. Unused formats/OS/features are not demanded
uniformly (POLICIES.md §2 / Issue AC「OS/機能別Evidence」).

| # | 受入条件（Issue #176 現行本文 verbatim） | 分類 | 根拠 / 次工程 |
| --- | --- | --- | --- |
| 1 | **仮想優先の置換DoD（2026-11-15現行適用）:** Synvelis製品のMixer/Inspector/Plugin Panelから#630実Test Native Editorを本番Host経由で実WindowにDockし、PD-01〜07の画素/Focus/Resize/Tab/2Window/ReDock/Crash/RTを#631の独立画素・Receipt・Project Oracleで受入。DComp失敗は修正後実走までFail。 | **NotRun（外部 Owner 主体）** | 実 Window/実 ABI Test Editor/独立 Oracle の実行環境が未整備。Owner: `scarecrow173/ruviform#630`（実ABI Test Native Editor・Windows child HWND）・`#631`（実GPUI Window Runner）・`#532`（既知 DComp Fail — 修正・実走まで未受入）。synvelis 側は `evidence/sv176/pd-01-07-case-definitions.md` で全 Real Plugin/Real OS 行を `NotRun` に凍結済み。 |
| 2 | Synvelis Mixer/Inspector/Plugin PanelのDock/Float/Tab/Focus/Selection/Save/RestoreをDriver-neutral製品ViewSpec/CommandとHeadless Fake Surface/Sessionの状態機械で実装・検証する。 | **NotRun（製品 view 未具現化）** | Driver-neutral ViewSpec/Command と headless 状態機械の実装は製品 PluginDock view slice の責務（`crates/synvelis-app/src/**` は本 slice の lease 外）。本 slice はその contract 語彙（`plugin_dock_view_spec` の semantic mount）と freeze を提供するのみ。headless mount のみ実施済み（`pd-headless-contract.md`）で、製品 view 実装の検証は含まない。 |
| 3 | PluginInstanceId/EditorSessionId/SurfaceSlotId/ViewInstanceIdの所有・世代・Open/Hide/ReDock/Closeと、DSP/Project/RT非干渉をMock/Fake Host/Windowの決定的テストで検証する。 | **merged-evidence pointer（L0/L1 部分）+ NotRun（plugin dock 結合部）** | ID 所有・世代・stale/epoch の L0/L1 契約は `evidence/sv114/l1-contract-comparison.md` §1–2（generation/epoch oracle）と `svt-headless-l1.md` が既に merged 証拠として持つ。Plugin dock 結合部（SurfaceSlotId↔plugin.editor・EditorSession 状態機械の決定的テスト）は依存未追加のため `NotRun`（`rf631.plugin-dock` probe 系は Ask-first の依存追加待ち）。pointer のみ、再主張なし。 |
| 4 | Windows/macOS/Linuxの既存CIでNative Build/Link、必要GPUI/Test/Plugin Host Feature、PluginEditorSession/NativeSurfaceSlot/Fake PanelのHeadless試験を対象SHAで実走する。 | **NotRun（pending: merge 後対象 SHA の CI Run URL 照合 — Coordinator 証拠工程）** | AC の中核証拠は対象 SHA での CI 実走であり merge 後にしか存在しない（POLICIES.md §3）。CI lane の存在と headless 試験の既存結果は `evidence/sv114/`・Ruviform `#388`/`#389`/`#390` が保持するが pointer のみ・再主張なし。本 slice 自身のローカル検証実測は `docs/impl/ir/sv-gpui-02-plugin-dock-gate.md` の Validation 節と `evidence/sv176/pd-headless-contract.md` の "What ran" 節に記録。Run URL 照合は Coordinator の証拠工程であり本 slice では実施しない。 |
| 5 | Ruviform #388/#389とSV #114/#28の公開Port・共通責務/Consumer UX境界を照合し、既知Windows DComp問題（RF #532）を未解消として正確に記録する。 | **merged-evidence pointer + 本 slice の記録** | `evidence/sv114/l1-contract-comparison.md` §3 が plugin surface `Unsupported` を記録済み（同 file に DComp/#532 の記録はない）。本 slice は `pd-headless-contract.md` の Non-claims 節（「`scarecrow173/ruviform#532` … remains an open Fail/pending item」）と `pd-01-07-case-definitions.md` の owner 列で `#532` を修正・実走まで未受入として記録する（pointer＋freeze 記録であり新規実測ではない）。 |
| 6 | PD-01〜07の実VST3/CLAP/AUの画素/IME/Focus/Resize/Host hang・OS別L0/L1・実Native Dockは現行初期必須から除き、対象OSで実Plugin GUIの利用が具体的に必要になった時だけActivationする。Generic/Floating/MockをNative Dock Passとは記録しない。 | **merged-evidence pointer（方針記録）** | 本 ledger・case definitions・test-spec PD-FRZ-02/03/07 がこの分離を文書化して凍結。実 Plugin GUI の Activation は利用要件発生時（POLICIES.md §2 Deferred）。Fake/Floating/Generic/Mock の非昇格は本 slice の全文書で明示。 |
| 7 | **Windows GUI-L2/EXT-L3（2026-11-15現行Required）:** WindowsでNative Plugin Dockを提供する場合はRF #630の正規ABI Test Native Editorの実Child HWNDと#631実GPUI Window Runnerを使用し、PD-01〜07の画素/Clip/Focus/IME/Resize/ReDock/2Window/Crash/RT非干渉をSynvelis製品Oracleで判定する。市販Pluginは通常必須にしないが、RF #532 DComp既知Failは修正・実走まで未受入。 | **NotRun（外部 Owner 主体・Required）** | Windows で Native Plugin Dock を提供する場合の Required 条件。実 child HWND・実 Window Runner・独立 Oracle の実行は `scarecrow173/ruviform#630`/#631/#532 待ち。DComp 既知 Fail は `evidence/sv176/pd-headless-contract.md` の Non-claims 節に open Fail/pending item として記録済み。 |
| 8 | **macOS/Linux L1必須:** Native Build/Link、Headless/RT/必要Featureを既存CIの同SHAで検証。実GUI/実Device/実Pluginは利用条件発生までDeferred。 | **NotRun（pending: 同 SHA の L1 CI 照合）+ Deferred（L2/L3）** | AC が要求するのは同一 SHA での L1 CI 検証であり、merge 後対象 SHA の照合待ち（行 4 と同じ pending CI 証拠工程）。macOS/Linux の実 GUI/実 Device/実 Plugin は POLICIES.md §2 により Deferred — PD-03（AU）は case definitions で Result=`NotRun`・要求状態=Deferred と記録済み。 |
| 9 | **OS/機能別Evidence:** Windowsの使用機能で実証できなければ未受入。未使用機器・Plugin/Featureを一律Gateにせず、実測/NotRun/Fail/NotRequiredを混同しない。 | **merged-evidence pointer（分類規則の凍結）** | 本 ledger 自体がこの分類規則の適用結果。`pd-01-07-case-definitions.md` の lane status summary が `NotRun`/`Unsupported`（`editor-attach` upstream `createView` null のみ）を機能単位で分離して記録（`Deferred` は Result 語彙ではなく要求状態の別軸）。未使用 format/OS/capability は要求しない（PD-FRZ-05）。 |

## 分類サマリ

- **merged-evidence pointer**: 行 3（L0/L1 部分 — plugin dock 結合部は NotRun）, 5（記録の再確認）, 6（方針）, 9（分類規則）
- **NotRun**: 行 1, 2, 4, 7, 8（外部 Owner/実環境/対象 SHA CI 照合待ち — `ruviform#630`/#631/#532・製品 view slice・Coordinator CI 証拠工程）
- **本 slice で `Pass`/`accepted` にした行**: なし

## 外部 Owner 依存の一覧（本 ledger が `NotRun`/pending に留める理由）

- `scarecrow173/ruviform#630` — 正規 ABI Test Native Editor・Windows child HWND
- `scarecrow173/ruviform#631` — 実 GPUI Window Runner・`rf631.plugin-dock` 実走 lane
- `scarecrow173/ruviform#532` — 既知 Windows DComp 画素不具合（修正・実走まで Fail/未受入）
- `scarecrow173/ruviform#388`/#389 — NativeSurfaceSlot / PluginEditorSession 公開 Port
- `scarecrow173/ruviform#294` — 両 Consumer 非回帰・candidate SHA 照合
- `scarecrow173/synvelis#114` — PD-01〜07 の実 OS/実 Plugin 受入統合
- `scarecrow173/synvelis#28` — Ruviform pin/consumer 追従
- 製品 PluginDock view slice（別タスク）— 行 2 の Driver-neutral 製品 ViewSpec/Command・headless 状態機械の実装・検証
- Coordinator 証拠工程 — merge 後対象 SHA の CI Run URL 照合（行 4/8）
