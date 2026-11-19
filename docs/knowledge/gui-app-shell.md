---
id: "KNOW-GDVlUcUErZOJEBbopQbLus"
type: "knowledge"
status: "draft"
lifecycle: "draft"
knowledge_kind: "codebase"
title: "GUI アプリシェル"
created: "2026-11-19"
updated: "2026-11-19"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/48"
    - "https://github.com/scarecrow173/synvelis/issues/102"
    - "https://github.com/scarecrow173/synvelis/issues/110"
    - "https://github.com/scarecrow173/synvelis/issues/111"
    - "https://github.com/scarecrow173/synvelis/issues/112"
    - "https://github.com/scarecrow173/synvelis/issues/113"
    - "https://github.com/scarecrow173/synvelis/issues/114"
    - "https://github.com/scarecrow173/synvelis/issues/176"
    - "https://github.com/scarecrow173/synvelis/issues/488"
    - "https://github.com/scarecrow173/ruviform/issues/260"
    - "https://github.com/scarecrow173/ruviform/issues/261"
    - "https://github.com/scarecrow173/ruviform/issues/376"
    - "https://github.com/scarecrow173/ruviform/issues/379"
    - "https://github.com/scarecrow173/ruviform/issues/383"
  informs: []
  explains: []
  supersedes: []
---

# GUI アプリシェル

## 概要

Synvelis のデスクトップ GUI は、`synvelis-app` crate を composition root とし、実窓 host に Ruviform の `ruviform-ui-webview` を `webview` feature で接続する構成で成立している。frontend は `ui/` の SolidJS + TypeScript + Vite pipeline で、Ruviform の plain TS Custom Elements を vendored `ui/vendor/ruviform` 経由で利用し、Rust との境界は `syn.view.*` / `syn.control.*` / `syn.viz.*` の typed op 面と bounded notify outbox が担う。本書は、このシェルの bootstrap・asset pipeline・view model・viz 契約・workspace GUI slice 群・multiwindow dock・close-veto・実機検証ゲート・control/recording op surface・lint 是正に関する設計決定と検証上の教訓をまとめる。共通する教訓は、upstream 共有 SDK の不具合を consumer 側 workaround で隠さず基盤側で修復して pin 更新で取り込むこと、headless/fake の Pass を実 OS 証拠へ昇格しない証拠規律、そして未実装経路を正直な失敗応答で返す op 設計の方針である。

## 詳細

### デスクトップ bootstrap と frontend asset pipeline

`synvelis-app` crate が composition root（`SynvelisApp` + `AppService` の `syn.*` op 面）を担い、実窓 host は Ruviform 既存の `ruviform-ui-webview` を `webview` feature で接続する。`ruviform-ui-webview` と `rust-embed` は optional 依存で、bin `synvelis` は `required-features = ["webview"]` を持つ。headless の lib/test は wry/tao 非依存を `check-headless-deps` で fail-closed に検査する（synvelis#110）。

frontend の依存経路は npm registry ではなく、Cargo.lock 解決 SHA のローカル cargo checkout から `ui/vendor/ruviform` を再生成する `sync-ui-deps`（`tools/sync-ruviform-ui.mjs`）が唯一の経路である。`check-ruviform-rev` が lockfile SHA・vendor `REV`・frontend manifest host の三者一致を検証し、不一致は stale vendor を検出する fail-closed として機能する（synvelis#110、synvelis#111）。`build.rs` が `Cargo.lock` から ruviform git source の解決 SHA を抽出して `SYNVELIS_RUVIFORM_REV` として焼き込み、runtime は永久 SHA を直書きせず lockfile を正本とする運用である（synvelis#110）。

asset pipeline は、vite build が `crates/synvelis-app/frontend/dist` に出力し、`synvelis-assets.json` manifest（host rev + per-file SHA-256）が rust-embed の `#[folder = "frontend/dist"]` で embed される。起動時に `check_bundle` が schema/revision/digest を検証し、不合格なら CSP 制限付きエラーページを表示して安全側に倒す fail-closed で、空 bundle での起動緑にはならない（synvelis#110）。CSP の `connect-src` は `'self' rvf: http://rvf.app http://rvf.data` で、`rvf://data` URL が webview 側で `http://rvf.data` origin に書き換わるため両 origin の許可が必要だった（design 初版で `rvf.data` 抜けを指摘され修正）（synvelis#110）。

