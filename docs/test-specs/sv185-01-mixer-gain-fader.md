---
id: "TSPEC-Xk9mQ2vR7wTzLp4nHs8dFj"
type: "test-spec"
status: "approved"
title: "SV185-01 検証仕様: mixer.gain projection・Fader controlled display/commit"
created: "2026-11-08"
updated: "2026-11-08"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/185"
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
    - "docs/specs/sv185-01-mixer-gain-domain-projection.md"
    - "docs/designs/sv185-mixer-gain-projection-design.md"
    - "docs/tasks/sv185-01-mixer-gain-fader.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/sv175-02-native-view-mount.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies:
    - "docs/specs/sv185-01-mixer-gain-domain-projection.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# SV185-01 検証仕様

headless は `ruviform_ui_test::FakeDriver` + 実 `ViewHost` +
`NativeShell`/`AppService` の既存 lane（`tests/view_mount.rs` と同じ
意味照合）。実機は Windows 実窓 + `CopyFromScreen` 実画面キャプチャ
+ 実マウス/キー入力 + アプリログ。語彙は
Passed/Failed/Blocked/NotRun/Unsupported。

## domain（`synvelis-project`）

| # | 観点 | 手順 | 期待 |
| --- | --- | --- | --- |
| D1 | bootstrap target | `Project::new` → `control.targets` | master strip owner・`ParameterRole::Fader`・`Continuous`/`Linear`/`ProjectMusical`・`base=1.0` の target が `MASTER_FADER_RAW` ID で 1 件存在 |
| D2 | well-known ID | `is_issuable_raw(MASTER_FADER_RAW)`、`max_issuable_id_raw` | false（発行域外）、watermark に影響しない |
| D3 | 除去禁止 | `RemoveParameterTarget` on bootstrap id | `Error::Validation`（well-known は op 対象外） |
| D4 | singleton | 同一 strip に `AddParameterTarget(role=Fader)` | `validate_control_binding` が拒否 |
| D5 | save/load | snapshot serialize→deserialize→`validate_references` | target が永続化され oracle を通る |
| D6 | send adopt 禁止 | `CreateSend`/`SetSendGainTarget` に `Adopt(MASTER_FADER_RAW)` | `Error::Validation`（well-known bootstrap target）— send teardown による wedge を防ぐ |
| D7 | v7→v8 migration | v7 envelope load | target 無し body に canonical target が補完される。master strip の issued `Fader` を持つ body ではその target が well-known ID へ re-key され、lane/assignment/send 参照と base が保持される |
| D8 | oracle 形状 | v8 body で fader target 欠落 / well-known ID に foreign role | `Error::Validation` で load 拒否 |

## headless app（`tests/view_mount.rs`・workspace・`tests/ui_ops.rs`）

| # | 観点 | 手順 | 期待 |
| --- | --- | --- | --- |
| M1 | subscribe | `main_view_targets()` に param tagged id を含む → `attach_view` | `syn.control.subscribe` が ok、初期 projection row が `target=param-…` で push される |
| M2 | controlled display | subscribe 直後の `observe("fader.gain")` | `observed` あり、`display` が `base=1.0`（normalized=plain=1.0）を反映 |
| M3 | gesture preview/commit | Begin → Preview(0.4) → Commit（既定 `AutomationMode::Read`） | preview 中は `display` が 0.4 を反映、commit で `Applied` receipt、release 後 display は authoritative base 1.0 に復帰（Read mode は書き込まない — domain 契約どおり） |
| M4 | cancel | Begin → Preview → Cancel | commit されず base 不変、receipt は Accepted、display は base に復帰 |
| M5 | 永続化 | save → open（reload）後に gesture | bootstrap target が snapshot に残り gesture が Applied |
| W1 | base edit 経路（workspace） | `AutomationMode::Off` で Begin → Preview → Commit | `SetParameterBase` が 1 undo で適用され `base` が更新、undo で 1.0 に復帰（`sv185_mixer_gain.rs`） |
| W2 | write なし経路 | gesture なし commit / stale generation | 何も書き込まれない（receipt/rejected が正直に返る） |
| U1 | targets listing | `syn.control.targets` | bootstrap fader の tagged id が `kind: "param"` で列挙される（`TARGET_CAP` は issued target のみに適用） |

## 実機（Windows 実窓）

| # | 観点 | 手順 | 期待 |
| --- | --- | --- | --- |
| R1 | Fader 描画 | Mixer tab を開く | Fader が track/fill/thumb で描画される（値=1.0 位置） |
| R2 | Fader ドラッグ | Fader を実マウスでドラッグ | fill/thumb が追従し、commit 後の表示値が projection と一致。アプリログ/画面差分で記録 |
| R3 | Tabs キーボード | Transport tab で Left/Right/Home/End | RF#451 の keyboard tab switching が動作（focus_next 未配線の FU-1b は範囲外） |
| R4 | OS coverage | macOS/Linux/IME/webview | 未検証は `NotRun`/`Unsupported` で記録 |

## 記録

- 実機証拠: `F:/repositry/audio-products/.devin/evidence/sv185-mixer-gain/`
  （verdict.md + キャプチャ + ログ + 計測スクリプト）。
- headless 結果は `mise run test` の実測で記録。cargo 直接実行は
  補助証拠として明記。
