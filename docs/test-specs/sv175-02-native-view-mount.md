---
id: "TSPEC-zjbHKZf81iknbLJPzohrKC"
type: "test-spec"
status: "approved"
title: "SV175-02 検証仕様: ViewHost mount → typed binding → lifecycle"
created: "2026-11-07"
updated: "2026-11-08"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/175"
    - "https://github.com/scarecrow173/synvelis/issues/112"
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
    - "docs/specs/sv-gpui-01-native-app-shell.md"
    - "docs/designs/sv175-native-shell-design.md"
    - "docs/tasks/sv175-02-native-view-mount.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/tasks/sv175-02-native-view-mount.md"
    - "docs/test-specs/sv175-01-native-shell-binding.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies:
    - "docs/specs/sv-gpui-01-native-app-shell.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# SV175-02 検証仕様

意味照合は `ruviform-ui-test::FakeDriver` + 実 `ViewHost` で
headless に行い、実 OS window/GPU/IME/focus は別 gate(実機 smoke、
未観測は `NotRun` 明記)。Fake lane の `deliver_to_host` は GPUI
lane `RvfView::drain_inbox` と同一 decode を再現し、同一 spec/
theme/fixture の両 lane が同じ wire を踏むことを検証する。

## screens(`src/screens.rs`)

| # | 観点 | 手順 | 期待 |
| --- | --- | --- | --- |
| P1 | spec 妥当性 | `main_screen_spec()` → `ViewHost::mount` | mount 成功、Tabs≥1 / Seek / Fader を含む |
| P2 | view id 一貫 | `main_view_id()` と `ViewInstanceId` | `"view-1"` と `origin_view` 一致 |

## view mount(`tests/view_mount.rs`)

| # | 観点 | 手順 | 期待 |
| --- | --- | --- | --- |
| M1 | attach+subscribe | `attach_view` | `syn.view.attach` ok(generation≥1)、`views[view-1]==window`、subscribe result ok |
| M2 | read-only projection | subscribe 後の outbox drain | `syn.control.projection` row が `transport/position` へ、翻訳 `Notify{control_id,revision,value}` が host binding `last_observed` に着地(`observe.display` への反映は B1 参照、未達) |
| M3 | gesture lifecycle | `dispatch_input` Begin→Preview→Commit | `rvf.ui.command.v1` → `syn.control.dispatch` → `syn.control.receipt`(applied)が翻訳 `Result` として host `last_receipt` に着地、生 notify も broadcast |
| M4 | cancel | Begin→Preview→Cancel | cancel receipt が report される |
| M5 | unsupported fader | `fader.gain` へ Begin | `Unavailable` event、Intent 不発、domain receipt 無し |
| M6 | 再 attach / host 再生 | 再 `attach_view` + `mount_main_view`(post-seal handle) | generation bump、新 host の command namespace が dead host と disjoint |
| M7 | save/reload | gesture→`project.new`→`save`→`open` | view attach/subscription が存続し post-reload gesture も applied |
| M8 | window 破棄 teardown | view window `Destroyed`(他 window 残存) | `syn.view.close` replay で registry/`views` 解放、subscription/teardown 完了 |
| M9 | seek range 更新 | mount(空 project)→`update_view` に長さ違い spec | 全 op が `Updated`（`Mount`/`Unmount` 無し）、`seek.position` の descriptor range が新長へ、host generation 不変 |
| M10 | project Button | `invoke(project.save)`/`invoke(project.reload)` → `rvf.ui.command.v1` wire | `syn.project.save`/`syn.project.open`(reload) が ok で dispatch され view が存続 |
| M11 | tab pointer 選択 | `invoke(screen.tabs)` の tab header press → `tab-select` action | shell が `view` target を `result_ok` ack、displayed tab が選択 index へ settle |

## project length(`synvelis-workspace`)

| # | 観点 | 手順 | 期待 |
| --- | --- | --- | --- |
| L1 | 空 project | `project_length_seconds` | `0.0`（seek fallback 240 s へ写像） |
| L2 | clip 末尾 | 120BPM/PPQN960 で末尾 2880 tick の clip 2 本 | `1.5` 秒（tick→秒は tempo map 経由、PPQN 量子化無し） |

## 既存非回帰

- `tests/shell.rs` / `tests/binding.rs` / `tests/service_ops.rs` 全件
  green(従来の attach/command/ownership/project 契約を維持)。
- `check-headless-deps` green — `gpui`/`ruviform-ui-gpui`/
  `synvelis-theme` は headless 依存木へ流入しない。
- `check-ruviform-rev` — lock/vendor/manifest が `79d1db0a` に一致。

## 実機 gate(別結果、未観測は正直に記録)

2026-11-07 実機 GPUI smoke 実施(`target/debug/synvelis-native.exe`,
Windows 11 実 window, jsonl log + `PrintWindow` ピクセル取得を
`target/gpui-smoke/` に保存)。観測範囲の結果:

| # | 観点 | 判定 | 証拠 |
| --- | --- | --- | --- |
| G1 | `synvelis-native` 起動→view 描画 | **Partial** | `main view attached`(view-1) / 窓生成 / tabs 描画は実機確認。ただし Seek コントロールは描画されない(下記 RF387 境界の障害) |
| G2 | Tab 切替 / Seek ドラッグ→receipt | **Partial** | ドラッグ(`PostMessage` WM_LBUTTONDOWN/MOUSEMOVE/UP)→ `syn.control.dispatch` ×14(Begin+12 Preview+Commit)・失敗 0 — 実入力→gesture→ProjectController 完走。Tab 切替は上流未実装(tab header に click handler 無し、`TabsSpec.active` は spec 値)。committed 値は下記障害で縮退 |
| G3 | 通常 close → teardown+exit | **Pass** | `CloseMainWindow`(WM_CLOSE)×2 回 → `syn.view.close` replay → `synvelis-native shutdown`、exit 0 |
| G4 | IME / native_surface_slot | 観測値 | 起動 log: `ime=unsupported`(#389 想定通り)、`native_surface_slot=supported`。IME 入力検証は NotRun |
| G5 | Save/Reload 実機観測 | **NotRun** | 対話 UI 未配線(実機での project.new/save 操作経路無し)。意味検証は M7 FakeDriver で済 |

### RF387 境界で観測された上流障害(恒久 workaround なしで記録)

実機 smoke で RF387 `ruviform-ui-gpui` renderer の描画欠陥を発見。
`control_element`(`render.rs` ~L700)・`waveform_element` が未スタイル
`canvas`(`Style::default()`: size auto・measure 無し)を row-`flex` div の
child として置くため、canvas が main-axis 幅 0 に縮退し、
track/fill/thumb quad が全て幅 0 で paint される。

実害:

- `Seek`/`Fader`/`Waveform` が実 window で一切描画されない
  (`PrintWindow` 全領域走査: `control_track` 0x262d36 ピクセル 0 件、
  tab body は一様 `surface_bg` 0x14181d)。
- canvas `bounds`(pointer→`normalized_for` マッピング基盤)が幅 0
  → `normalized_at` の `extent<=0` guard により pointer 位置に
  依らず `n=0` — gesture lifecycle は完走するが commit 値が
  pointer 位置を反映しない。

Synvelis 側の恒久 workaround は不可(spec 形状では回避不能な
上流 element builder の欠陥であり、RF387 境界で報告する)。
意味照合は FakeDriver lane(M1–M8)で完備しており、実機 lane の
本件は RF387/#313 real-GUI gate の残課題として root へ報告する。

## 独立レビュー結果と受入判定 (root 実施)

worker 側 Reviewer A/B はモデルのレート制限で 2 回とも **Failed/NotRun**。
別証拠として root が独立 A/B → 別 Validator → Judge を実施
(`F:/repositry/audio-products/.devin/evidence/gui-v2-replan-20261107/sv175-root-review/`)。

| ID | 判定 | 内容 |
| --- | --- | --- |
| B1 | CONFIRMED / P1 | RF79d の `view.apply_projection` は `binding.last_observed` のみ更新し `observe.display`(dispatcher value)へ反映しない。GPUI 表示が外部 state に追従しない。責務は RF 境界。追跡: https://github.com/scarecrow173/ruviform/issues/414 。実 probe は NotRun(静的 review) |
| B2 | CONFIRMED / P2 | Cancel テストは一般 receipt 非空のみ確認(試験制限) |
| B3 | CONFIRMED / P2 | 新 reattach テストは ID 集合差のみ。generation / stale 遮断は未確認(試験制限) |

最終判定: **Native 受入は BLOCKED (RF414)**。Synvelis 側の恒久 workaround・RF 編集・新機能は行わない。
GPUI 実機は Partial(canvas 未描画 / Tab 操作未実装 / commit 値縮退)、実 UI Save/Reload は NotRun、
親 #175 / #112 は未達。

### 再開手順
1. Ruviform #414(projection → display 反映)と RF387 canvas 幅欠陥の修正を待つ。
2. lock/REV を修正 revision へ同期し `check-ruviform-rev` を通す。
3. 実 GPUI で非ゼロ projection 表示と Tab/Fader 操作を再測定し、B2/B3 の試験を強化する。

## 2026-11-08 slice2 実機再測定（pin `769d595a`）

前回 Fail/Blocked/NotRun だった項目を ruviform `769d595a`
（RF#450/451/453/456/448 含む）で再測定。`CopyFromScreen` 実画面
キャプチャ + 実マウス入力 + アプリログで判定
（`PrintWindow` は本 GPUI 窓で stale frame を返すため不採用）。
証拠: `F:/repositry/audio-products/.devin/evidence/sv175-slice2/verdict.md`。

| # | 観点 | 判定 | 証拠 |
| --- | --- | --- | --- |
| G1 | Seek 実窓描画 | **Passed** | track `#262d36`/fill `#3d7be0`/thumb `#e8edf2` を画素確認（`seek-drag-*`） |
| G2 | Seek ドラッグ操作 | **Passed** | ドラッグで thumb 0→69%→0 と fill 追従（各 ~2-3 万 px 差分） |
| G3 | Tabs pointer 切替 | **Passed** | Mixer↔Transport 双方向で ~4.2 万 px 差分の表示切替 |
| G4 | Tabs キーボード切替 | **Unsupported** | upstream FU-1 未実装（Synvelis 側で実装しない） |
| G5 | Save/Reload 実 UI 操作 | **Passed** | 実クリックで `syn.project.save`/`syn.project.open`(reload) completed ログ |
| G6 | Seek range の project length 追従 | **Passed** | クリップ入り bundle Reload で `seek range updated project_length_seconds=1.5` + thumb が新 range に追従（`ViewHost::update` 非再マウント） |
| G7 | Fader `mixer.gain` projection | **Unsupported** | 投影元無し。`unsupported: true` で描画のみ・入力不可のまま（捏造しない） |
| G8 | IME / 非 Windows / close | Unsupported / NotRun / Passed | `ime=unsupported`、Windows のみ実測、`CloseMainWindow`→`synvelis-native shutdown` |