frontend component test は vitest + happy-dom を採用する。`node --test` では `solid-js/web` が Node export condition で SSR build に解決され client render できないためである（synvelis#110）。RF-GT-03 test control port は upstream 未実装かつ release 成果物に test 制御を含められないため不採用とした（synvelis#110）。

実績として、実 Windows で `synvelis.exe` が `Synvelis` 窓を開き WebView2 が bundle を描画、`asset bundle ok` を通過し窓 close で exit 0 を確認済みである。macOS/Linux 実機・release embed lane・`rvf://data` pull の end-to-end は未検証として明示されている（synvelis#110）。

### view model・typed IPC と attach epoch の教訓

Rust 側は `syn.view.*`/`syn.control.*`/`syn.viz.*` op 面と bounded notify outbox（`VecDeque`、cap 超過で最古 drop + counter 記録）を持ち、host tick が outbox を webview へ drain する。frontend は upstream `BindingPort` を実装し、`dispatch`→`syn.control.dispatch`、`subscribe`→`syn.control.subscribe` + projection route、`query`→`syn.viz.query` に写像する（synvelis#111）。

`syn.control.dispatch` の op result は queue ack のみで、値の真実は `syn.control.projection` notify 経由という ADR 決定がある。receipt を値反映と誤読しない契約で、mounted positive control で「receipt `applied` 後も pending 維持・projection 着地でのみ解除」を実測した（synvelis#111）。notify channel は `{channel, data}` 形で `syn.control.receipt` / `syn.control.projection` / `syn.viz.frame` / `syn.viz.event` / `syn.view.state` を emit し、frontend は `onEvent` 単一購読の裏で `data.channel` 別 fan-out する router を持つ（synvelis#111）。

frontend の DOM 非依存 `ConsoleViewModel` が `ViewLink` + `CommandBinding` を所有し、attach/detach/close を `syn.view.*` op + link lifecycle へ写像する。object 値 prop は JSON attribute 化せず `setProp` 経由の real property assignment のみとする（synvelis#111）。Solid `onCleanup` だけでは await を跨ぐ mount の cleanup や late attach を保証できないため、cleanup 登録は有効な Solid owner の同期区間へ移し、attach/viz bind の各 await 後は disposed lifetime を再開しない設計に修正された（synvelis#111）。

attach epoch の教訓が重要である。`ViewLink` の generation が link 自身の attach 回数（local counter）で、backend `ControlsUi` の view epoch（`attach_view` 遷移で +1）と乖離すると、`ctx.generation < binding.generation` で dispatch が `stale_intents` 計上・receipt なしの沈黙破棄となり、subscription/projection だけが生きた「zombie console」になる。SDK 側は `ViewLink.attach(windowId, generation?)` で authoritative epoch を link・全 binding/query/subscription context へ採用する形に修復され（Ruviform PR #383、RF main `49de02c`）、consumer は attach 応答の `res.generation` を `link.attach` に渡す（synvelis#111、ruviform#376/#379/#383）。

恒久回帰の作り方にも教訓がある。fake host が SDK 送出値をそのまま受理するだけでは epoch 乖離を観測できないため、backend `ControlsUi` の epoch/stale-guard を独立 ledger として実装した `ui/test/backend-epoch.ts` を新設し、wire `ctx.generation` を ledger の backend live epoch と照合した。旧 SDK で 3 件 RED（意味のある assertion 失敗）→ 修復 SDK で GREEN の RED→GREEN を確認している（synvelis#111）。upstream 共有 SDK の不具合は Synvelis 側 workaround や backend stale guard の緩和で隠さず、Ruviform 側 Issue/PR で修復して consumer として pin 更新で取り込む方針が取られた（synvelis#111、ruviform#376→#379→#383）。

