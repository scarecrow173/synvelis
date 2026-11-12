---
id: "PLAN-Jf4tYu7cVb1nMk9sLd3rXe"
type: "plan"
status: "approved"
title: "SV-GUI-03 slice5 実施計画 — GPUI-simulated scenario lane"
created: "2026-11-12"
updated: "2026-11-12"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/112"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/sv-gui-03-05-gpui-scenario.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/sv-gui-03-05-gpui-scenario-design.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/sv-gui-03-05-gpui-scenario.md"
    - "docs/designs/sv-gui-03-05-gpui-scenario-design.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/sv-gui-03-05-gpui-scenario.md"
  references: []
  defers: []
  deferred-by: []
---

# SV-GUI-03 slice5 実施計画

## 前提確認

- ruviform pin rev は `fd1d4d5f4de57092b96ad1cd309b85fe47cbef38`
  (`Cargo.lock`)で、`ruviform-ui-gpui` の `gpui-test` feature・
  `GpuiGuiTest`/`GpuiObserver`/`ViewMount`/
  `GpuiDriver::create_view_window` を含む。pin 更新不要、
  `sync-ui-deps` も不要。
- slice4 までの product spec は `arrangement.table`
  (`TableSelectionSpec` on `VIEW_STATE_TARGET`)・`screen.tabs`
  (`TabsSelectionSpec`、5 tab・active 0)・`seek.position`/
  `seek2.position`(Seek on `transport/position`)・`fader.gain`
  (Fader on `mixer_gain_target()`)・`transport.play`/
  `transport2.play`(Button on `transport`)を実装済み。
  scenario は既存 spec の mount のみで、製品コード変更なし。
- audio-lens `crates/al-gui` が `gpui-test` feature・
  `tests/common/scenario.rs`・`tests/scenario_gpui.rs`・
  `*-ui-gpui` mise lane の完全な先行例。

## 手順

1. `crates/synvelis-app/Cargo.toml` に `gpui-test` feature を
   追加(`native` + `ruviform-ui-gpui/gpui-test`)。
2. `tests/common/mod.rs` + `tests/common/scenario.rs` を新規
   作成 — driver-neutral scenario helper・product oracle・
   honesty helper・`ScenarioReport` 骨格。
3. `tests/screen_scenario.rs` を共通 helper 利用へ最小リファクタ
   (Fake lane 維持 + secondary window・select/tab・honesty
   scenario を追加)。
4. `tests/scenario_gpui.rs` を新規作成
   (`#![cfg(feature = "gpui-test")]`)— mount helper・
   main scenario・2 window・Fake/GPUI parity・stale pin・
   unsupported honesty の 5 test。
5. `mise.toml` に `check-app-gpui`/`lint-app-gpui`/
   `test-app-gpui` を追加(webview/native lane と同形)。
6. docs チェーン作成(spec/design/plan/task/test-spec/ir/ADR)。
7. canonical 検証一式を実走:
   `fmt-check`/`check`/`lint`/`test`/`check-app-gpui`/
   `lint-app-gpui`/`test-app-gpui`/`check-app-native`/
   `lint-app-native`/`test-msrv`/`build`。
8. Conventional Commits で commit → push → PR(日本語、
   `Refs #112`)→ Issue #112 コメント → `worker_done`。

## File map

| file | 変更 |
| --- | --- |
| `crates/synvelis-app/Cargo.toml` | `gpui-test` feature 追加 |
| `crates/synvelis-app/tests/common/mod.rs` | 新規(`pub mod scenario` + `allow(dead_code)`) |
| `crates/synvelis-app/tests/common/scenario.rs` | 新規(driver-neutral scenario/oracle/honesty/report) |
| `crates/synvelis-app/tests/screen_scenario.rs` | 共通 helper 利用へリファクタ + scenario 追加 |
| `crates/synvelis-app/tests/scenario_gpui.rs` | 新規(GPUI lane、`gpui-test` gate) |
| `mise.toml` | `check/lint/test-app-gpui` task 追加 |
| `docs/{specs,designs,plans,tasks,test-specs,impl/ir,adr}/sv-gui-03-05-gpui-scenario*.md` | 新規 doc chain |

## Risk register

- **gpui 初回 compile が重い**: `check-app-gpui` が gpui 依存を
  引く。task 分離済み、`check`/`lint`/`test` 既定 lane は不変。
- **`GpuiGuiTest` が `!Send`/`TestAppContext` 制約**: audio-lens
  と同じ plain `#[test]` + `TestAppContext::single()` 構成で
  問題ない想定。失敗時は adapter 側制約として報告。
- **select/tab の `phase`**: Applied receipt post 後に
  `CommandPhase::Applied` を oracle とする。receipt routing は
  `emit_intent` の `command_index` 経由で Button invoke と同一
  機構 — view_mount.rs の既存検証と整合。
- **`screen.tabs` が 5 tab**: task の「4 タブ」は non-default
  tab 巡回と解釈(spec/design に明記)。
- **共有 `mise.toml`**: task 追加のみ・既存 task 無変更。
  rollback は task 削除のみ。

## Rollback

- `gpui-test` feature・`tests/scenario_gpui.rs`・
  `tests/common/`・`mise.toml` の 3 task を削除すれば完全に
  戻る。`screen_scenario.rs` は共通 helper を残しても
  Fake lane として機能する(共通 helper は test 内完結)。

## Verification matrix

| task | 目的 |
| --- | --- |
| `mise run fmt-check` | rustfmt |
| `mise run check` | workspace headless check(gpui 非依存確認) |
| `mise run lint` | workspace clippy -D warnings |
| `mise run test` | workspace test(Fake lane 含む) |
| `mise run check-app-gpui` | gpui-test feature check |
| `mise run lint-app-gpui` | gpui-test feature clippy |
| `mise run test-app-gpui` | GPUI scenario lane 本体 |
| `mise run check-app-native` | native lane 非回帰 |
| `mise run lint-app-native` | native lane lint 非回帰 |
| `mise run test-msrv` | MSRV 非回帰 |
| `mise run build` | workspace build |
