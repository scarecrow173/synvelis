# AUTX-07 mount wiring — Case 台帳 (Issue #266 SV266-NR-05 slice)

作成: 2026-11-17 / branch: `feature/sv266-autx7-mount-wiring`
対象: `crates/synvelis-workspace/src/automation_ui.rs`・
`crates/synvelis-app/src/{automation,app,binding,service,shell,
ui_ops}.rs`・`crates/synvelis-app/src/bin/synvelis-native.rs` と
対応 test

語彙は POLICIES.md §2/§3 に従う。**headless Pass は実機 Pass では
ない**。実機未検証項目は Case ID 付きで NotRun/Deferred として
記録する。本スライスは `crates/synvelis-gateway`・
`crates/synvelis-automation`・`assets/themes/` を変更しない。
前 slice の台帳は `evidence/sv266/README.md`（SV266-NR-05 を
本 slice で消化）。

検証環境: Windows 11 (x86_64)、rustc は `mise` 管理 toolchain、
`cargo --locked` による補助証跡。canonical 検証は `mise run` レーン
（PR 本文に記録）。

## 1. 受入条件 → Case → test → 結果

| Issue #266 受入行（SV266-NR-05） | Case | 検証内容 | test / 証跡 | Expected | Observed | Result |
| --- | --- | --- | --- | --- | --- | --- |
| composition root が 4 automation surface を mount し `autx.*` action を shell 経由で dispatch する | SV266W-AC-01 | `syn.autx.visual{open}` が 4 surface を detached view として adopt、`view-{n}` 返却、automation screen cell が surface 別 state を publish、close で entry 消失 | `synvelis-app/tests/sv266_wiring.rs`: `autx_visual_opens_four_surfaces_with_published_states`・`autx_visual_close_drops_published_state`・`autx_visual_rejects_unknown_surface_and_foreign_view` | 4 distinct `view-{n}`・variant 一致の `AutomationScreenState`・`automation_screen_spec` 構成可能・close で publish map から除去 | 3 tests pass | **Pass（headless）** |
| `autx.*` target の params 付き intent が `decode_view_command` を通過し `decode_automation_command` へ届く。domain action の params は reject 維持 | SV266W-AC-02 | binding decoder の params gate + malformed/unknown の typed reject | `sv266_wiring.rs`: `decode_view_command_admits_autx_params_and_rejects_domain_params`・`autx_action_rejects_unknown_target_action_and_malformed_params`。既存 `autx07_automation.rs` decode matrix | autx target+params → `action_params=Some`、domain target+params → error、unknown target/action・malformed params → `invalid_params` | 2 tests pass + 既存 7 tests pass | **Pass（headless）** |
| shell が `autx.*` を `syn.autx.action` へ route し Result が到達 channel へ返る（`syn.control.dispatch`/`parse_target` 非経由） | SV266W-AC-03 | mounted lane: app → automation `ViewHost` → `NativeShell` → `FakeDriver`（surface 毎 `Ready` window + `attach_view`）で table select / button invoke が `rvf.ui.command.v1` → `syn.autx.action` → window `Result` を完結 | `sv266_wiring.rs`: `mounted_palette_select_and_submit_dispatch_through_shell`・`mounted_unknown_autx_action_rejects_on_originating_channel` | select → `kind:"view-local"` + `state.palette_selected` 更新、submit → `kind:"request"` + lowered `requests`、unknown action → 同一 window で `ok:false` | 2 tests pass | **Pass（headless）** |
| view-local action は `ViewState.automation` のみを変更し project 正本・revision を動かさない | SV266W-AC-04 | select/category が revision 不変で `palette_selected`/`palette_category` を更新、category 変更で stale row index をクリア | `sv266_wiring.rs`: `autx_local_select_mutates_only_view_state` | `kind:"view-local"`・`requests:[]`・revision before==after・`palette_selected=None` after category | 1 test pass | **Pass（headless）** |
| catalog 未 push の request action は typed rejection（fake success なし）、view-local は影響なし | SV266W-AC-05 | catalog 無しで submit → `unavailable`、select は通常どおり mutate | `sv266_wiring.rs`: `autx_request_without_catalog_rejects_typed` | error code `unavailable`・view-local 継続 | 1 test pass | **Pass（headless）** |
| catalog push 後の request action が wire request JSON へ lowering（consistency・idempotency・target 検査維持） | SV266W-AC-06 | `syn.autx.catalog` push → submit/preview/macro select/apply が `requests` を返す。malformed catalog push は全体 reject で旧 catalog 温存 | `sv266_wiring.rs`: `autx_catalog_push_enables_request_lowering`・`autx_macro_select_apply_lowers_per_step_requests`。`synvelis-workspace/tests/autx07_catalog_seam.rs` 全 10 test | submit→`expected-revision`、preview→`latest`、macro apply→step 毎 request + `<key>/<index>` idempotency、unknown capability/target mismatch → typed error | 2 + 10 tests pass | **Pass（headless）** |
| producer 不在の workflow/run surface は honest empty state + typed reject で劣化 | SV266W-AC-07 | run select/cancel が monitor projection 不在を `unavailable`/`invalid_params` で表明、fake run id を保存しない | `sv266_wiring.rs`: `autx_run_select_and_cancel_reject_without_monitor_data` | select → `unavailable`、cancel（未選択）→ `invalid_params` | 1 test pass | **Pass（headless）** |
| `synvelis-app` が `synvelis-gateway` に直接依存しない（AC-05 維持） | SV266W-AC-08 | `AutomationCatalog` seam が JSON 境界で gateway 型を app から隠蔽 | `check-headless-deps` + `cargo tree` 目視 + app test が literal catalog JSON のみ使用 | app の依存木に `synvelis-gateway` 無し | Cargo.toml 依存一覧に gateway 無し（dev-deps 含む） | **Pass** |
| 実 GPUI/実 OS の mount・Pointer/Keyboard/IME/Focus/DPI/2Window/WindowClose 受入 | SV266W-NR-01 | 実 OS lane — 本スライス範囲外 | — | — | — | **NotRun**（#114/#267 別 Gate） |
| `SYNVELIS_AUTX_VIEWS` gate 下の native 実窓での 4 window open/attach/spec push | SV266W-NR-02 | native bin 配線は実装済みだが実窓起動は手動 lane | `mise run run-app-native`（env `SYNVELIS_AUTX_VIEWS=1`） | 実窓で 4 surface が開き操作できる | 未実施 — 実機 Pass に昇格しない | **NotRun** |
| live `synvelis-automation` adapter（#264）からの catalog/plan/monitor 実供給 | SV266W-NR-03 | producer 経路は adapter slice の責務 | — | — | — | **Deferred**（#264） |