`provenance` は optional field として absent で送り、null は upstream validator が拒否するため conformance lane で検出・修正済みである（synvelis#111）。view close は Project/audio/render job を暗黙キャンセルしない。detach が gesture を cancel するが transport は cancel しないことを `detach_cancels_gesture_but_not_transport` で実測した（synvelis#111）。RF-GT-02 残 corpus（`ipc/intents`・`ipc/events`・`ipc/data`）は upstream Rust conformance test が同一 corpus を検証済みで、Synvelis 側は `syn.*` wire shape のみ追加検証し corpus 全量再実行は重複回避で行わない判断である（synvelis#111）。

### viz headless contract の構造

`synvelis-workspace::viz_ui` が generic な data/query/snapshot/lifecycle 契約を所有し、Track/Bus/Clip/Probe 等の DAW 意味論は canonical target id（`waveform:clip-<tag>` 等の opaque string）と timeline item builder に閉じ込める。`ruviform-viz` Rust crate（generic LOD/viewport/lane primitives）を workspace 依存として採用し、generic 実装の重複を避けた（synvelis#48、ruviform#260/#261）。TS `ui/viz` の element/scene/renderer は移植しない。upstream が canonical で renderer fork 禁止のため、headless mirror は model+lifecycle（ports/surfaces/pump/link/queries）のみを対象とする（synvelis#48）。TS `ui/viz` の `AbortSignal`/async query は async mirror として再現せず、querySeq+generation+revision 照合で同等の cancel/stale 保証を実現し、テストを完全同期・決定的にした（synvelis#48）。

waveform query path は span×plot_px に bounded で、bucket == plot px とし、LRU cache → miss で port query → byte budget 検査 → revision 二重照合（reply revision == issued revision かつ source の現在 revision が issued と一致）→ stale reply は 1 回 retry の後 `view_stale` + `VizStatus::Stale` という流れである（synvelis#48）。snapshot pump は attached meter/spectrum のみ走査し各 surface が port から最新 complete frame を pull する。新規 seq のみ landing し、republish は新データとして扱わない。これが producer idle で stale へ老化する唯一の機構である。producer `dropped` + seq gap を累積 loss として bookkeeping し、`staleAfterMs` 超過で stale に 1 回だけ遷移する。UI から RT へ同期アクセスせず、producer が port へ push、UI は pull のみとする（synvelis#48）。

review で検出・修正された教訓が 2 件ある。pump が同 seq frame でも `last_pull_ms` を refresh していたため「最新 frame が古い」判定が起きず producer idle で stale にならなかった。同 seq は landing せず age-check のみへ修正した（synvelis#48）。`WaveformSeries::view` は range を clamp してから bin するため、out-of-range pan で requested span 外側が EMPTY bin にならず bin が圧縮される。per-bin `view(c, s_i, e_i, 1)` reduce に変更して requested span をそのまま bin 分割するようにした（synvelis#48）。

JSON wire 上の non-finite f64 は serde_json で `null` に縮退する（NaN↔null narrowing）。gap marker 保持のため `serde_f64`/`serde_f64_vec` ヘルパで non-finite は JSON `null`、読み戻しは `NaN` に写像し全 f64 wire field へ展開した。struct wire 型は camelCase + `deny_unknown_fields` に統一する。internally-tagged enum に newtype-string variant（`SourceError(String)`）は serialize 不可能なため struct variant `SourceError { detail }` へ変更した（synvelis#48 review P1）。`synced_rev` sentinel `0` は「revision 0 と同期済み」と「未同期」を区別できないため `Option<u64>` へ変更した（synvelis#48）。

未対応として報告された既知制限が 2 件ある。`tempo_map.ppqn` vs `project.ppqn` の乖離は `synvelis-project` 側の invariant gap（`SetPpqn` が 960 固定のため現行では一致、crafted snapshot で乖離可能）で別 issue 報告対象である。`MemoryWaveform::query` の channels リスト乗算増幅は bounded-read 境界の port 側改善として follow-up とした（synvelis#48）。実 device GUI・WebView・GPU rendering・HiDPI・browser 実測 performance は manual gate で本 impl スコープ外と明記されている（synvelis#48）。

### workspace GUI slice 群の実装パターン

