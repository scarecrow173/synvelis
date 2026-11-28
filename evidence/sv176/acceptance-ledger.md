# Issue #176 受入条件 Acceptance Ledger (SV-GPUI-02 / PD-01〜07)

Date: 2026-11-15 (行2/3 更新: 2026-11-24 headless PluginDock panel
slice 着地分; 行4/6/9 + 補強証拠更新: 2026-11-25 S1 headless
real-ABI probe slice; 行4/8 更新: 2026-11-25 Coordinator CI 証拠
工程 — 対象 SHA `71275d85` で Run 45482081443 全 lane success).
Status: **行 2・行 3（plugin-dock 結合部）は
headless 実装+検証で充足し、S1 で実 ABI test-plugin 上の補強証拠を
獲得。行 4/6/9 は S1 実走分を記録済み。行 4・行 8 の L1 部分は
対象 SHA CI 実走の照合が完了し充足。行 1/7 は引き続き
`NotRun`/外部 Owner。** Case definitions:
`evidence/sv176/pd-01-07-case-definitions.md`. Headless contract
evidence: `evidence/sv176/pd-headless-contract.md`（semantic mount
lane）+ `tests/plugin_dock.rs`（panel/op tier）+ S1 の
`evidence/sv176/s1-headless-probe.md`/`s1-headless-probe.jsonl` +
`tests/plugin_dock_probe.rs`（in-process real-ABI probe tier）。Session integration evidence: `evidence/sv176/s4-session-integration.md`。

The 9 lines below are transcribed verbatim from Issue #176
「受入条件」as of 2026-11-15 (the 「仮想優先の置換DoD」 first line is
already marked 2026-11-15現行適用 upstream). Each line is classified
into exactly one of: (i) merged-evidence pointer, (ii) `NotRun`,
(iii) 外部 Owner pending. Unused formats/OS/features are not demanded
uniformly (POLICIES.md §2 / Issue AC「OS/機能別Evidence」).

