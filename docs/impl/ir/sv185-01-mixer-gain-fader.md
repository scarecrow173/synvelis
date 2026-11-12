---
id: "IMPL-SV18501MixerGainFader"
type: "impl"
status: "completed"
title: "SV185-01 mixer.gain domain projection — master strip Fader bootstrap・v7→v8 migration・Fader 接続 実装記録"
created: "2026-11-08"
updated: "2026-11-08"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/185"
    - "https://github.com/scarecrow173/synvelis/issues/175"
    - "https://github.com/scarecrow173/ruviform/issues/414"
    - "https://github.com/scarecrow173/ruviform/issues/387"
    - "https://github.com/scarecrow173/ruviform/issues/451"
    - "https://github.com/scarecrow173/ruviform/issues/461"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-workspace/tests/sv185_mixer_gain.rs"
      - type: "file"
        path: "docs/impl/ir/sv185-01-mixer-gain-fader.md"
      - type: "file"
        path: "docs/specs/sv185-01-mixer-gain-domain-projection.md"
      - type: "file"
        path: "docs/designs/sv185-mixer-gain-projection-design.md"
      - type: "file"
        path: "docs/plans/sv185-mixer-gain-projection.md"
      - type: "file"
        path: "docs/tasks/sv185-01-mixer-gain-fader.md"
      - type: "file"
        path: "docs/test-specs/sv185-01-mixer-gain-fader.md"
    modified:
      - type: "file"
        path: "Cargo.lock"
      - type: "file"
        path: "crates/synvelis-app/src/bin/synvelis.rs"
      - type: "file"
        path: "crates/synvelis-app/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-app/src/screens.rs"
      - type: "file"
        path: "crates/synvelis-app/src/shell.rs"
      - type: "file"
        path: "crates/synvelis-app/src/ui_ops.rs"
      - type: "file"
        path: "crates/synvelis-app/tests/ui_ops.rs"
      - type: "file"
        path: "crates/synvelis-app/tests/view_mount.rs"
      - type: "file"
        path: "crates/synvelis-project/src/control.rs"
      - type: "file"
        path: "crates/synvelis-project/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-project/src/migration.rs"
      - type: "file"
        path: "crates/synvelis-project/src/ops.rs"
      - type: "file"
        path: "crates/synvelis-project/src/project.rs"
      - type: "file"
        path: "crates/synvelis-project/src/store.rs"
      - type: "file"
        path: "crates/synvelis-project/tests/aut_02.rs"
      - type: "file"
        path: "crates/synvelis-project/tests/ctl_01.rs"
      - type: "file"
        path: "crates/synvelis-project/tests/md_a.rs"
      - type: "file"
        path: "crates/synvelis-project/tests/midi_source.rs"
      - type: "file"
        path: "crates/synvelis-project/tests/routing_sf01.rs"
      - type: "file"
        path: "crates/synvelis-project/tests/sf_02.rs"
      - type: "file"
        path: "crates/synvelis-workspace/tests/aut_01.rs"
      - type: "file"
        path: "crates/synvelis-workspace/tests/aut_03.rs"
      - type: "file"
        path: "crates/synvelis-workspace/tests/aut_04.rs"
      - type: "file"
        path: "crates/synvelis-workspace/tests/aut_04_rt.rs"
      - type: "file"
        path: "crates/synvelis-workspace/tests/pc_a.rs"
      - type: "file"
        path: "crates/synvelis-workspace/tests/pc_b.rs"
      - type: "file"
        path: "crates/synvelis-workspace/tests/pc_c1.rs"
      - type: "file"
        path: "crates/synvelis-workspace/tests/sv47_controls.rs"
      - type: "file"
        path: "docs/designs/README.md"
      - type: "file"
        path: "docs/impl/ir/README.md"
      - type: "file"
        path: "docs/plans/README.md"
      - type: "file"
        path: "docs/specs/README.md"
      - type: "file"
        path: "docs/tasks/README.md"
      - type: "file"
        path: "docs/test-specs/README.md"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/tasks/sv185-01-mixer-gain-fader.md"
  implemented-by: []
  depends-on:
    - "docs/specs/sv185-01-mixer-gain-domain-projection.md"
    - "docs/designs/sv185-mixer-gain-projection-design.md"
    - "docs/plans/sv185-mixer-gain-projection.md"
    - "docs/tasks/sv175-02-native-view-mount.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/sv185-01-mixer-gain-fader.md"
    - "docs/impl/ir/sv175-01-native-shell-binding.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/sv185-01-mixer-gain-fader.md"
  references:
    - "F:/repositry/audio-products/.devin/evidence/sv185-mixer-gain/verdict.md"
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# SV185-01 実装記録: mixer.gain domain projection — Fader 接続

