---
id: "TASK-pIf6c5QRXRonGNwoqfvJgY"
type: "task"
status: "done"
title: "SV175-02: Native view mount — 最小 Tab/Panel/Fader 縦slice"
created: "2026-11-07"
updated: "2026-11-08"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/175"
    - "https://github.com/scarecrow173/synvelis/issues/112"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-app/src/screens.rs"
      - type: "file"
        path: "crates/synvelis-app/tests/view_mount.rs"
    modified:
      - type: "file"
        path: "crates/synvelis-app/src/app.rs"
      - type: "file"
        path: "crates/synvelis-app/src/shell.rs"
      - type: "file"
        path: "crates/synvelis-app/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-app/src/bin/synvelis-native.rs"
      - type: "file"
        path: "crates/synvelis-app/Cargo.toml"
      - type: "file"
        path: "Cargo.lock"
      - type: "file"
        path: "mise.toml"
    deleted: []
    renamed: []
    moved: []
    generated:
      - type: "file"
        path: "ui/vendor/ruviform"
  implements:
    - "docs/plans/sv175-native-shell.md"
  implemented-by: []
  depends-on:
    - "docs/specs/sv-gpui-01-native-app-shell.md"
    - "docs/designs/sv175-native-shell-design.md"
    - "docs/tasks/sv175-01-native-shell-binding.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/sv175-02-native-view-mount.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/sv175-02-native-view-mount.md"
  references: []
  defers: []
  deferred-by: []
---

# SV175-02 実装タスク

SV175-01 で `NotRun` と記録した最小 Tab+Fader slice を、RF387
(`68bb8710` で実装、現 lock は `79d1db0a`) の実 API で実装する。#112 の全4画面完成・#113 一般
Dock・#176 PluginDock 受入は開始条件にしない。

## 変更点

- `crates/synvelis-app/src/screens.rs` 新設: `main_screen_spec()`
  (`Panel > Tabs`、Transport tab の `Seek`→`transport/position`、
  Mixer tab の `Fader` を `unsupported` マウント)、
  `main_view_instance()`/`main_view_id()`(`ViewInstanceId(1)` →
  `"view-1"`)、`main_view_targets()`、`mount_main_view()`。
- `crates/synvelis-app/src/app.rs`: `SynvelisApp` に共有 cell の
  session-owned `AttachEpochAllocator`(`attach_epoch_allocator()`/
  `mint_attach_epoch()`)。app が `AppService`/`UiRuntime` へ封入
  された後も composition root 側から mint 可能。
- `crates/synvelis-app/src/shell.rs`: `NativeShell` が allocator
  handle を app から capture し `attach_view()` を追加 — window
  作成後に `syn.view.attach`+`syn.control.subscribe` を runtime
  dispatch 経路で発行し、`views`/`view_generation` を従来通りの
  result 観測で reconcile + outbox drain。
- `crates/synvelis-app/src/bin/synvelis-native.rs`:
  `mount_main_view` → `GpuiDriver::create_view_window` → `Ready` で
  `attach_view`。製品 theme は `synvelis-theme` カタログ
  (`synvelis/dark/comfortable`) を embed した DTCG 文書から解決し
  `ViewMount.theme` へ(RF385 解釈は upstream に委譲、解釈実装は
  持たない)。`synvelis-theme` を `native` feature の optional dep
  として追加。
- `crates/synvelis-app/tests/view_mount.rs` 新設: FakeDriver 上で
  mount→attach→subscribe→projection→gesture(Begin/Preview/Commit/
  Cancel)→receipt→save/reload→close→teardown→再 attach/host 再生
  の意味照合。`deliver_to_host` は `RvfView::drain_inbox` と同一の
  `Result`→`CommandReceipt`/`Notify`→`StateProjection` decode を
  再現し、Fake/GPUI 両 lane が同一 wire を踏むことを保証する。
- `Cargo.lock`: ruviform `27698cb1` → `68bb8710`(RF406) → `79d1db0a`(RF409/RF411 含む main、`test-msrv` 1.89 実測 exit 0)。
  `ui/vendor/ruviform` を `sync-ui-deps` で同 REV に再生成。

## 設計決定

- `control_id == target` を mount 側の規約とする:
  `notify_to_view_event` は `control_id = target` で投げ、
  `ViewHost::apply_projection` は `control_id` で相関するため、
  乖離させると projection が黙って落ちる。
- Mixer `Fader` は `unsupported: true`: 起動 project には
  `mixer.*` param target が存在しない(`syn.control.targets` は
  `transport`/`transport/position` のみ)。存在しない target へ
  command を mint させない正直な「未対応」表示。RF410 P2 の
  shared-`control_id` 未確定問題を避けるため全 control_id を
  一意にする。
- `attach_view` は subscribe 失敗を `warn` に留める(attach 成功は
  Err 伝播): view mount は attach が必須、subscription は target
  単位の degradable 要素。