## 2. Case 詳細（headless 実施分）

| Case | crate::test | 状態 | 備考 |
| --- | --- | --- | --- |
| SV266W-T-01 | `synvelis-app::sv266_wiring` 全 12 test | Pass | mount register・decode/dispatch・local-only・no-catalog・catalog-lower・strict reject・mounted lane 2 件 |
| SV266W-T-02 | `synvelis-workspace::autx07_catalog_seam` 全 10 test | Pass | strict parse・grant 再導出・projection・lowering・capture・preset 解決 |
| SV266W-T-03 | `synvelis-app::autx07_automation`・`binding`・`shell`・`ui_ops` 既存 test | Pass | 回帰なし（decode matrix・view-state lifecycle・wire translate 等） |
| SV266W-T-04 | `cargo fmt --all --check` | Pass | fmt 適用済み |
| SV266W-T-05 | `mise run verify-full`（check/lint/test workspace） | 記録は PR 本文 | canonical lane |
| SV266W-T-06 | `mise run check-headless-deps` | 記録は PR 本文 | 依存方向 gate |
| SV266W-T-07 | `mise run check-app-native`/`lint-app-native`/`build-app-native` | 記録は PR 本文 | `synvelis-native.rs` 変更の native feature 検証 |

## 3. NotRun / Deferred 対応表

| Case | 内容 | 状態 | Required/Deferred | 後続 gate / 必要ファイル |
| --- | --- | --- | --- | --- |
| SV266W-NR-01 | 実 GPUI Windows/macOS/Linux Pointer/Keyboard/IME/Focus/DPI/2Window/WindowClose・Alert/Undo/Save/Reload | **NotRun** | Required（GUI-L2）だが本スライスは headless-first | synvelis#114/#267。`SYNVELIS_AUTX_VIEWS` gate の実窓 lane で Case ID を引き継ぐ |
| SV266W-NR-02 | `SYNVELIS_AUTX_VIEWS=1` での `synvelis-native` 実窓 4 surface 手動検証 | **NotRun** | Deferred（実機 lane — headless では spec/attach/dispatch まで検証済み） | `mise run build-app-native` + `run-app-native`。window title・layout・実 pointer 操作の確認 |
| SV266W-NR-03 | `synvelis-automation` live adapter（#264）からの catalog/plan/monitor push | **Deferred** | #264 別 Worker リース | `syn.autx.catalog` op が JSON 境界として既に注入可能 — producer 接続のみ残る |
| SV266W-NR-04 | Workflow plan / Run monitor projection の publish 経路（`WorkflowPlanView`/`RunMonitorView` producer） | **Deferred** | #264 adapter slice の責務 | 現状は honest empty state + run select/cancel の typed reject で正直に劣化。`publish_automation_screens` は producer 投入時にそのまま使える |
| SV266W-NR-05 | gpui-test semantic scenario への 4 surface 追加（SV266-NR-06 相当） | **Deferred** | 任意 lane | `mise run test-app-gpui` が必要になる変更を含む後続 slice |

## 4. 補足

- catalog seam（`AutomationCatalog`）は「push 文書 = parse 対象」の
  単一契約。`synvelis-app` は gateway 型を名指さず、projection 参照
  と lowering 結果 JSON のみを消費する（`check-headless-deps` で
  依存方向を担保）。
- `ViewState.automation` が view-local state の唯一の正本。
  `PaletteCategory` 選択時に `palette_selected` をクリアするのは
  domain filter で row index が再 base されるため（stale index が
  別 capability を指すのを防ぐ fail-closed）。
- request lowering の `requests` は wire request JSON であり、
  実行・receipt は caller の gateway lane の責務 — op は catalog
  不在・lowering 失敗のいずれでも fake success を返さない。
- native run loop は publish 型に `PartialEq` が無いため
  `AutomationScreenPublish.rev` stamp で更新検出する
  （`docs/impl/exp/autx-07-mount-wiring.md` E2）。
