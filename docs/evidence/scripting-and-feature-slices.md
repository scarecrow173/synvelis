# スクリプト基盤と個別機能スライス — 実行証拠

Synvelis の個別機能スライス群（スクリプト基盤 SV-SCRIPT-01〜04、recorder wire、binding controls、native shell/view mount、theme/preset、mixer gain、RCP selection、Ruviform pin 更新、独立 CI、webdav flake 是正、SV-GUI-05 product e2e）で実施された検証の記録をまとめる。headless/fake driver 上の Pass と実機・実 OS での受入は区別して記し、NotRun・Deferred の検証は実施済みと誤読されないよう明示する。

## synvelis#114 recorder wire

`tests/recorder_wire.rs` の headless E2E 12 test（test-local fake `Device`/`InputStream`・deterministic PCM・`UiRuntime` dispatch）が全て pass した。`mise run check/lint/test/build synvelis-app`・check-app-native/lint-app-native・check-app-gpui/lint-app-gpui・check-headless-deps・check-ruviform-rev・fmt-check・prek が全て exit 0 である。

review-protocol では cooperative reviewer 6 candidates + adversarial reviewer 9 candidates を read-only validator が source 照合し、B 全 9 件 CONFIRMED + `loop_start >= transport_start` 検証欠落を追加検出した。judge は CONFIRMED の wedge/orphan/abort 経路のみ修正した（commit retry wedge・stop partial-finalize wedge・commit outcome orphan・undo/redo gate・channels 上限 256・loop 位置検証・dead params 削除）。report-only 3 件（start 失敗の lane pin・共有 FaultLatch の全 stream recoverable 化・error code 分類 nit）は記録のみで修正していない。

実 device 列挙・実 GUI-L2・EXT-L3・Issue #114 残 AC は NotRun/Deferred を維持している。なお実験記録の本文は E4 で「12 test を実装」と記すが event log は 7→8→12 test の段階的増加を示しており、review round を跨いだ記述の揺れがある。

## synvelis#245 webdav flake

CI run 42938180676 の `verify (windows-latest)` で `tests/webdav.rs` 3 件が test 内最初の provider 呼出で `TransientIo`（`read head: os error 10054`/`10053`）に flake した。fixture env-gated logging で並列 3 worker を再現させると約 3 %/run で失敗し、失敗 conn は全て `read_request` の `WouldBlock`（os error 10035）で drop と観測された。Winsock の accepted-socket nonblocking 継承と特定し、`handle_conn` 冒頭の `set_nonblocking(false)` + accept loop の per-connection 一過性エラー分類で修正した。

回帰 test `fixture_serves_request_written_after_accept`（150 ms 遅延 request）は修正前 revert で決定的に `os error 10053` で fail、復元後 pass することを確認した。webdav 43/43 pass、並列ストレス 0 fail、fmt/check/lint/test/test-rt/test-msrv が全て exit 0 である。CI 実測は PR CI に委譲しており、単一 green は flake 除去の証明にならないと明記されている。

## synvelis#293 RCP semantic→selection

新規 `rcp_selection.rs` producer + `drive_selection_plan` + oracle 9/9 が green である。公開 API のみを使い、`ProjectController` + `StaticProcessorHost` + `GainProcessor` で engine plan 構築脚を実走した。RCP-EX-01〜05（publish/missing/exact 置換/offline gate/export 拒否/edit lifecycle/stale gate/edge kind/fail-closed）を網羅している。

RCP-EX-06（PCM/real plugin/OS/RT）の実機受入は NotRun のまま synvelis#296 RCP-GATE へ defer した。headless green は実機 Pass に昇格しない。

## synvelis#271 SV-SCRIPT-01

新規 crate `synvelis-script` に `ScriptApiModel`/`BindingSet`/`LuauStub`/`ScriptAnalyzer` port を実装し、56 test（analyzer 7・binding 13・golden 10・luau 9・model 17）が green である。canonical JSON+SHA-256 の digest 連鎖と fail-closed verdict を fixture で固定した。