SV-GUI-03（synvelis#112）は Transport/Inspector・Arrangement・Mixer・selection/Inspector・scenario lane・keymap 契約・keymap dispatch の 7 slice で構成される。slice1（Transport/Inspector）は `MainScreenState` 系の共有 cell を `app.rs` に持ち、`ui_pump` 末尾の `capture_screen_state` が `controls.transport_row`/`strip_row` の `observed` を読み state cell に書込む。spec 更新は `PartialEq` dedup で push する。`service.rs` が編集範囲外のため `publish_project_length` の呼出名を維持し本体を state 更新に変更するなど、編集境界を守る調整を行った（synvelis#112）。

slice2（Arrangement read-only Table + Undo/Redo）は workspace に `arrangement_ui.rs`（`ArrangementRow` + `arrangement_rows`）を新設し、`timeline_items` を `pub(crate)` 化した。Undo/Redo は `syn.project.undo`/`redo` op として `project` target 経由である（synvelis#112）。slice3（Mixer strip master Mute/Solo Toggle）は、well-known raw ID が `pub(crate)` で app 層から参照不可・算術複製は private 契約依存のため、toggle target 発見を role+owner 走査にした。subscription は `main_view_targets` の静的一覧ではなく `ui_pump` の `reconcile_mixer_subscriptions` で project 依存値に収束的に追従させる ADR 決定である（synvelis#112）。

slice4（Arrangement selection→Inspector + Meter/Waveform viz mount）は、Arrangement `Table` の row selection intent を view-local `ObjectSelections`/inspection focus に反映し Inspector 表示 target を rebinding した。`syn.view.select` op を新設し、view-local params を `VIEW_STATE_TARGET` のみに許す既存 decoder 規則へ追加した。selection（編集選択）と inspection focus（観測）は workspace で意図的に別 state のまま維持し、表示だけ inspection に追随させた。meter level history は app 内部 bounded `VecDeque` で、`Meter` spec は resolved bins のみ運び query 契約を持たないため landed `MeterFrame` の蓄積が唯一の正直な供給源である（synvelis#112）。

slice5（GPUI-simulated scenario lane）は `tests/common/scenario.rs` に driver-neutral scenario helper を集約し `SemanticGuiTest` port のみで記述する。Fake lane（`tests/screen_scenario.rs`）と GPUI-simulated lane（`tests/scenario_gpui.rs`、`#![cfg(feature = "gpui-test")]`、`ViewHost` + `AttachEpochAllocator` + `create_view_window` の production composition で mount）で同一 scenario を実行する。`ScenarioReport` は adapter 実測の `driver()`/`capabilities()` をそのまま記録し、semantic pass を強い証拠へ昇格させない。製品コード変更なし・検証 lane のみの追加である（synvelis#112）。

slice6（Keyboard/Gesture 契約、docs-only）は「契約文書を先に正本化し実装は後続 slice が従う」アプローチである。`[contract]` id（`transport.toggle` 等 14 id）は `spec_for` 未登録の契約先行 id として命名し、登録は実装 slice へ Deferred とした。View/Application lane の suppression ゲート配置は「振る舞いを契約で固定・実装配置は Deferred」とした。既定バインドは `Mod` 論理修飾キーで OS 差異を吸収し、Export は既定バインドなし、auto-repeat は scroll/undo のみ許容とした（synvelis#112）。

slice7（keymap dispatch 実装）は、slice6 で正本化した契約の発火行列を `keymap.rs` の `lane_fires` 1 箇所に集約し、`dispatch` 側の既存 suppression（非 keymap caller 用）は維持する構成を採用した。`dispatch` の suppression 集合拡張案は Undo lane の context 別配送（`dispatch_undo`）と二重化・誤抑制リスクで棄却した。Undo lane は keymap 層で Modal/PluginGui 抑制のみ行い配送は `dispatch_undo` に委譲する。`PhysicalModifiers` は clippy `struct_excessive_bools` 対応で bitmask 化した。`edit.delete` は explicit_targets → selection snapshot → origin view active domain の順で解決し、`Clips` のみ `clip.remove` へ id 書換えて `self.dispatch` に再入する（1 dispatch=1 undo 単位維持）。他 domain は推測せず `Unresolved` とした。未実装経路（save/export/stop-return/record/copy/paste）は全て正直な `Unresolved{reason}` で fake success にしない。viewport は view-local・`Applied{revision: None}` で、`Esc` は完全 bare のみ reserved transient-cancel、auto-repeat は契約 id 単位（`view.scroll-*`/`edit.undo`/`edit.redo` のみ受理）である（synvelis#112）。