## Summary

Issue #185。SV175-02 で `unsupported: true` のまま残った Mixer
`Fader`（論理 target `mixer.gain`）に domain projection 元を整備し、
driver-neutral ViewSpec 経由で controlled display と
`Begin → Preview → Commit/Cancel` の commit 経路を接続した。
`synvelis-project` に master strip の well-known `Fader`
`ParameterTarget`（`MASTER_FADER_RAW`）の bootstrap・load oracle・
schema v7→v8 migration を追加し、`synvelis-app` の Mixer Fader を
tagged `param-<32hex>` target の supported kind に置き換えた。
ruviform pin を `769d595a` → `863a7a9d` へ更新し、webview entry を
`NativeShell` 構成へ移行した。headless lane と Windows 実窓で
検証済み。

## Context

- Spec: `docs/specs/sv185-01-mixer-gain-domain-projection.md`
- Design: `docs/designs/sv185-mixer-gain-projection-design.md`
- Plan: `docs/plans/sv185-mixer-gain-projection.md`
- Task: `docs/tasks/sv185-01-mixer-gain-fader.md`（implements）
- Test-spec: `docs/test-specs/sv185-01-mixer-gain-fader.md`
  （verified-by）
- depends-on: 上記 spec/design/plan +
  `docs/tasks/sv175-02-native-view-mount.md`
- related: `docs/impl/ir/sv175-01-native-shell-binding.md`
- Source: synvelis Issue #185・#175、ruviform Issue #414・#387・
  #451・#461。
- ブランチは既存 `scarecrow173/sv175-native-view-mount`（Draft PR
  #183）で継続。

## Implementation

### domain（`synvelis-project`）

- `control.rs`: `MASTER_FADER_RAW = WELL_KNOWN_BASE + 6`（routing
  bootstrap の `+1..=+5` に続く予約連番）。共有 constructor
  `master_fader_target(master_strip)` が canonical target を生成 —
  `ControlOwner::Strip(master)`・`ParameterRole::Fader`・
  `Continuous`/`Linear`/`ProjectMusical`・`capability: None`・
  `base: 1.0`。`ControlModel::bootstrap(master_strip)` が fresh
  project の最小構成としてこれを seed する。
- `project.rs`: `Project::new` が routing 構築後に master bus の
  strip を引き `ControlModel::bootstrap` を適用。`validate_references`
  が well-known fader の存在に加え `role == Fader`・
  `owner == Strip(master)` を oracle として要求（ID に擬態した
  foreign entry も拒否。name/kind/mapping/base は re-key された
  ユーザー fader を通すため拘束しない）。
- `ops.rs`: `RemoveParameterTarget` が `!is_issuable_raw` を拒否
  （既存 guard）。review で発見した send gain adoption 経路 —
  `AddSend`/`SetSendGainTarget` は apply で target の owner/role を
  `Send`/`SendLevel` に書き換える — にも同じ guard を追加。
  well-known fader を adopt すると send teardown
  （`remove_send_owned`）が target を削除し oracle が project を
  reject する wedge を防ぐ。
- `migration.rs`/`store.rs`: `SCHEMA_VERSION` 7→8。v7→v8 step
  `migrate_v7_bootstrap_fader` は canonical target を補完する。
  v7 body が既に master strip の *issued* `Fader` target を持つ
  場合（v7 では singleton rule 上合法）はそれを well-known ID へ
  **re-key** — `targets` key/`id` と `lanes[].target`・
  `assignments[].target`/`depth_source`・`sends[].gain_target` の
  参照を張り替え、base・kind・name を保持する。2 個目の `Fader`
  挿入は singleton rule で load 不能になるため。