| # | 受入条件（Issue #176 現行本文 verbatim） | 分類 | 根拠 / 次工程 |
| --- | --- | --- | --- |
| 1 | **仮想優先の置換DoD（2026-11-15現行適用）:** Synvelis製品のMixer/Inspector/Plugin Panelから#630実Test Native Editorを本番Host経由で実WindowにDockし、PD-01〜07の画素/Focus/Resize/Tab/2Window/ReDock/Crash/RTを#631の独立画素・Receipt・Project Oracleで受入。DComp失敗は修正後実走までFail。 | **NotRun（外部 Owner 主体）** | 実 Window/実 ABI Test Editor/独立 Oracle の実行環境が未整備。Owner: `scarecrow173/ruviform#630`（実ABI Test Native Editor・Windows child HWND）・`#631`（実GPUI Window Runner）・`#532`（既知 DComp Fail — 修正・実走まで未受入）。synvelis 側は `evidence/sv176/pd-01-07-case-definitions.md` で全 Real Plugin/Real OS 行を `NotRun` に凍結済み。 |
| 2 | Synvelis Mixer/Inspector/Plugin PanelのDock/Float/Tab/Focus/Selection/Save/RestoreをDriver-neutral製品ViewSpec/CommandとHeadless Fake Surface/Sessionの状態機械で実装・検証する。 | **充足（headless slice — 2026-11-24）** | 製品 `PluginDock` パネル状態機械（`crates/synvelis-app/src/plugin_dock.rs`）+ `syn.plugin_dock.{open,hide,show,focus,resize,close,save,restore,state,list}` op 語彙を実装。Panel=Panel `plugin.dock`+NativeSlot `plugin.editor` の upstream 語彙に整合、Dock/Float は `AttachMode{Docked,Floating}`、Tab/Focus は `set_visibility`/`set_focus`、Save/Restore は論理 `PluginDockSnapshot`。検証: `tests/plugin_dock.rs` 25 tests + `mise run test synvelis-app` 全回帰 pass。doc chain: `docs/specs|test-specs|designs/sv-gpui-02-plugin-dock-headless-panel*`。 |
| 3 | PluginInstanceId/EditorSessionId/SurfaceSlotId/ViewInstanceIdの所有・世代・Open/Hide/ReDock/Closeと、DSP/Project/RT非干渉をMock/Fake Host/Windowの決定的テストで検証する。 | **merged-evidence pointer（L0/L1 部分）+ 充足（plugin-dock 結合部 — 2026-11-24）** | L0/L1 契約は `evidence/sv114/l1-contract-comparison.md` §1–2（再主張なし）。plugin-dock 結合部は `tests/plugin_dock.rs` が決定的に検証: `StablePluginInstanceId→PluginHandle→PluginEditorSessionId→SurfaceSlotId/lease→ViewInstanceId` の mint/所有・stale/revoked lease 拒否・共有 `PluginEditorSessions` の panel 横断 `SessionBusy`・Open/Hide/Show/ReDock（2-phase dock seam）/Close・window-loss/instance-term 収束・`plugin_dock_ops_do_not_touch_project_or_dsp_state` による DSP/Project/RT 非干渉 oracle。adapter 拒否・`Unsupported` は `Failed`/`Unavailable` に正直着地。 |
| 4 | Windows/macOS/Linuxの既存CIでNative Build/Link、必要GPUI/Test/Plugin Host Feature、PluginEditorSession/NativeSurfaceSlot/Fake PanelのHeadless試験を対象SHAで実走する。 | **充足（対象 SHA CI 実走 — 2026-11-25 Coordinator 証拠工程完了）** | 対象 SHA `71275d85747c05741787e8b20f0f7f3c4e51f287`（S1 merge 後 main）で `ci` workflow を `expected_sha` 指定 dispatch し全 lane 完走: Run `https://github.com/scarecrow173/synvelis/actions/runs/45482081443` — verify（windows/ubuntu/macos）・msrv・rt・gw10×3OS・ui×3OS・dep-guard・evidence・ci-summary（`verify-sha` contract + fail-closed gate）全て success。headless 試験（PluginDock/Fake Panel/session 系）は verify の `test-full`/`test-app-gpui` が対象 SHA で実走。S1 slice のローカル実走: `evidence/sv176/s1-headless-probe.md`（実 ABI test-plugin `HEADLESS_MATRIX` + product legs の Windows 実測）を同 SHA ローカルで記録済み。 |
| 5 | Ruviform #388/#389とSV #114/#28の公開Port・共通責務/Consumer UX境界を照合し、既知Windows DComp問題（RF #532）を未解消として正確に記録する。 | **merged-evidence pointer + 記録の再確認（2026-11-24 更新）** | `evidence/sv114/l1-contract-comparison.md` §3 が plugin surface `Unsupported` を記録済み。**upstream 状態変化の再記録:** `scarecrow173/ruviform#532` は CLOSED/COMPLETED に遷移（2026-11-24 `gh issue view` 実測）。ただし Synvelis 製品側受入（行 1・行 7 の PD-01〜07 実走）では未検証のまま — 行 1/7 が `NotRun` の間は「修正込み実走待ち」として扱い、「未受入」と「upstream open」の混同はしない。 |
| 6 | PD-01〜07の実VST3/CLAP/AUの画素/IME/Focus/Resize/Host hang・OS別L0/L1・実Native Dockは現行初期必須から除き、対象OSで実Plugin GUIの利用が具体的に必要になった時だけActivationする。Generic/Floating/MockをNative Dock Passとは記録しない。 | **merged-evidence pointer（方針記録）+ S1 実走で非昇格規則を確認（2026-11-25）** | 本 ledger・case definitions・test-spec PD-FRZ-02/03/07 がこの分離を文書化して凍結。実 Plugin GUI の Activation は利用要件発生時（POLICIES.md §2 Deferred）。S1 headless probe は実 VST3/CLAP test-plugin 上で実走したが in-process real-ABI のみ — Native Dock Pass とは記録せず、`editor-attach` は定義済み `Unsupported`（`createView` null / 非 HWND parent の定義済み拒否）に正直着地したことを `s1-headless-probe.md` が実測で確認（非昇格規則の動作検証）。Fake/Floating/Generic/Mock の非昇格は本 slice の全文書で明示。 |
| 7 | **Windows GUI-L2/EXT-L3（2026-11-15現行Required）:** WindowsでNative Plugin Dockを提供する場合はRF #630の正規ABI Test Native Editorの実Child HWNDと#631実GPUI Window Runnerを使用し、PD-01〜07の画素/Clip/Focus/IME/Resize/ReDock/2Window/Crash/RT非干渉をSynvelis製品Oracleで判定する。市販Pluginは通常必須にしないが、RF #532 DComp既知Failは修正・実走まで未受入。 | **NotRun（外部 Owner 主体・Required）** | Windows で Native Plugin Dock を提供する場合の Required 条件。実 child HWND・実 Window Runner・独立 Oracle の実行は `scarecrow173/ruviform#630`/#631/#532 待ち。DComp 既知 Fail は `evidence/sv176/pd-headless-contract.md` の Non-claims 節に open Fail/pending item として記録済み。 |
| 8 | **macOS/Linux L1必須:** Native Build/Link、Headless/RT/必要Featureを既存CIの同SHAで検証。実GUI/実Device/実Pluginは利用条件発生までDeferred。 | **充足（L1 — 同 SHA CI 照合完了）+ Deferred（L2/L3）** | 行 4 と同じ Run `https://github.com/scarecrow173/synvelis/actions/runs/45482081443`（SHA `71275d85`）で verify-macos・verify-ubuntu が全 step success — Native Build/Link・Headless/RT・必要 Feature の L1 実走を対象 SHA で照合済み（Coordinator 証拠工程 2026-11-25 完了）。macOS/Linux の実 GUI/実 Device/実 Plugin は POLICIES.md §2 により Deferred — PD-03（AU）は case definitions で Result=`NotRun`・要求状態=Deferred と記録済み。 |
| 9 | **OS/機能別Evidence:** Windowsの使用機能で実証できなければ未受入。未使用機器・Plugin/Featureを一律Gateにせず、実測/NotRun/Fail/NotRequiredを混同しない。 | **merged-evidence pointer（分類規則の凍結）+ S1 実走結果の分離記録（2026-11-25）** | 本 ledger 自体がこの分類規則の適用結果。S1 で `artifact`/`host-lifecycle`/`session-lifecycle`/`host-terminate`/`open-or-degrade` は実測 `Pass`（in-process real-ABI）、`editor-attach` は `Unsupported`（定義済み拒否）、`dock-pixels` は `NotRun`（`VisualCapture` 不在）と verdict を混同せず機能単位で分離 — `s1-headless-probe.jsonl` が記録する `verdict`/`environment.plugin_artifact`（sha256 属性化）がその実測根拠。`pd-01-07-case-definitions.md` の lane status summary に反映済み（`Deferred` は Result 語彙ではなく要求状態の別軸）。未使用 format/OS/capability は要求しない（PD-FRZ-05）。 |