文書不整合が 1 件記録されている。workspace UX 仕様のキーボード節は Ime の Undo を「発火しない」とするが、現行 `dispatch_undo` は text lane へ配送する。契約は現行コード挙動に合わせ、実機検証を synvelis#114 へ委譲した（synvelis#112）。

実機確認の記録として、slice1 task は `run-app-native` で Play→Playing / Stop→Stopped / Inspector tab breakdown 表示を実窓 + ログ + OCR で確認した。slice2/3 は PR #199/#201 で main 統合済み、slice4 は PR #240 まで言及される（synvelis#112）。

### multiwindow workspace・dock・close-veto の設計

SV-GUI-04（synvelis#113）の precursor として、`synvelis-app` に multi-window 対応の headless `WindowHandle` 実装 `HeadlessWindowHost` を追加し、`WorkspaceManager` の dock 契約と `NativeShell` の view-origin 検証を OS 無しで通せる fixture（`tests/sv113_dock.rs`）を整備した。`ShellWindowHost` の単一 window 契約（`create_window` → `BackendUnavailable`）は不変である。app 層に独自型を置いたのは app→workspace の依存方向を保ち app 層の host port 契約を app crate が所有するためである。`eval` は未知/破棄済み window 宛を記録せず drop し、trait の delivery safety contract「silently dropped」と一致させた（synvelis#113）。

two-window slice1 は secondary view（`view-2`）を additive に追加し、`SYNVELIS_SECOND_WINDOW` env gate で native bin が 2 窓 mount する。env 未設定時は従来通り 1 窓 boot の非回帰を確認した。native 実機が環境要因で不可なら NotRun と記録し、fake/headless の Pass を実 OS 証拠へ昇格しない方針である（synvelis#113）。

slice2 dock transfer の設計では、window/view の権威を `WorkspaceManager` 一本に保ち、UI registries/bindings と shell の `views` map は成功した workspace op の投影とする。`synvelis-workspace` に `adopt_window`（外部生成 window を `veto_close: false`・`DetachViews` 既定で登録、host window 生成は呼ばない）と `adopt_view`（`ViewInstanceId`/`ViewType`/title を持つ view descriptor を dock へ登録、`next_view` 前進）を追加し、`WorkspaceError::DuplicateWindow` で重複登録を fail-closed 拒否する（synvelis#113、PR #208 設計チェーン）。

`UiRuntime` は window を生成できない（driver は shell 所有）ため、`syn.window.dock_prepare_new_window` は shell への `WindowRequest` キュー経由で非同期に window を作り、`Created` lifecycle payload の `dock_view` で pending destination を一意に束縛する。初版の「owner 以外の window 名を destination とみなす」緩い判定は無関係 window への stray attach を受理したため、self-review で発見され `pending == Some(win)` の完全一致に修正された（synvelis#113）。

`syn.window.dock_*` op family（prepare/commit/abort/detach/reattach/prepare_new_window）と dock-aware `syn.view.*` を実装した。`syn.view.close` は pending move abort → view 削除 → UI teardown の順である。`op_window_lifecycle` は `created`/`ready` で未登録 window を adopt し `dock_view` payload を `AwaitingWindow` の pending destination に記録、`destroyed` は `WindowCloseDisposition` を適用する（`DetachViews`→detach replay・`ReleaseViews`→close replay・`syn.view.close` 非 replay）（synvelis#113）。

dock-domain scoping は `dock_view_info` が `view-1`/`view-2` のみ `Some` を返し、それ以外の id（`console`/`meters` 等）は legacy UI-only path に残す判定を 1 関数に集約した。将来 `view-N` を追加する場合は `dock_view_info` への登録が必須で、未登録 id は legacy path に落ちるため誤 adopt は起きない（synvelis#113）。