- Epoch と logical id の分離: `ViewInstanceId`(永続 identity、
  `origin_view = "view-N"` へ写像)と `AttachEpoch`(runtime 保証、
  session allocator が mint)は別物。host 再生時は同一
  allocator から新 epoch を mint し、command-id namespace
  (`view-{id}:a{epoch}:m{mount}:{control}`)が dead namespace に
  隔離される。

## 完了条件

- `mise run fmt`/`check`/`lint`/`test` green、`check-app-native`/
  `lint-app-native` 実測。
- `tests/view_mount.rs` 全件 green(FakeDriver 意味照合)。
- 実 GPUI 起動 smoke(mount 描画・close)を実行可能範囲で実施し
  evidence 保存。OS render/input/IME/GPU の未観測項目は
  `NotRun`/`Blocked` +理由で記録、`Pass` にしない。

## 残存・境界

- RF410 P2:`ViewHost::command_index`/`CommandBinding::outbox` の
  retention は本 slice の fader 利用(有限 gesture)では実害なし、
  `push_event` 512 cap の drop、projection 相関が `control_id`
  のみ(reattach 後の最初の projection は旧 revision も一度は受け
  入れ自己修復)は既知制限として RF issue 境界で記録。恒久
  workaround は入れない。
- IME=`Unsupported`、`native_surface_slot`=backend 依存は
  capability 出力通り。実 OS/製品 E2E・親 Issue #175/#112 完全
  受入は未達。