## S2 native-slot authority slice（2026-11-26）

本節は `feat/176-plugin-dock-native-slot` の structural follow-up 実績を記録する。詳細な lane/verdict/non-claims は `evidence/sv176/s2-native-slot-authority.md` を参照。

- `PluginDock` から panel-private `SurfaceSlotRegistry` を取り除き、slot declaration・lease mint/validate/release・geometry/visibility・busy 制御を、composition root または test fixture から注入された単一の `NativeSlotAuthority` に集約した。`PluginEditorSessions` は app-shared のままである。
- lease redemption は session-carried `ParentSlotLease` を opaque に扱い、issuer-side で同じ authority に照合して `ParentLease::new(SlotLease::new(raw).seq(), token)` に写像する構造へ修正した。slot index / `SurfaceSlotId` generation を parent token へ写像する旧構造は残っていない。
- headless では共有 fake authority を使う `tests/plugin_dock_surface.rs` 7 tests と既存 `tests/plugin_dock.rs` 26 tests が、shared-only validation、foreign authority refusal、revoked/stale/forged lease refusal、index recycle、busy refusal、open/hide/show/redock/close、window-loss 収束、DSP/Project 非干渉を決定的に確認した。
- GPUI-simulated では canonical `GpuiDriver::surface()` 上の slot を `plugin.editor` key に bind し、renderer の layout/visibility projection と unbind 後 suppression Hidden を canonical ledger 上で確認した。これは `GpuiSimulated` semantic/layout evidence であり、real OS window/native container/child HWND/pixel/input evidence ではない。
- 受入分類は変えない。行 2・行 3 の structural gap を前進させるが、行 1・行 7 の Required real-window/native-dock gate は `NotRun` のまま、行 4・行 8 の統合 CI 判定は merge 後対象 SHA の Coordinator 証拠工程に残る。行 6 の非昇格規則どおり、headless/GPUI-simulated を Native Dock Pass と記録しない。


