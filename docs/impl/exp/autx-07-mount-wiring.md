---
id: "EXP-AUTX07MountWiring"
title: "AUTX-07 composition-mount wiring 実装メモ (synvelis#266 SV266-NR-05)"
created: "2026-11-17"
updated: "2026-11-17"
---

# AUTX-07 mount wiring — experiment log

## E1. catalog seam を workspace 側に置く判断

`synvelis-app` は `synvelis-gateway` に依存できない（AC-05、
`check-headless-deps` が検査）ため、authority-pushed catalog を
どう app へ渡すかが最初の設計点だった。選択肢は (a) app 側に
独自の中間型を定義して workspace で変換、(b) workspace に
serde 可能な中立 `AutomationCatalog` を置き JSON 境界で注入、の
2 つ。(b) を採用 — projection/request lowering が消費する型
（`CapabilityDescriptor`・`MacroPresetView`・`GrantState` 等）は
既に serde を持つか小さな derive 追加で済み、app は
`AutomationCatalog::from_json` で parse して `action_catalog`/
`macro_browser`/`lower` の結果だけを JSON/値で受け取る。
「第二の typed push path」を作らないため、push 文書と parse 対象を
一致させた（`deny_unknown_fields` で余計な key も拒否）。

`WireRequest` は gateway 側で deserialize-only（inbound 契約）の
ため serialize を持たない。`lower()` の戻り値を app へ渡すには
wire 形 JSON が必要で、`wire_request_json`/`wire_target_json`/
`wire_consistency_json` を seam 内に手書きした — gateway の serde
shape をミラーするが、これも workspace→app の出口であり app が
gateway 型を名指さない制約を守る。

## E2. publish 検出に `PartialEq` ではなく rev stamp

`AutomationScreenState` は `ActionCatalogView`/`MacroBrowserView`
等を内包し、それらは gateway 契約型を埋め込むため `PartialEq` を
導出していない（既存型を変えない判断）。VCS の screen cell は
値比較で更新検出していたが、automation では `publish_automation_
screens` の実行ごとに `autx_publish_rev` を increment し、
`AutomationScreenPublish{state, rev}` を cell に格納。native
run loop は `BTreeMap<ViewInstanceId,u64>` の stamp 比較で
「publish が走ったか」だけを見る — projection 内容の deep
compare を避けつつ、state が変わらない op での無駄な spec push
も抑えられる（rev は publish pass 毎に進むので厳密には
「内容が同じでも push は走る」が、spec 再構成は idempotent で
害がない）。

## E3. `syn.autx.action` の op 分類

shell は `is_autx_target` で 5 target を `syn.autx.action` へ
route するが、op 内でも `decode_automation_command` が
`(target, action, params)` → `AutomationCommand::{ViewLocal,
Request, CancelSelectedRun}` に分類する。`ViewLocal` は
`ViewState.automation` のみ mutate、`Request` は catalog 必須、
`CancelSelectedRun` は monitor projection から operation id を
解決してから `JobCancel` lowering — 「request ≠ confirmed」の
契約を保つため cancel も lowering 結果を返すだけで confirmed
cancelled とは表示しない。

catalog 未 push の request action は `codes::UNAVAILABLE` で
typed reject（fake success・no-op を返さない）。`RunSelect` は
monitor projection が無い現状では run id を解決できないため
常に `unavailable` — dangling index を `run_selected` に保存
しない fail-closed を選んだ。`PaletteCategory` 選択時に
`palette_selected` を `None` に戻すのは、domain filter で row
index が再 base されるため（stale index が別 capability を指す
事故を防ぐ）。

consistency の選択は app 層の責務として `lower_autx_requests`
に集約: preview 系（`Preview`/`MacroPreview`）は `latest`、
mutation 系は `self.workspace.session().revision().0` の
`expected-revision` — stale confirm が bind 時に落ちる契約。

## E4. mounted fixture の形

`tests/common/mounted.rs` は main view 専用（`main_screen_cell`・
`main_view_targets` 前提）なので、automation 4 surface は
`sv266_wiring.rs` 内に `AutxMounted` fixture として実装した:
app で `syn.autx.visual` を 4 回 open → `automation_screen_cell`
取得（app seal 前）→ surface 毎に `ViewHost` を
`mount_automation_view` で mount → `NativeShell` + `FakeDriver`
で surface 毎に `Ready` window を立て `attach_view(targets=&[])`
（automation view は control subscription を持たない）。
intent mint/deliver は `MountedFixture` と同じ
`rvf.ui.command.v1` envelope 経路を踏む。

## E5. つまずき

- `WireRequest` serialize で `request.capability` 等を値 move
  すると借用エラー — `&request.*` 参照渡しに修正。
- native run loop の automation 更新検出で publish 型に
  `PartialEq` を要求しかけた — projection 型は derive していない
  ため rev stamp 方式に変更（E2）。
- `PendingMount::spec` が publish entry 消失時に spec を返せず
  `Option<ViewSpec>` 化 — 「view が open と update の間に消えた」
  場合に捏造 spec を push しない fail-closed。
- `syn.autx.visual{close}` は `view-1`（main view）を拒否 —
  `AUTX_VIEW_KINDS` による kind check で composition-root view
  への誤爆を防ぐ。`view_id_watermark().max(2)` で予約 id 帯を
  避ける mint 規則は VCS open と同型。
- seam test で `Availability` の variant 名を `unavailable` と
  誤記 — 実際は `planned`/`disabled{reason}`/`retired`。
  fixture の serde shape を実機 serialize で確認してから
  literal JSON を書く手順が有効だった。
- app test の catalog JSON は `synvelis-gateway` fixture を
  serialize した実 shape を縮小して literal 化 — app が gateway
  に依存しない制約の中で wire 形の正しさを担保。