### app（`synvelis-app`）

- `screens.rs`: Mixer Fader の `target`/`control_id` を
  `mixer_gain_target()`（tagged `param-<32hex>`）に、
  `unsupported: false`、normalized range `[0,1]`、`default 1.0`、
  `step 0.05`、vertical・`DetachPolicy::Cancel`・accessible label
  "Gain"。`main_view_targets()` が transport position + mixer gain
  の 2 target を返す。
- `ui_ops.rs`: param projection row に `display_value =
  {"normalized": host_normalized}` を付与（RF414:
  `projection_value_input` は多キー object を decode しないため
  adapter で normalized 単一キー形へ明示変換）。`syn.control.targets`
  の `TARGET_CAP`(256) は issued target のみに適用 — well-known ID
  は id 順で常に末尾に並び、cap が構造 target を先に落とすため。
- `shell.rs`/`bin/synvelis.rs`: ruviform pin `863a7a9d` で
  `ruviform-ui-webview` の `HostConfig`/`run`/`HostEvent` API が
  driver-neutral `run_multi`/`SessionConfig`/`WindowDriver`/
  `DriverEvent` に置き換わったため、webview entry を `NativeShell`
  構成へ移行。`NativeShell::new_with_data` で `DataRegistry` を
  `rvf://data` scheme handler と共有（clone は同一 store）。

### ruviform pin

- `769d595a` → `863a7a9d`（RF#451 Tabs keyboard、RF#453 canvas
  size、RF#461 paint_layer O(n²) 解消を含む main 最新）。
  `Cargo.lock` 26 sources 全て `863a7a9d`、`ui/vendor` 同期、
  `check-ruviform-rev` で lock/manifest/vendor 一致を確認。

## Related Experiments

実験ログなし — `metadata.experiments` の adopted/rejected は空で、
関連する `docs/impl/exp/` の実験ファイルもない。

## Validation

| lane | 結果 |
| --- | --- |
| `mise run fmt` / `fmt-check` | pass |
| `mise run check` | pass |
| `mise run lint` | pass, warning 0 |
| `mise run test` | pass, 失敗 0 |
| `mise run build` | pass |
| `mise run check-headless-deps` | pass |
| `mise run check-ruviform-rev` | pass, 26 sources `863a7a9d` |
| `mise run check-app-native` / `lint-app-native` / `build-app-native` | pass |
| `mise run check-app-webview` / `lint-app-webview` / `test-app-webview` | pass |
| `cargo test -p synvelis-project --test ctl_01` | 21 pass（SV185 section 含む） |
| `cargo test -p synvelis-project --test sf_02` | 16 pass（send adopt guard 含む） |
| `cargo test -p synvelis-workspace --test sv185_mixer_gain` | 3 pass |
| `cargo test -p synvelis-app --test view_mount` | 13 pass |
| `cargo test -p synvelis-app --test ui_ops` | pass（targets listing 含む） |

※ cargo 直接実行は補助証拠。canonical は `mise run <task>`。

### 実機検証（Windows 実窓）

`target/debug/synvelis-native.exe` を `SYNVELIS_PROJECT_DIR` 付きで
起動し、`CopyFromScreen` 実画面キャプチャ（`PrintWindow` は GPUI で
旧 frame を返し得るため不使用）+ 実マウス/キー入力で確認。
証拠: `F:/repositry/audio-products/.devin/evidence/sv185-mixer-gain/`
（`verdict.md` + キャプチャ + ログ + 計測スクリプト）。

- Fader 描画: Mixer tab で track `#262D36`・fill `#3D7BE0`・
  thumb `#E8EDF2` を pixel scan で確認 — **Passed**
- Fader ドラッグ: 実マウスドラッグで fill/thumb が追従
  （画面差分・geometry で記録）— **Passed**
- controlled display: projection の `host_normalized` が display に
  反映されることを headless（view_mount）で検証、実窓では描画・
  ドラッグ応答を確認 — **Passed**
