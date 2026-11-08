---
id: "SPEC-Xk9mQ2vR7wTzLp4nHs8dFj"
type: "spec"
status: "approved"
title: "SV185-01 mixer.gain domain projection — master strip Fader target の bootstrap と Fader controlled display/commit 接続"
created: "2026-11-08"
updated: "2026-11-08"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/185"
    - "https://github.com/scarecrow173/synvelis/issues/175"
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
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/tasks/sv175-02-native-view-mount.md"
    - "docs/test-specs/sv175-02-native-view-mount.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "https://github.com/scarecrow173/ruviform/issues/414"
    - "https://github.com/scarecrow173/ruviform/issues/387"
  defers: []
  deferred-by: []
---

# SV185-01 mixer.gain domain projection 仕様

## What Should Be Built

Issue #185。SV175-02 で `unsupported: true` のまま残った Mixer `Fader`
（論理 target `mixer.gain`）に、domain/service 層の実 projection 元を
整備し、driver-neutral ViewSpec 経由で controlled display と
`Begin → Preview → Commit/Cancel` の commit 経路を接続する。

1. **domain projection 元の新設**: `synvelis-project` の
   `ControlModel` bootstrap に、master strip（`RoutingModel::bootstrap`
   が必ず持つ master bus の strip）を owner とする
   `ParameterRole::Fader` / `ValueKind::Continuous` /
   `ParamMapping::Linear` / `AutomationTimeScope::ProjectMusical` の
   `ParameterTarget` を 1 件持たせる。ID は routing bootstrap entity
   と同じ予約 well-known range（`WELL_KNOWN_BASE` 以降）の定数とし、
   `IdGenerator` の発行域と衝突しない。`Project::new` が routing
   bootstrap と同じ「合法 project が必ず持つ最小構成」として生成し、
   save/load snapshot に永続化される。schema version を v8 に上げ、
   v7→v8 migration が旧 snapshot に canonical target を補う
   （v7 で合法だった master strip の issued `Fader` target は
   well-known ID へ re-key し、base・lane・assignment・send 参照を
   保持する）。
2. **wire target**: Fader の binding `target`/`control_id` は当該
   target の tagged id（`param-<32hex>`）。`mixer.gain` という文字列
   alias は作らず、domain 概念としての「master strip の fader gain」
   に対応する同等の projection 元とする（Issue の「または同等の
   domain 概念」に該当）。
3. **controlled display**: `syn.control.projection` の param row に、
   view wire が decode できる `display_value`（`{"normalized": …}`、
   `host_normalized` が正本）を付与する。RF414 の
   `projection_value_input` は多キー object を decode しないため、
   adapter 層で normalized 単一キー形に明示変換する（Seek の秒
   `display_value` と同じ責務境界）。
4. **commit 経路**: 既存の `ControlsUi::dispatch` →
   `gesture_begin`/`gesture_update`/`control_gesture_end` →
   `ProjectOp::SetParameterBase`（`Off` mode の `BaseEdit`、1 undo）
   をそのまま使う。gesture/receipt/projection の区別、generation
   guard、stale receipt 吸収は既存契約のまま変更しない。
5. **除去・乗っ取りの禁止**: bootstrap の well-known target は
   `RemoveParameterTarget` で除去できず、send gain target への
   adopt（`AddSend`/`SetSendGainTarget` — apply で owner/role を
   `Send`/`SendLevel` に書き換える経路）も拒否する。reserved range
   の entity は op 発行対象外、master bus「exactly one」と同じ構造的
   不変条件であり、send teardown が target を消して project を
   wedge させる経路を塞ぐ。load 時の oracle は well-known ID の
   存在に加え `role == Fader`・`owner == Strip(master)` を要求する
   （ID に擬態した foreign entry も拒否）。

## Non-Goals

- dB など plain domain の単位付き gain カーブ（capability 未設定、
  normalized=plain の線形 [0,1] のまま）。
- v1–v6 schema の snapshot への migrate 適用 — v5→v6→v7 の既存
  ladder が処理し、v7→v8 step が fader を補う。v4 以前は従来どおり
  `UnsupportedSchema` で拒否する。
- 他 strip（track strip 等）への fader target 展開、Pan/Mute/Send
  の UI 接続。mute/solo は既存 op の lazy 生成経路のまま。
- ruviform 側の変更。pin 更新は既存の追従手順のみ。

## Acceptance（Issue #185 転記 + 本 slice の語彙）

- `mixer.gain` 相当の projection 元が domain/service 層に存在し、
  Fader の controlled display（表示値）と commit（書き込み経路）が
  driver-neutral ViewSpec 経由で接続される。
- `main_screen_spec` の Fader から `unsupported: true` が除かれ、
  projection 元が確定した kind になる。
- headless: Fader の controlled display が実値（authoritative
  projection の `host_normalized`）を反映し、drag/commit が domain
  値へ到達することを `tests/view_mount.rs` で検証する。
- 実機: Windows 実窓で Fader 描画・ドラッグ操作の証拠を
  `.devin/evidence/` に記録する。Passed/Failed/Blocked/NotRun/
  Unsupported の語彙で記録し、未検証 OS/IME/webview は
  `NotRun`/`Unsupported` とする。