## S4 session-integration slice（2026-11-28）

本節は `feat/176-plugin-dock-session` の structural follow-up 実績を記録する。詳細な lane/verdict/non-claims は `evidence/sv176/s4-session-integration.md` を参照。

- editor-session deadline 評価と panel event drain を `ui_pump` の 1 つの authoritative seam に接続し、frontend op がない bare shell `Tick` でも内部 lifecycle op 経由で pump されることを manual clock test で固定した。`Failed{Timeout}` は exactly once で投影される。
- project new/open/reload 境界で旧 project の panel/session/lease/slot を収束する。host 不在・adapter close refusal は正直な termination cause へ落とし、shared authority 上の slot が `Released` になることを決定的に確認した。
- PD-07 では project bundle の `plugin-docks.json` sidecar に logical state のみを保存し、session/slot/lease/handle/token を含まないことを recursive key scan で固定した。restore 後 panel は `Closed` 相当であり、window adoption 後の明示 open だけが editor を再 open する。
- real ABI probe では ruviform#630 製 `test_gain` の `Unsupported` refusal が app 経路の `ui_pump` projection に exactly once で届くことを確認した。これは in-process headless evidence であり、real OS/native dock evidence ではない。
- 受入分類は変えない。行 2・行 3 の headless session-integration 証拠を強化し、行 6・行 9 の verdict 分離規則に従うが、行 1・行 7 は `NotRun` のまま、行 4・行 8 の統合 CI 判定は merge 後対象 SHA の Coordinator 証拠工程に残る。

## 分類サマリ

- **merged-evidence pointer**: 行 3（L0/L1 部分）, 5（記録の再確認 — `ruviform#532` CLOSED 再記録）, 6（方針 + S1 で非昇格規則を実走確認）, 9（分類規則 + S1 verdict 分離記録）
- **充足（headless — 2026-11-24 slice）**: 行 2, 行 3（plugin-dock 結合部; S1 real-ABI probe が同一 state machine の正直着地を実バイナリ上で補強 — `s1-legs/`）
- **充足（対象 SHA CI 実走 — 2026-11-25 Coordinator 証拠工程）**: 行 4, 行 8（L1 部分; Run `https://github.com/scarecrow173/synvelis/actions/runs/45482081443`・SHA `71275d85`・全 lane success + `verify-sha` contract pass。行 8 の L2/L3 は引き続き Deferred）
- **NotRun**: 行 1, 7（外部 Owner — `ruviform#630`/#631・#532 実環境ゲート）
- **本 slice で `Pass`/`accepted` にした行**: なし — S1 は実 ABI test-plugin 上の in-process headless 実走（`artifact`/`host-lifecycle`/`session-lifecycle`/`host-terminate`/`open-or-degrade` = `Pass`、`editor-attach` = `Unsupported`、`dock-pixels` = `NotRun`）を行 4/6/9 の記録と行 2/3 の補強証拠として追加したが、いずれの受入行も Native Dock/実 OS `Pass` には昇格していない（行 6 非昇格規則どおり）。行 2/3 の充足は前 slice（2026-11-24）時点のものを維持

## 外部 Owner 依存の一覧（本 ledger が `NotRun`/pending に留める理由）

- `scarecrow173/ruviform#630` — 正規 ABI Test Native Editor・Windows child HWND
- `scarecrow173/ruviform#631` — 実 GPUI Window Runner・`rf631.plugin-dock` 実走 lane
- `scarecrow173/ruviform#532` — Windows DComp 画素不具合（upstream issue は CLOSED — Synvelis 製品側 PD-01〜07 実走での検証は行 1/7 に従属）
- `scarecrow173/ruviform#388`/#389 — NativeSurfaceSlot / PluginEditorSession 公開 Port
- `scarecrow173/ruviform#294` — 両 Consumer 非回帰・candidate SHA 照合
- `scarecrow173/synvelis#114` — PD-01〜07 の実 OS/実 Plugin 受入統合
- `scarecrow173/synvelis#28` — Ruviform pin/consumer 追従
- ~~Coordinator 証拠工程 — merge 後対象 SHA の CI Run URL 照合（行 4/8）~~ **完了 2026-11-25** — Run `45482081443`（SHA `71275d85`）全 lane success で行 4/8 の L1 照合を解消