- Tabs キーボード Left/Right/Home/End: RF#451 の keyboard tab
  switching が動作（tab/seek の画面差分で記録）。FU-1b の
  `focus_next` Tab 循環は既知残課題で範囲外 — **Passed**
- graceful shutdown: `synvelis-native shutdown` で正常終了 —
  **Passed**
- macOS/Linux/IME/webview 実機: **NotRun**（Windows native のみ検証）
- commit の base 永続化: 既定 `AutomationMode::Read` では gesture
  commit は preview を authoritative base に戻す domain 契約
  （全 parameter target 共通・automation mode UI は別 slice）。
  書き込み経路は workspace test で `AutomationMode::Off` の
  `BaseEdit`/`SetParameterBase` として検証済み — **Passed**
  （headless）/ 実窓での Off-mode 操作は **NotRun**

## 独立 review と対応

review-protocol に従い cooperative + adversarial の独立 reviewer
2 系統で作業ツリーを検査。確定した指摘と対応:

- **[P1→fix] send gain adoption が bootstrap fader を破壊し得る**:
  `AddSend`/`SetSendGainTarget` の validate が `is_issuable_raw` を
  見ておらず、apply が owner/role を `Send`/`SendLevel` に
  書き換え → send teardown が target を削除 → oracle reject で
  send 除去不能の wedge。両 validate に guard 追加 +
  `sf_02.rs` test。
- **[P2→fix] v7→v8 migration が既存 master-strip `Fader` で
  unloadable に**: v7 で合法だった issued `Fader` target がある
  project は 2 個目の `Fader` で singleton 違反。migration を
  re-key 方式に変更（参照を全て張り替え、ユーザー値を保持）+
  `ctl_01.rs` test。
- **[P2→fix] load oracle が key 存在のみ検査**: well-known ID の
  entry が foreign role/owner でも通過し得た。`role == Fader`・
  `owner == Strip(master)` を要求 + `ctl_01.rs` test。
- **[P3→fix] `syn.control.targets` の cap が構造 target を落とす**:
  well-known ID は id 順末尾のため `TARGET_CAP` で先に脱落。
  cap を issued target のみに限定 + `ui_ops.rs` test。
- **[P2→fix] doc chain が実装と矛盾**: spec/design/plan が
  migration を Non-Goal/却下としていたが実装は v8 migration +
  oracle を含む。全 doc を実装内容に整合（D7 追加・test-spec に
  D6–D8/W1–W2/U1 行追加）+ `screens.rs` の stale comment 修正。
- **[報告のみ] 既定 `AutomationMode::Read` で commit は base を
  書き込まない**: 全 parameter target 共通の domain 契約であり
  本 slice 固有の欠陥ではない。mode 切替 UI は別 slice。impl
  record と PR コメントに既知制約として記録。

## Risks

- `AutomationMode` を `Off` にする UI/wire 経路は存在しない —
  gesture commit は preview→base 復帰（Read）で、base 永続化は
  host-only `set_mode` のみ。automation mode surface は後続
  issue であり、それまで Fader commit は project に永続化されない
  （全 parameter target 共通の domain 契約で本 slice 固有の
  欠陥ではないが、利用者が永続化を誤認する余地がある）。
- FU-1b Tab 循環 `focus_next` 未配線（ruviform 既知残課題）—
  Tabs の Arrow/Home/End は RF#451 で動作確認済みだが Tab キー
  巡回は未対応。
- webview lane は compile/test 検証済みだが実 WebView2 実窓での
  対話検証は未実施（**NotRun**）。macOS/Linux/IME 実機も
  **NotRun**（Windows native のみ検証）。

## Follow-ups

- automation mode surface（`AutomationMode::Off` への UI/wire
  経路）— 後続 issue。
- FU-1b Tab 循環 `focus_next` — ruviform 側の既知残課題の解消。
- webview lane の実 WebView2 実窓での対話検証 — **NotRun**。
- macOS/Linux/IME 実機検証 — **NotRun**。
- 実窓での `AutomationMode::Off` gesture 操作確認 — **NotRun**
  （headless では `BaseEdit`/`SetParameterBase` として検証済み）。