## synvelis#272 SV-SCRIPT-02

`ScriptActionDefinition`/`ActionRecipe`/`RecipeSpec` と `compile_action_recipe`（literal-only `MacroStep` → 変更なし `MacroCompiler`）を実装した。script 78 test・automation 125 test・gateway 437 test（新規 `script_action` 4 件含む）が green である。batch recipe は `synvelis.operation.batch` → `ProjectOp::Composite` として commit され、expected-revision 1→2 の 1 CAS・1 Undo で member effect が原子的に復元されることを確認した。gateway bindings fallback 昇格は条件非該当で Deferred（非空 `bindings` は従来どおり `PreparationRejected`）。

## synvelis#273 workflow-plan（slice1）

`WorkflowScriptBuilder` + `import_workflow_document`/`WorkflowResolver`/`FileWorkflowResolver` を実装した。script 85 test・automation 141 test（importer 16 件）が green である。YAML `BASIC_WORKFLOW` と同値の Luau spec が同一 `WorkflowDefinition`・同一 `ExecutablePlan` digest・同一 step order を生成する golden を確認した。Luau→Rhai→YAML の 3 段参照が `mid__leaf__tracks` の二重 namespace step と virtual output 引継ぎで compile される。fail-closed 行列（cycle・version/digest/language drift・未知/不足/過剰 input・未知/未 emit output・duplicate id・depth/step 予算・path escape・未知 source・未来 Step 参照）は全て typed reject である。

## synvelis#273 luau-backend（slice2）

`synvelis-script-luau`（mlua 0.12.2 Luau feature、coordinator 承認で当該 crate のみ隔離）で実 VM 評価を結合した。script 97 test・script-luau 29 test（実 VM）・automation 184 test・cli 24 test が green である。review cycle で P1×3（sandbox escape＝`configure_luau` の暗黙 global install・lift DAG 爆発＝`MAX_LIFT_NODES` 追加・`SourceDrift` Display の char-boundary panic）+ P2×2 + P3×5 を修正し blocker 0 とした。`.luau` → `WorkflowDefinition`/plan digest が YAML 同値と一致する。lease 外変更 4 件は retro-approve として PR #505 本文に明記した。macOS/Linux vendored build は NotRun（Windows のみ検証）。interim adapter と RF-SCRIPT 正式 contract の整合は ruviform#559 gate（ruviform#692 で製品 port 追跡）で再検証する。

## synvelis#274 SV-SCRIPT-04

`AnalyzerRegistry`/`LuauPortAnalyzer`（port-level）/`ScriptEditorDocument`/`ViewType::ScriptEditor`/`StatusTone` diagnostic 語彙を実装した。automation `script_analyzer` 12 test・workspace `script_editor` 8 test・app `svscript04_script_editor` 8 test が全て pass である。headless/fake driver の Pass であり、実 GPUI・実 OS・実 Luau.Analysis FFI の Pass ではないと明記されている。

## synvelis#175-01 native shell binding

driver-neutral `NativeShell` + typed `BindingAdapter` + `synvelis-native` bin（`GpuiDriver` composition root）を実装した。`tests/binding.rs` 12 test・`tests/shell.rs` 10 test が pass し、workspace 全 lane が green である。`cargo check -p synvelis-app --features native` も pass（`proc-macro-error2` の future-incompat 警告のみ）。実 OS/GPU 上の GPUI 実行は未観測であり、FakeDriver は契約上の正しさのみを示す。native acceptance には別途実機検証が必要と記録されている。

## synvelis#175-02 view mount

