# S3 実ウィンドウ preflight: Windows PD-01 / PD-02

2026-11-28、[Synvelis Issue 176](https://github.com/scarecrow173/synvelis/issues/176) の試験準備記録。
この文書は実行結果ではない。PD-01（VST3）と PD-02（CLAP）は、Windows で提供する Native Dock の
GUI-L2 / EXT-L3 として `Required`、Synvelis の実行結果は両形式とも `NotRun`。受入台帳の行 1・7 は未達であり、
DComp の既知 Fail は修正を含む製品実走で反証するまで解除しない。macOS/Linux は OS-L1 が `Required`、
実 GUI・実 Plugin（PD-03 の AU を含む）は利用要件が生じるまで `Deferred` / `NotRun`。
個別 Case の正本は [PD-01〜07 定義](pd-01-07-case-definitions.md) と
[検証仕様](../../docs/test-specs/pd-01-07-plugin-dock.md)、判定語彙は [POLICIES.md §2–3](../../POLICIES.md) に従う。

## 照合したリビジョンと DComp

| 対象 | 照合結果 |
| --- | --- |
| Synvelis | main `bf0442cfad73c5102d137f3f1e20ac74d84860b3`。[PR 674](https://github.com/scarecrow173/synvelis/pull/674) の共有 slot authority を含む。[S2 証拠](s2-native-slot-authority.md) は GPUI-simulated であり、実 parent の redeem や画素を実測していない。 |
| Ruviform / GPUI | `Cargo.lock` は Ruviform `511e1a7944c35797942ba065c092b9263067c48a`、GPUI `v0.2.2-rf2` (`b3144f286b942fbb5f478da73db13a826cef054a`) を解決する。[RF 630](https://github.com/scarecrow173/ruviform/issues/630) の実 ABI fixture と [RF 631](https://github.com/scarecrow173/ruviform/issues/631) の実窓 runner はこの Ruviform rev に存在し、両 Issue は CLOSED / COMPLETED。 |
| RF 532 | [上流の採否](https://github.com/scarecrow173/ruviform/issues/532) は CLOSED / COMPLETED。採用方式は GPUI fork の clip partition。DComp flip-model の提示面で hosted child HWND の領域を覆わないようにする。[固定依存の上流実測](https://github.com/scarecrow173/ruviform/blob/511e1a7944c35797942ba065c092b9263067c48a/evidence/rf532/pinned-dep-results.md) では DComp 有効・docked-in-place で `real-dcomp-rasterfix` が Pass。runner 自身による別窓の遮蔽は [RF PR 861](https://github.com/scarecrow173/ruviform/pull/861) の観測修正で処置され、当該修正も固定 rev に含まれる。`WS_EX_LAYERED` 適用不能の診断は別件。 |

修正済みの GPUI rev が lock にあることは、製品の組込み実測ではない。Synvelis の Mixer から同じ実 editor を開き、
通常の DComp 設定で clip・resize・tab 後の画面合成を確認する。[受入台帳](acceptance-ledger.md) の過去の
`NotRun` は上流 Issue の close だけでは更新しない。

## 固定 pin の fixture と上流対照 lane

| Case | 選ぶ descriptor と実ウィンドウ | runner 入力と照合する行 |
| --- | --- | --- |
| PD-01 | `ruviform_plugin_test_vst3.dll` の `test_editor` Audio Module Class。`createView("editor")` の `IPlugView` が子 HWND を作る。Controller class や GUI のない `test_gain` は選ばない。識別色は緑。 | `RUVIFORM_PLUGIN_DOCK_TEST_VST3`。`rf631.plugin-dock-real` の `real-editor-attach`、`real-dcomp-pixel`、`real-editor-visibility`、`real-editor-resize` を対照にする。 |
| PD-02 | `ruviform_plugin_test_clap.dll` の `org.ruviform-test.clap-test-editor` (`test_editor`)。`clap.ext.gui` の `create` / `set_parent` / `show` で子 HWND を表示する。識別色は青。 | `RUVIFORM_PLUGIN_DOCK_TEST_CLAP`。`--format clap` で `rf630.plugin-dock-real-clap` の対応する `real-clap-*` 行を別 JSONL に出す。 |

別の実行担当が固定 Ruviform rev の専用 checkout と対話デスクトップで実行する正規コマンドは、
`mise run test-ui-plugin-dock-real` と `mise run test-ui-plugin-dock-real-clap`。各タスクは対応する
`ruviform-plugin-test-vst3` / `ruviform-plugin-test-clap` を `--locked --features l1-runner` で build し、
`ruviform-plugin-dock-test` の `plugin-dock-real-scenario` を `--features real-dock` で起動する。
既定出力は `evidence/rf631/plugin-dock-real-scenario.jsonl` と `evidence/rf630/plugin-dock-real-clap.jsonl`。
証跡の上書きを避ける必要があれば、同じ build の後に runner を直接呼び、形式別の新規 `--out <jsonl>` と
`--pixel-artifacts <dir>` を指定する（補助経路。引数・exit の契約は
[固定 rev の runner](https://github.com/scarecrow173/ruviform/blob/511e1a7944c35797942ba065c092b9263067c48a/crates/ruviform-plugin-dock-test/src/bin/plugin-dock-real-scenario.rs)）。

上流 runner は明示した DLL の絶対パスを優先し、不正な明示パスから自動 fallback しない。
未指定時は runner の `deps/`、次いで runner のディレクトリにある規定名の DLL を探す。
`l1-runner` の EXE は runner の隣または一階層上で探す。この EXE は隔離・障害系の別経路であり、
上表の実 Dock は `InProcessEditorOps`（Host L0）で行う。[S1](s1-headless-probe.md) の hashed `deps` DLL
探索は Synvelis テスト側の補助であり、上流 real runner や製品の探索仕様ではない。
上流の終了コード 0 は `NotRun` / `Unsupported` を含み得る。`--blocked-probe` も実窓を一切開かない。
JSONL の Case ごとの verdict と独立観測を読み、上流結果は製品結果に転記しない。

## Synvelis の Host と起動経路

- `composition.rs` の `ProductPlugin` / `ProductOutOfProcessPlugin` と
  `compose_routing_engine_with_out_of_process` は既存の本番 routing API。
  `mise run test-pc-c4-l1` は実 VST3/CLAP の `test_gain` を使う L1 DSP/binding 試験であり、
  この API を通るが実 Editor は開かない。`synvelis-native.rs` の `compose_product_routing` は
  `SYNVELIS_PROJECT_DIR` があるとき空の factory 群で engine を作る。ないときは engine stub のまま。
- `synvelis-plugin-test-{vst3,clap}-l1-runner.exe` は配布ランタイムではなくテスト用。
  既存のテストは `SYNVELIS_TIM03_L1_{VST3,CLAP}_RUNNER` または Cargo の profile ディレクトリから EXE を選ぶ。
  子は引数を受けず `SYNVELIS_TIM03_L1_PLUGIN_DIR` または EXE の隣から規定名の unhashed DLL を dlopen する。
  親が検査した DLL と子が読んだ DLL の SHA-256 を一致させる。これらの環境変数は `synvelis-native` の
  plugin 登録設定ではない。
- 固定 rev の `L1EditorOps::open` は `AttachMode::Docked` を `Unsupported` とし、子の wire v6
  `GUI_FLOATING` は子所有の Floating のみを許す。この制約は
  [固定 rev の host](https://github.com/scarecrow173/ruviform/blob/511e1a7944c35797942ba065c092b9263067c48a/crates/ruviform-plugin-host/src/editor_session.rs)
  に定義される。L1 routing が利用できても L1 Dock の配線だけで解消できる制約ではない。
  初回の PD-01/02 候補は本番 L0 で同一 instance を処理・編集する経路。L1 Dock が提供条件なら、
  Ruviform の Host/Export 契約拡張と Synvelis 組込みを別途要する。L1 の Floating 障害試験は Dock の代用にならない。
- `SynvelisApp::set_plugin_editor_host` と `set_native_slot_authority` の受け口は存在するが、
  現行 `synvelis-native` は注入・実窓への `plugin.editor` bind を行わない。Mixer にも実 insert から
  editor を開く操作がない。`syn.plugin_dock.open` がサービスに存在しても host/authority 不在時は
  `unavailable` であり、`syn-ctl drive --op` の IPC 注入は Mixer の実入力でもない。

この状態では、環境変数を設定して既存の `mise run run-app-native` を起動するだけでは製品 Case を実行できない。
製品用 PD-01/02 コマンドは未実装。

## 実走の手順と判定

1. **環境を固定する。** Windows x86_64 のログオン済み・アンロック済みの対話デスクトップを確保する。
   ビルド SHA、Ruviform/GPUI rev、Feature と Host mode、Windows build、GPU/driver、モニター・DPI、
   セッション種別と DComp 設定を記録する。非対話 CI、Session 0、ロック画面や切断 RDP で代替しない。
   `GPUI_DISABLE_DIRECT_COMPOSITION` を使う場合は原因切り分け専用とし、通常設定の合否に流用しない。
2. **実バイナリを選定する。** 同じ pin/arch から build した各 DLL を一時 fixture ディレクトリに置く。
   製品が実際に読んだ絶対パス、SHA-256、`plugin_id`、`name`、`vendor`、`version` の 6 項目を記録する。
   VST3 の ID は Audio Module の scan 結果を使い、名前だけで選択しない。
   製品では本番 scanner/loader を通す。製品が標準拡張子・bundle を要求するなら、その staged layout で
   scan/Load を確認し、上流 runner が裸の DLL を開けることを製品の load path 証跡としない。
3. **Mixer から開く。** 使い捨ての Project に各 `test_editor` を 1 つずつ挿した状態を用意し、
   形式ごとに別 run で Mixer の insert を実 pointer/key 入力で選択して Dock を開く。
   `StablePluginInstanceId` → live `PluginHandle` → `PluginEditorSessionId` → `SurfaceSlotId` と
   `ViewInstanceId` の対応・generation、同一 authority の lease、製品 Command ID / Receipt を記録する。
   独立 Win32 観測で実 child HWND の class・親 container・root window・所有 PID と表示状態を確認する。
   別の test-only host に同じ DLL を二重 Load して作った editor は無効。
4. **Tab・Resize・Clip・Focus を観測する。** 開く前、開いた後、非選択 tab に切り替えた後、復帰後、
   小・大の二つのサイズへ変更した後、閉じた後を記録する。表示 bounds と物理画素への変換を
   OS の geometry から求め、デスクトップの合成後画素（上流は screen-DC `BitBlt`）を
   slot 内と隣接領域で取得する。slot 内の可視領域に fixture 色を検出し、非表示時と clip 境界外に漏れないことを
   検証する。上流の child 可視判定は色一致率 `>= 0.5`。製品側では capture 前に期待矩形と
   許容差を定め、矩形・色比率・PNG/hash を保存する。`PrintWindow` や child DC は診断用であり、
   合成後の画面証拠ではない。CLAP は tab の hide/show 後も同じ子 HWND が生存することを期待し、
   VST3 の再 attach 時に同じ HWND であることは要求しない。
5. **入力と製品状態を別に検証する。** 実操作に対応する keyboard/focus の到達先と結果を OS の
   focus/GUI thread 情報、fixture のイベント、製品の選択状態で観測する。座標、入力時刻、対象、前後の focus を
   記録し、tab 復帰時のフォーカスと DAW shortcut の誤発火を確認する。Receipt は同じ Command ID の
   製品発行分を突き合わせ、harness が作った Applied は除く。独立 Project oracle で選択、insert/instance、
   graph/undo・文書の不変部分を前後比較する。意図した UI layout の変化は別に記録し、DSP/RT は固定入力で
   PCM・処理進行・異常を比較する。IPC 応答や画素の一致だけでは RT 非干渉を証明しない。
   IME、2Window/ReDock、Crash/Hang、Save/Restore の全条件は PD-04〜07 側で別途実走する。

## 失敗時の証跡と片付け

- 未構築の load/host/audio/desktop 条件や欠落した観測では対象項目を `NotRun`。
  技術上の明示的な拒否は理由を添えて `Unsupported`。実行して期待に反した場合は `Fail`。
  対象 HWND と有効な合成 capture が揃っているのに fixture 色が見えない・clip から漏れる場合は `Fail`。
  別窓が capture を遮る場合は遮蔽を記録して取り除き、観測可能な状態でやり直す。
- 失敗時は fallback する前に、同じ run ID で stderr、製品/子プロセスの exit code、部分 JSONL、
  HWND 親子・可視・DPI、合成画素、Receipt、Project/RT の前後値を保存する。No exit/timeout はそのまま記録する。
  VST3 と CLAP の行を混ぜず、対象 SHA・コマンド・期待値・観測値・判定・証跡の場所・担当を残す。
  Generic/Floating に復旧できても Native Dock の失敗を置き換えない。
- 正常時は editor を閉じ、session/lease/slot の解放と子 HWND 消滅を確認し、fixture を unload して終了する。
  hang 時は証跡保存後、この run が起動した PID のみに絞って後続の終了処理を行い、残存プロセス・窓を確認する。
  保存するのは証跡のみ。使い捨て Project と staged fixture は製品データから隔離し、
  一時設定と DComp/入力環境を復元する。共有 plugin ディレクトリや利用者の Project は変更しない。

## 必須の残作業と任意の補強

| 区分 | 担当・変更候補 | 完了条件 |
| --- | --- | --- |
| 必須: 製品 load・Host wiring | Synvelis Issue 101 と 176。`crates/synvelis-app/src/bin/synvelis-native.rs`、`composition.rs`、必要な host 側は `crates/synvelis-player/src/host.rs`。 | fixture を隔離した場所から本番 loader で scan/Load。routing と editor が同一の live instance を使う。L1 Dock を提供するなら Host/Export の対応は RF 388/389 側で別途実装・実証。 |
| 必須: Mixer・実 slot | Synvelis Issue 176。`crates/synvelis-app/src/{screens,ui_ops,plugin_dock}.rs` と native composition。 | Mixer の操作を製品 Command に結び、canonical `GpuiDriver::surface()` を注入し、製品の `plugin.editor` と実 slot を bind/unbind。公開 Port の責務は [L1 契約照合](../sv114/l1-contract-comparison.md) に従い HWND/合成実装は Ruviform driver 内に置く。 |
| 必須: 製品実窓と証跡 | Synvelis Issue 176 / 114 と対話実行担当。`crates/synvelis-app/tests/sv_os_real.rs` / `tests/common/sv_os_catalog.rs` を参照し、製品 PD の scenario・出力と opt-in `mise.toml` の入口を追加する。 | Ruviform の実 OS 観測を再利用し、製品 Host、真の Receipt、画素、Project/RT を同一 run で突合。既存 SV-OS-01 の入力・capture と harness-posted Receipt はそのまま製品 PD に転用できない。PD-01/02 実測後も他の Required Case は別判定。 |
| 必須: 対象 SHA の OS-L1 | Synvelis CI / Coordinator。`.github/workflows/{daily-ci,ci}.yml` の既存 verify と `ci-summary`。 | `test-full`、`build-app-native`、`test-app-gpui` 等の 3 OS 同一 main SHA の実走を確認。不足する Feature/fixture prerequisite は既存 job に補う。旧 SHA の [受入台帳](acceptance-ledger.md) や Daily の green は新 SHA の証拠でも実窓証拠でもない。 |
| 任意: 追加対照 | Ruviform / Synvelis の実行担当。上流の診断 PNG・動画、追加 GPU/DPI、市販 Plugin、物理機器、Windows opt-in の CI 自動化。 | 基本 Case の必須観測は省略しない。自動化がなくても明示承認のある対話環境で証拠を採取できる。この文書作業で GUI・手動 CI は起動しない。 |

## この文書変更のローカル検証

上記の Synvelis main を base とする文書差分の検証。実行環境は Windows 11 Pro x86_64 26H2、build `26300.9974`。

| コマンド / 対象 | exit | 結果 |
| --- | ---: | --- |
| `mise run check-ruviform-rev` | 1 | `Fail`。worktree の既存 vendor `REV` が `20a24a66ee90f4d462224391e78c10c93f4f31ba`、lock が `511e1a7944c35797942ba065c092b9263067c48a`。変更禁止のため同期せず、実走担当が別工程で `mise run sync-ui-deps` 後に再確認する。 |
| `mise exec -- prek run --all-files` | 0 | `Pass`。構成・整形・actionlint。相対 Markdown リンクの参照先と `git diff --cached --check` も確認した。 |
| Ruviform 実窓・Synvelis PD-01〜07・対象 SHA の統合 CI | — | `NotRun`。GUI を操作せず、runner・pin・CI を変更しない。統合 CI の照合は merge 後の対象 SHA で行う。 |

ローカルの文書検証は GUI-L2 / EXT-L3 または OS-L1 の製品受入を意味しない。