close-veto は、`NativeShell` の `CloseRequested` が無条件 `Approve` していた部分を app-global guard query `syn.window.close_guard` 経由の vetoable decision に置換した。guard は未保存変更（`live_revision` vs `saved().revision`）・open recording transaction・`RecorderSlot::active()` を reasons として報告し、topology は持たない（`last_window` 判定は `driver.windows()` を持つ shell の責務）。shell は末尾 live window の close にのみ適用し、残存 window が共有 `ProjectSession` を保持するため非最終 window は無条件 `Approve` とする。query の op error・Result 欠落・malformed payload は fail-closed で `Veto` に倒す（`guarded` 初期値 `true` で分岐ではなく初期値で保証）（synvelis#113）。`syn.app.quit` の per-window `request_close` も同一経路を通るため複数 window の quit で一部 veto が決定的に発生する。veto 後の「保存しますか?」確認 UX は無く `CloseDecision::Defer` の非同期継続は non-goal である。`RecorderSlot::active()` は stub のままだが、open recording transaction（`control().recording()`）は実状態を読むため transaction 経路の veto は既に有効である（synvelis#113）。

### 実機検証ケースの freeze と証拠スキーマ

SV-GUI-05（synvelis#114）は証拠文書スライスとして SV-OS-01〜09 の real-OS ケース定義を Issue 本文を正本に freeze し、受入条件 8 行を merged-evidence pointer / `NotRun` / 外部 Owner に分類する acceptance ledger を固定した。実 OS/実 GUI/実 Plugin レーンは一切実行せず、全 Case・全 AC 行を `NotRun`/pointer/外部 Owner のまま維持し `Pass` への昇格はない（synvelis#114）。

SV-GPUI-02（synvelis#176）も同型の freeze slice で、PD-01〜07 の plugin-dock 受入ケースを Issue 本文「2026-11-07 横断レビュー確定仕様 v2」を正本に freeze し、受入条件 9 行を 3 分類した。optional impl leg として `rf631.plugin-dock` 語彙の headless view-mount contract test を Fake lane（常時）と GPUI-simulated lane（`gpui-test` feature）の 2 file で追加した。semantic `view-mount` Pass が Native Dock Pass と誤読されないよう、test 名・assert・report detail・evidence の全てで区別を固定した（synvelis#176）。

証拠スキーマは `Case × OS × plugin format × driver/backend/GPU × plugin artifact identity（PluginArtifactEvidence 6 dims）× host/repo SHA × Expected/Observed/Result/Artifact/Owner` で、Headless/Simulated・Real OS・Real Plugin ABI/Native Dock の別評価列を持つ。`PluginHostProbe` 系 case は依存追加が Ask-first のため `NotRun`/Deferred、`editor-attach` は upstream `createView` null の技術的 `Unsupported`、`dock-pixels` は `VisualCapture` 非存在で `NotRun` である（synvelis#176）。SV-OS-09 は plugin surface が製品層で unavailable の現状を `Unsupported` にせず #176 具現化まで `NotRun`（Deferred）とした。環境・機能不在は技術的非対応ではないという POLICIES.md §3 の規律による（synvelis#114）。

upstream `rf631.plugin-dock` の 8 case 語彙（`view-mount`/`artifact`/`host-lifecycle`/`editor-attach`/`session-lifecycle`/`host-terminate`/`open-or-degrade`/`dock-pixels`）は `ruviform-ui-test`（pin `ac10a2f7`）export が正本で、`ruviform-plugin-dock-test` は参照のみ・依存ではない。upstream 進行中（ruviform#630/#631・#532 未解消）のため pin 更新で export 形状が変わる可能性をリスクとして記録している（synvelis#176）。

### control/recording op surface と serde wire 契約