`main_screen_spec`（Panel > Tabs、Seek→`transport/position`、Mixer Fader）の mount・attach・subscribe・gesture・receipt・save/reload・teardown を FakeDriver で意味照合する `tests/view_mount.rs` 12 test + workspace `project_length` 2 test が pass した。実機 Windows 実窓では Seek 描画・ドラッグ・Tabs pointer 切替（双方向）・Save/Reload ボタン・seek range の project length 追従（Reload 後 `project_length_seconds=1.5`）・close→shutdown が全て Passed である。`PrintWindow` が古いフレームを返すため `CopyFromScreen` を正とした。

Tabs キーボード切替は upstream FU-1 未実装で Unsupported、IME・非 Windows OS は NotRun である。Fader `mixer.gain` は当時 projection 元が無く `unsupported: true` を維持した（後に synvelis#185 で解消）。ruviform pin は `27698cb1` → `68bb8710` → `79d1db0a` → `20a24a66` → `769d595a` と複数回更新された。

## synvelis#177 theme/preset

`synvelis-theme` leaf crate + `synvelis-workspace::preset`（schema2 slot preset）を実装した。Rev.2 で RF385（`ruviform-ui-theme`・`rvf-dtcg-profile.v1`、ruviform main PR #397 squash `df45d10`）着地に伴い `UnresolvedThemeResolver` seam を実 adapter（`resolve_documents` → `TokenDocument::from_json` → `ThemeResolver::resolve` → `ResolvedTheme`）へ置換し、SV 側の token 系 validation を撤去して envelope-only + 原文保持 `ThemeDocument` へ縮退した。

`cargo test -p synvelis-theme` は 14 pass（出荷 5 資産が SV envelope + upstream parser 双方を通過、dark/light × compact/comfortable 全 4 組合せで `catalog.resolve` が `ResolvedTheme` を返す）、workspace 947 test は 0 fail（ruviform `5c0f16b` main 上）である。`synvelis-light` は `color.bg-active`/`color.bg-input` を含まない既知差分（dark 12 色 vs light 10 色、provisional）。真の schema-1 preset ファイル移行は既知 limitation である。

## synvelis#185 mixer gain fader

`MASTER_FADER_RAW` bootstrap + load oracle + v7→v8 migration（既存 issued Fader の well-known ID re-key）+ send gain adopt guard + Mixer Fader の supported kind 化を実装した。`ctl_01` 21・`sf_02` 16・`sv185_mixer_gain` 3・`view_mount` 13 test が pass した。実機 Windows で Fader 描画（pixel scan で track/fill/thumb 色確認）・実マウスドラッグ・Tabs キーボード Left/Right/Home/End・graceful shutdown が全て Passed である。pin `769d595a` → `863a7a9d`（26 sources 全て一致）。

webview 実窓対話・macOS/Linux/IME・`AutomationMode::Off` 実窓操作は NotRun である。既定 `AutomationMode::Read` で commit が base を書き込まないのは全 parameter target 共通の domain 契約として記録され、本 slice 固有の欠陥ではない。なお task 文書の status は `in-progress` のままだが impl record は `completed` で実績・検証は完了しており、文書間の status 不整合がある。

## synvelis#28 pin 更新 slice