- 一般 Dock(#113)・PluginDock(#176)・実外部 editor 受入は後段。

## 2026-11-08 再測定: main 追従・RF414 消費・Seek 単位の秒化

- main `8c11e31`（PR#180）を merge。競合は `docs/tasks/README.md` の索引行のみで、両側を保持した。
- ruviform `79d1db0a` → `20a24a66`（main、PR#415: RF414 projection→controlled display）。
  `Cargo.lock` の ruviform source 26 件が同一 SHA、`ui/vendor/ruviform/REV` と一致（`mise run check-ruviform-rev` pass）。
- 再測定で判明した事実: transport projection の `value` は `TransportProjection` の多キー object であり、
  RF414 の `projection_value_input` は単一スカラー（または単一キーの `plain`/`normalized`）しか controlled
  display に渡さない。そのため、object のままでは Seek の display は authoritative projection で更新されなかった。
  ruviform 側の object フィールド指定は追加せず、SV の binding adapter で単位付きスカラーに明示変換する方針を採った。

### 単位契約（Seek = 秒）

- 表示: `TickTimeMapping::beat_to_seconds`（`synvelis-project`）。ruviform `TempoMap::beat_to_media` で
  有理数の拍を秒に変換し、マイクロ秒（1e-6 s）に量子化する。量子化の丸めは ruviform の base 量子化規則に従う。
- 書き込み: `TickTimeMapping::seconds_to_beat`。入力秒は最近接マイクロ秒に丸めてから `media_to_beat` で拍に戻す。
  commit 経路は `synvelis-workspace` の `seek_point`（project の tempo map を使用）。
- 換算元は project の tempo map。既定 120 BPM では 1 拍 = 0.5 秒。BeatPoint の有理数は保持し、PPQN 量子化は行わない。
- 経路: `ui_ops` が行に `display_value`（秒）を付与 → `binding.rs` が view wire の `value` に使う（`observed` の
  生 object は他の読み手のため残す）。
- 範囲: Seek は boot 時の固定 0..240 秒。ruviform `20a24a66` の ViewHost に descriptor 差替 API がないため、
  project length への追従は未達（remount が必要。follow-up）。

### 未達・残課題

- Seek range は project length に追従しない（固定 0..240 秒）。
- Fader（`mixer.gain`）は projection 元が無く、controlled display の実測は NotRun。
- RF387 側の canvas 幅 0・SeekFader 未描画（実機、RF387 Partial/open）は本 slice では未観測。
- 実 GPUI 実窓、OS render/input/IME は未検証（Windows 以外は未実測）。

## 2026-11-08 slice2: pin 769d595a 統合・Seek range 追従・Save/Reload UI・実機再検証

upstream RF#450（canvas 修正）/RF#451（Tabs pointer・`ViewHost::update`
descriptor 更新）/RF#453（残 canvas 監査）/RF#456（job 終端 receipt）/
RF#448（Table 選択・Button）を含む ruviform main `769d595a` へ pin を更新し、
前回 `NotRun`/`Fail` だった受入項目を実装・再測定した。

### pin / lock

- `Cargo.lock` の ruviform source 26 件を `20a24a66` → `769d595a` へ統一
  （既定ブランチ追従の git 依存、SHA ハードコード無し）。`ui/vendor/ruviform`
  を `sync-ui-deps` で再生成し `REV` 一致、`mise run check-ruviform-rev` pass。

### Seek range の project length 追従

- `synvelis_workspace::viz_ui::project_length_seconds(session, media_rate)`
  を新設。`timeline_items` と同じ clip 解決経路で全 clip の末尾を秒換算し
  最大値を返す（空 project は `0.0`）。media_rate は timeline と同じ 48_000。
- `SynvelisApp` が共有 `Rc<Cell<f64>>` を公開し、`AppService::call` の
  各 op 後に publish。native composition root は `GpuiDriver` の clone
  handle を保持し、event loop で length 変化を検出したときだけ
  `GpuiDriver::update_view(window, main_screen_spec(length))` を呼ぶ。
  `ViewHost::update` が component key で reconcile するため remount 無し・
  host generation 維持・`Updated` op のみ。
- `seek_range_seconds(length)`: 正の有限 length → 実長、空/非有限 →
  従来の fallback `0..240` 秒。`update_view` 失敗は `tracing::error` に
  留めて描画を止めない。

### Save/Reload の実 UI 経路

- `main_screen_spec` の toolbar に `ComponentSpec::Button`
  （`project.save`/`project.reload`、`PROJECT_TARGET = "project"`）を追加。
- shell は `rvf.ui.command.v1` の `Action` intent で `project` target を
  `syn.project.save` / `syn.project.reload` へ route し、結果を view
  receipt として返す（Button の optimistic 状態が settle する）。
- `syn.project.reload` op を新設: session が保持する `bundle_path` を
  `project_open` で reopen（未保存編集を破棄）。mounted view と
  attach/subscribe 状態は維持される。

### Tabs pointer 切替

- `TabsSpec.selection = TabsSelectionSpec { target: "view" }` で upstream の
  pointer 選択契約を有効化。tab 押下は `tab-select` action
  （`params={"index":n}`、view-local state）を発行し、shell が
  `VIEW_STATE_TARGET` 宛を `result_ok` で ack して optimistic 選択を確定する。
- `RvfActionIntent` に `params: Option<Value>` を追加したが、
  `target == "view"` の場合のみ受理。domain action に params が載る
  場合は従来通り decode 拒否（strict contract 維持）。
- キーボード tab 切替は upstream FU-1 未実装のまま。Synvelis 側で
  独自実装しない。

### Fader `mixer.gain`

- `syn.control.targets` に `mixer.*` の投影元が存在しないことを再確認。
  `unsupported: true` を維持し、projection を捏造しない。実機では
  default_value 0.5 の fill で描画されるが入力不可（Unavailable）の
  まま — Unsupported 判定を継続。

### テスト

- `tests/view_mount.rs` 12 件: range 更新の in-place reconcile
  （`Updated` のみ・generation 不変・spec 差替）、default range、
  Save/Reload Button の `ViewHost::invoke` → service route、
  tab pointer 選択の shell ack、既存 gesture/projection/reload 非回帰。
- `crates/synvelis-workspace/tests/project_length.rs` 2 件:
  空 project → 0.0、120BPM/PPQN960 で 2880 tick 末尾 → 1.5 秒。
- `crates/synvelis-workspace/tests/fixture_bundle.rs`（`#[ignore]`）:
  実機検証用に `DirectoryBundleStore`+`ProjectController` の実保存経路で
  クリップ入り bundle を書き出す fixture。

### 実機再検証（2026-11-08, Windows 11 実窓）

`target/debug/synvelis-native.exe` を `SYNVELIS_PROJECT_DIR` 付きで起動し、
`CopyFromScreen` 実画面キャプチャ + 実マウス入力 + アプリログで再測定。
証拠: `F:/repositry/audio-products/.devin/evidence/sv175-slice2/verdict.md`。

| 項目 | 判定 |
| --- | --- |
| Seek 実窓描画（track/fill/thumb） | Passed（RF#450/453 で解消） |
| Seek ドラッグ → 表示追従 | Passed |
| Tabs pointer 切替（双方向） | Passed |
| Tabs キーボード切替 | Unsupported（upstream FU-1） |
| Save / Reload ボタン実 UI 操作 | Passed（`syn.project.save`/`open` ログ） |
| Seek range の project length 追従 | Passed（Reload 後 `seek range updated project_length_seconds=1.5` + thumb が新 range に追従） |
| Fader `mixer.gain` projection | Unsupported（投影元無し、`unsupported: true` 維持） |
| IME / 非 Windows OS | Unsupported / NotRun |
| close → shutdown | Passed |

計測上の注意: `PrintWindow(PW_RENDERFULLCONTENT)` は本 GPUI 窓で
古い/代替フレームを返す（48 色 vs 実画面 168 色）。前回証拠の
ピクセル判定はこの影響を受けていた可能性があり、今回は
`CopyFromScreen` を正とした。