UI-C5（synvelis#102）の知見は次の通りである。`ControlEntityRef` は serde impl を持たず編集範囲外のため、per-view focus の永続化には tagged 文字列（`param-…`/`automation-lane-…`/`mod-source-…`/`mod-assign-…`）を採用し `control_entity_tag`/`control_entity_parse` で round-trip する。4 種の id parser は prefix で一意に確定し、malformed tag は `None` へ劣化する（synvelis#102）。

`capture()`/`restore()` は in-memory の typed struct をそのまま受け渡すため serde attribute を通らず、`#[serde(skip)]` の explore stacks も生き残る。永続化契約が意味を持つのは JSON 境界（`to_json`/`from_json`）のみで、`signal_flow` と同じ構造である（synvelis#102）。`#[serde(default)]` のみでは未知 field を無視して decode 成功するため、`deny_unknown_fields` を併用して未知 shape は decode error → `Option` フォールバックで `None` へ落ちる tolerant decode 契約に統一した（synvelis#102）。

`control_edge_commit` の `Stale` は異常ではなく gesture の契約上の終了形態のため、`Result` の error にせず応答 `outcome` field の typed 値（`applied`/`cancelled`/`stale`）として返す。`RecordCommitError` は真の失敗として error detail に verbatim で返す（synvelis#102）。`syn.recording.status` の `Prepare` state は caller-held `PreparedRecordingCommit` が必要で構造上 op 経由では到達不能であり、無理に payload を op state に持ち込まず op doc に非到達を明記した（synvelis#102）。app 層は wire parse/serialize と op dispatch のみを担い domain semantics の複写は行わず、commit ロジックは workspace の `control_record_*`/`control_edge_*` に一元化する（synvelis#102）。

### lint feature-unification による enum サイズ肥大化

sv488（synvelis#488）では、`mise run lint-app-native`（`synvelis-app --features native` の clippy）が `clippy::large_enum_variant` で失敗した。原因は `synvelis-app` の `native` feature が `gpui → gpui_perf` 経由で `serde_json/preserve_order` を feature unification し、`serde_json::Map` が `IndexMap` 化して `Value` が肥大化、`ImportSubmit` variant が ≥305B（vs `ImportRebase` ≥80B）に達したことである。`lint synvelis-workspace` 単独では `preserve_order` が無効で発火しない非対称性があった（synvelis#488）。

修正は `ImportSubmit.target` を `Option<ImportTargetWire>` → `Option<Box<ImportTargetWire>>` に field 単位で Box 化したものである。`Option<Box<T>>` は serde に透過的で JSON shape 不変である。variant 全体の newtype Box 化は internally-tagged enum の newtype variant と `deny_unknown_fields` の相互作用検証が必要で diff も大きいため不採用とし、`#[allow]` は Issue 受入条件で禁止されている（synvelis#488）。feature unification の影響範囲（workspace 内の `serde_json::Value` 保持構造のサイズ監査）は未棚卸しで、同種 lint 再発リスクは残るが gpui 依存グラフ由来で synvelis 側の制御外として Deferred Design Concerns に記録した（synvelis#488）。

### 検証 lane と依存同期の運用規約

検証 lane の正本は `mise run` 系である。`fmt-check`/`check`/`lint`（`-D warnings`）/`test` に加え、feature 別に `check-app-webview`/`lint-app-webview`/`test-app-webview`、`check-app-native`/`lint-app-native`/`build-app-native`、`check/lint/test-app-gpui`（`gpui-test` feature）、`test-rt`、`test-msrv`（rust@1.89.0）、`build` がある。補助証拠として単体 `cargo test` は使うが canonical = mise で複数文書で一貫している（synvelis#110/#111/#112/#113/#488）。

ruviform 依存は既定ブランチ追従の git 依存で解決 revision は `Cargo.lock` に記録する。`Cargo.lock` の ruviform 解決 rev が変わる変更を取り込んだ後は `mise run sync-ui-deps` で `ui/vendor/ruviform` を再同期し、`check-ruviform-rev` の REV 不一致は stale vendor を検出する fail-closed である。特定 SHA の恒久ハードコード・vendor コピーの手編集は禁止である（synvelis#110/#111）。