863a7a9d → cc7d5296: 15 commits、公開 API 削除・変更なし、製品コード変更なし。`fmt-check` のみ main 既存破損 `routing_engine.rs` で fail し、スコープ外として報告した。9865cd57 → fd1d4d5f: 6 commits、全件 additive、`pub` 削除 0 件。`test-msrv`（rust 1.89）pass。test-app-webview で rustc `0xc0000409` が 1 度だけ発生し単独・post-merge 再実行で pass、pin 非起因の環境 flaky と切り分けた。0f5b5737 → b02942c2: 80 commits、公開 API の削除・signature 変更は実質なし。唯一の root re-export 削除は `ruviform-ui-test` の `scenario::visual` で item は module path で存続・synvelis 未使用。webdav test が `TransientIo` os error 10053 で 1 度 flake し、後に synvelis#245 で原因特定した。前任 Worker のプロセスクラッシュ 0xc0000409 で中断した dispatch を引き継ぎ、残っていた未コミット `Cargo.lock` 更新が pin 更新と一致することを確認して続行した。fd1d4d5f → ac10a2f7: 128 commits/108 files。破壊的変更は全て未使用面（ParentLease 語彙・wire v5・`MergeOutcome`・bridge counter/ledger 再構成・ui-test driver 語彙）。`Cargo.lock` 差分は 27 package の source fragment 54 行のみ。新規 crate（plugin-dock-test・plugin-test-vst3・audio-test・io-e2e 等）は全て依存木外で不採用とした。abb9f372 → cbcc8343: 12 commits、`pub` 削除行スキャン 0 件の純粋 additive。依存木内 4 crate の差分は全て additive/内部修正。`lint-app-native`/`lint-app-gpui` が synvelis#488 由来の既存 `clippy::large_enum_variant` で fail したが、feature 統合機構を `cargo tree -e features` で確認し pin 非起因と切り分け、`synvelis-workspace` は HARD BOUNDARY のため未修正とした。`ruviform-audio-test` は 2026-11-15 承認で dev-dep 依存木入り済み。audio-lens#127 が同一 SHA で rev-watch slice を実施中の tuple 整合も確認した。

なお `fmt-check` の main 既存破損（`routing_engine.rs`）は cc7d5296 slice（2026-11-09）で fail 記録されたが、同日以降の fd1d4d5f/b02942c2 slice（2026-11-11）では pass しており、間に修正が入ったと推測されるが明記はない。

## synvelis#83 独立 PR/Push CI

`.github/workflows/ci.yml` に 6 job（verify 3-OS・rt-verify・gw10-verify 3-OS・msrv-verify・dep-guard・evidence）+ composite action `ruviform-dep-access` + mise `test-rt`/`lint-actions`（actionlint 1.7.12）を実装した。ローカル検証は fmt-check・check・lint・test・gw10（G0〜GTV 全 gate PASS、SYS-02-ux/SYS-03-ux/SYS-05-powerloss/CONSUMER-* は NOT-RUN、SYS-07-rt は UNSUPPORTED）・test-rt・lint-actions・prek・rt_ 非空ガード・dep-guard ロジック単体試験・dep-access 匿名経路・test-msrv（rust@1.89.0）・doc chain audit が全て pass である。

GitHub runner 上の実挙動・macOS/ubuntu lane・deploy_key mode の実動作は未検証であり、actionlint とロジック単体試験のみで `CROSS_REPO_DEPLOY_KEY` 未設定のままである。

## SV-GUI-05 svt product e2e（synvelis#114 関連）

`tests/svt_product_e2e.rs` の SVT-01〜09 に対応する 10 test が pass（Windows・mise toolchain）した。GPUI lane（`svt_product_e2e_gpui.rs`、`gpui-test` feature）は別途 `GpuiGuiTest` で SVT-02/03/05/07 相当を実行した。実 Window/OS Input/Focus/IME/DPI/Pixel・native dock・実 plugin editor・実 Audio/MIDI は GUI-L2/EXT-L3 の別 gate（synvelis#176 系、Ruviform #313/#314/#388/#389 acceptance）で Deferred である。`CallOutcome::Job` を返す `syn.*` op は現行 AppService に job producer が無く Unsupported、`syn.app.info` は plugin editor session を正直に unavailable と報告する（fake success にしない）。

## 出処

この記録は synvelis#28、#47、#83、#114、#175、#176、#177、#185、#245、#271、#272、#273、#274、#293、#488 および ruviform#559・ruviform#692 の各 Issue に基づく。スクリプト基盤スライス、recorder wire、binding controls、native shell/view mount、theme/preset、mixer gain、RCP selection、Ruviform pin 更新、独立 CI、webdav flake 是正、SV-GUI-05 product e2e の各作業から抽出した。