POLICIES.md §2/§3 の証拠規律では、OS-L1 必須・GUI-L2/EXT-L3 は提供機能のみ Required・未使用は要求しない。Pass/Fail/NotRun/Unsupported/Blocked を分離し、Fake/Headless の Pass を実機証拠へ昇格しない。環境・機能不在は技術的非対応（Unsupported）ではない（synvelis#114/#176）。実 OS 証拠は env-gated native lane（`SYNVELIS_SECOND_WINDOW` 等）と Issue gate（#114 real-OS、#176 plugin dock）に分離する（synvelis#113/#114）。

文書チェーンの運用では、spec→design→plan→task→test-spec→impl(ir/exp) の canonical front matter 鎖を作り、`doc_maintenance.js index apply`/`sync_impl_index.js` で README index を再生成する（手編集しない）。`doc_status.js lint/audit`・`audit_impl_record.js`・`audit_experiment_log.js` で deterministic finding を確認する。`route_graph.js` は repo 全体の既存 broken `defers` relation（main 由来）で fail-closed `blocked` を返すため、該当 slice では coordinator 指定の文書鎖を手動で踏襲し偽装しない（synvelis#112/#114/#176/#102）。review-protocol は独立 reviewer（cooperative/adversarial の分離 pass）+ Validator/Judge で diff を adversarial review し、confirmed P0/P1 は全件修正して regression test で固定する。「観測 N 件確認」は「N 個の新規欠陥」ではないと明記し、現状値を assert する観測 probe を canonical suite の成功数に合算しない規律がある（synvelis#111/#48/#176）。

status 管理の教訓として、Task status の正規値は `done`（`completed` から正規化）であり、`in-progress`/`draft` の文書と `completed`/`done` の文書を区別して未完了計画の記述を実績として書かない。IR 文書の履歴異常として、sv-gui-02 IR は既存の複数 task をまとめる relation と created/updated=2026-11-25 の将来日付を「履歴上の既存異常」として残し、根拠のない日付変更や task 分割を行わない判断が記録されている（synvelis#111）。flaky の扱いとして、`test-msrv` で `synvelis-library` webdav の `same_origin_redirect_is_followed` が OS error 10053 で 1 件失敗したが、同 toolchain の単体 retry と全量 rerun で Pass し、変更範囲外の flaky と判定して記録した（synvelis#112）。merge 前の定型は `git fetch origin` → `git merge origin/main` → 生成物再生成（`Cargo.lock`・`ui/package-lock.json`・`ui/vendor`・`frontend/dist`・`mise.lock`）→ 全 mise 検証 → review-protocol → Conventional Commits → PR（日本語本文、`Refs #N`、tracker Issue は Close しない）である。並行 dispatch との分離では、編集範囲が他 dispatch と衝突する場合はファイル単位で lease を分け（例: `host.rs`/`lib.rs` re-export 1 行/`tests/`/`docs/` のみ）、`service.rs` 等の共有ファイルは編集しない判断を plan/task に明記する（synvelis#113）。

## 適用対象

この知識は Synvelis の GUI 層全体に影響する。`synvelis-app`（composition root、webview/native bin、asset embed、window/dock host）、`synvelis-workspace`（`viz_ui`・`arrangement_ui`・keymap・control/recording op・dock contract）、`ui/` frontend（SolidJS view model、`BindingPort` 実装、keymap contract）が対象である。境界として Ruviform の `ruviform-ui-webview`・`ruviform-ui-test`・`ruviform-viz` など upstream 共有 SDK・crate との依存方向と pin 更新手順を規定する。検証面では headless/fake/GPUI-simulated/real-OS の各 lane と `mise run` タスク群、および証拠 freeze ledger の分類規則がこの知識の適用範囲である。

## 出処

GUI アプリシェル構築エピック群（SV-GUI-01〜05: synvelis#110/#111/#112/#113/#114、SV-GPUI-02: synvelis#176）、viz headless contract（SV-48: synvelis#48）、UI-C5 control/recording op surface（synvelis#102）、lint feature-unification 是正（sv488: synvelis#488）からの抽出である。Ruviform 側の関連 Issue/PR は viz contract（ruviform#260/#261）と attach epoch 修復（ruviform#376/#379/#383）である。
