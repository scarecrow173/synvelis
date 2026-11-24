---
id: "DISC-sv232WvInvLedger01xQ9z"
type: "discovery"
status: "resolved"
title: "SV232 Legacy WebView 棚卸し台帳 — Synvelis 側使用箇所・機能対応表・撤去分類規則"
created: "2026-11-24"
updated: "2026-11-24"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/232"
  related:
    - "https://github.com/scarecrow173/ruviform/issues/419"
    - "https://github.com/scarecrow173/ruviform/issues/465"
    - "https://github.com/scarecrow173/synvelis/issues/28"
    - "https://github.com/scarecrow173/ruviform/issues/294"
    - "https://github.com/scarecrow173/synvelis/issues/230"
    - "https://github.com/scarecrow173/synvelis/issues/114"
    - "https://github.com/scarecrow173/synvelis/issues/176"
    - "https://github.com/scarecrow173/ruviform/issues/630"
    - "https://github.com/scarecrow173/ruviform/issues/631"
---

# SV232 Legacy WebView 棚卸し台帳

synvelis#232 の headless 文書 slice として、Synvelis 側の旧 WebView 経路の
使用箇所を網羅列挙し、撤去可否の暫定分類・機能対応表・撤去分類規則の初版を
記録した台帳。物理撤去（AC3 以降）は本 slice の対象外であり、Windows L2
（sv#114 受入・rf#631 実 GPUI Window Runner）と Plugin Dock L3（sv#176 受入・
rf#630 Test Native Editor）の external gate 達成後の後続 slice で実施する。

## 対象 SHA と列挙の網羅性

- Synvelis 作業 HEAD: `4b899b3ce1def2db1feddc1d7d3c5266ad1b91b7`
- Ruviform 解決 rev: `cd4213a1d665600f6adfefa0c3a90f2be9b4e4ce`（`Cargo.lock`）
- 列挙コマンド・exit code・ヒット件数・台帳項目との対応は
  `evidence/sv232/enumeration.md` に記録し、第三者が同手順で再現検証できる。

## 分類軸

Issue 本文「legacy 専用と Project/Service/Window/IPC の共有意味論をファイル内で
区分し、無条件にファイル丸ごと消さない」に従い、3 分類 + 混在ファイルの
行単位区分を使う。RF#465 上流台帳（`ruviform/docs/discovery/
issue-465-legacy-webview-inventory.md`）の `remove_candidate` / `reuse` /
`provisional` / `exception` との対応も併記する。

| 分類 | 意味 | RF#465 対応 |
| --- | --- | --- |
| `legacy 専用` | webview bin / SolidJS frontend 経路だけが使う定義。共有意味論を含まず、経路撤去とともに削除できる | `remove_candidate` |
| `共有（保護対象）` | driver-neutral の Project/Service/Window/IPC 意味論。webview 経路撤去後も native/headless 経路が使い続けるため削除しない | `reuse` |
| `要維持` | 単一 revision guard 等、撤去後も残る統治機構。「用途の消失を検証するまで維持」（sv#230 方針） | `reuse` / `exception` |
| `暫定（provisional）` | 現時点では撤去可否を確定できない項目。gate 達成後の後続 slice が最終判定する | `provisional` |

## AC1 — 棚卸し台帳

### 1. Cargo 依存面

| 項目 | パス・行 | 使用形態 | Native 移行先 | 暫定分類 |
| --- | --- | --- | --- | --- |
| `ruviform-ui-webview` workspace dep | `Cargo.toml:77` | optional git dep（webview bin のみが到達） | なし（撤去） | `legacy 専用` |
| `ruviform-ui-webview` optional dep | `crates/synvelis-app/Cargo.toml:35` | `webview` feature 経由でのみ有効化 | なし（撤去） | `legacy 専用` |
| `webview` feature 定義 | `crates/synvelis-app/Cargo.toml:45-49` | `dep:ruviform-ui-webview` を束ねる feature | `synvelis-native` bin（feature 不要） | `legacy 専用` |
| bin `synvelis` required-features | `crates/synvelis-app/Cargo.toml:72-75` | `required-features = ["webview"]` | `synvelis-native` bin | `legacy 専用` |
| `Cargo.lock` 派生 entry | `Cargo.lock`（`ruviform-ui-webview`・`webview2-com*`・`wry`/`tao` 系） | dep 解決の派生物 | 依存削除時に `cargo update` で再生成 | `legacy 専用`（派生） |

### 2. `crates/synvelis-app/src/` コード面

| 項目 | パス・行 | 使用形態 | Native 移行先 | 暫定分類 |
| --- | --- | --- | --- | --- |
| `synvelis` product bin 全体 | `src/bin/synvelis.rs` | webview bin：`use ruviform_ui_webview`、`SessionConfig`/`WebViewWindowConfig`/`detect_backend`/`run_multi`、`rvf://app` scheme、exit code 2/3/4 | `src/bin/synvelis-native.rs` | `legacy 専用` |
| frontend bundle embed | `src/assets.rs` | `RustEmbed` で `frontend/dist` を同梱 + `error_asset_source`。consumer は webview bin のみ | なし（native は asset embed を持たない） | `legacy 専用` |
| assets module gate | `src/lib.rs`（`#[cfg(feature = "webview")] pub mod assets` 付近） | feature gate による公開制御 | gate ごと撤去 | `legacy 専用` |
| `check_bundle`（bundle 整合検査） | `src/revision.rs`（`check_bundle` 関数） | webview bin が起動時に呼ぶ frontend bundle 検査 | なし | `legacy 専用`（行単位） |
| `HOST_REVISION` 定数 | `src/revision.rs` | `SYNVELIS_RUVIFORM_REV` 検査の参照値。native bin・`syn_ctl`・uitest も参照 | そのまま維持 | `要維持`（行単位） |
| `SYNVELIS_RUVIFORM_REV` 単一 rev guard | `build.rs` | 単一 revision guard（sv#230 由来） | そのまま維持（`ui/vendor` 消失後も guard として機能） | `要維持` |
| `WindowHandle` port | `src/host.rs` | driver-neutral の Window handle 抽象。`synvelis-workspace` が consume する共有契約 | そのまま維持（上流 SV-004：RF の WindowHandle 撤去を block する consumer） | `共有（保護対象）` |
| backend report・`"webview"` capabilities | `src/app.rs:387`・`915`・`1459` | `BackendReport` 受理と capabilities 名 `"webview"` を報告する共有経路 | `BackendReport` 意味論は維持。frontend 向け `"webview"` capability 文字列のみ行単位で撤去候補 | `共有（保護対象）`（行単位区分） |
| `BackendUnavailable` | `src/error.rs` | driver-neutral の backend 不在 error variant | そのまま維持 | `共有（保護対象）` |
| `compose_product_routing` | `src/composition.rs` | webview bin が呼ぶ共有 routing 構成 helper。native bin は現在同名の local 変種（`compose_routing_engine` 使用）を持ち、共有 helper への移行は後続 slice（`tools/repair.mjs` 記録済み） | そのまま維持（native 側呼出統合は撤去後の整理事項） | `共有（保護対象）` |
| doc 言及のみの共有モジュール | `src/shell.rs`・`src/ui_ops.rs`・`src/binding.rs` | doc コメント中の `WebView` 文言（誤分類防止のため列挙） | そのまま維持 | `共有（保護対象）` |

### 3. `crates/synvelis-app/tests/` 試験面

`git grep -ln "webview" crates/synvelis-app/tests/` で 11 ファイルを検出。
いずれも feature gate を持たない headless 試験であり、`webview` 文字列の
使用形態で個別判定した。

| 項目 | パス | 使用形態 | 暫定分類 |
| --- | --- | --- | --- |
| PC-C4 webview composition acceptance | `tests/pc_c4_webview_composition.rs` | 共有 helper `compose_product_routing` の契約試験。名称に webview を含むが検査対象は driver-neutral の合成契約（`Option<PathBuf>` gate・engine attach/detach） | `共有（保護対象）`（ファイル名の webview は歴史的命名） |
| fixture backend 名利用の 10 ファイル | `tests/logging.rs`・`monitor_arm.rs`・`rcd_04_product_e2e.rs`・`recorder_wire.rs`・`recording_projection.rs`・`service_ops.rs`・`tim01_ppqn_app.rs`・`tim01_ppqn_preview.rs`・`tim01_ppqn_ui.rs`・`ui_ops.rs` | `BackendReport::available("webview2", ...)` を合成 fixture 名として使用する driver-neutral 試験。`service_ops.rs:68` は `backend.name == "webview2"` を検査する fixture 整合 assertion | `共有（保護対象）`（fixture 文字列は試験データであり経路依存ではない） |
| bundle/rev guard 試験 | `tests/asset_check.rs`（`webview` 文字列は含まないが `check_bundle`/`HOST_REVISION` を検査） | webview 経路の bundle check と共有 rev guard の双方を検査 | `暫定`（`check_bundle` 部分は経路撤去時に試験ごと整理、`HOST_REVISION` 部分は維持 — 行単位区分） |

### 4. `ui/` frontend 面（追跡 34 ファイル）

SolidJS+TS の旧 frontend 一式。ファイル丸ごと消さず、共有意味論を行単位で
区分する。`ui/vendor/ruviform`・`ui/node_modules`・`frontend/dist` は
gitignore 済み生成物であり、正本は `tools/sync-ruviform-ui.mjs` の
`PACKAGES`（`ui/core`・`ui/components`・`ui/binding`・`ui/viz`・`ui/guitest`）
と `Cargo.lock` 解決 rev。

| 項目 | パス | 使用形態 | Native 移行先 | 暫定分類 |
| --- | --- | --- | --- | --- |
| op 名 union | `ui/src/ipc.ts` | `OpName` union が `syn.*` op 契約名を mirror（`sys.capabilities`・`syn.app.info`・`syn.workspace.mount`・`syn.project.*`・`syn.control.*`・`syn.viz.*` 等 39 種） | `syn.*` op 契約は `service.rs`（約 187 登録）に残る。frontend 側 mirror は撤去 | `legacy 専用`（契約の正本は service 側） |
| binding/port・router | `ui/src/binding/`・`ui/src/router.ts` | `syn.control` receipt / `syn.viz` wire の TS 側 demux | GPUI 側 `binding.rs`・scenario が wire 契約を保持 | `legacy 専用`（wire 意味論は Rust 側に残存） |
| store/viewmodel | `ui/src/store.ts`・`ui/src/viewmodel/` | `syn.*` payload の TS 側 projection（signal-flow・probe viewmodel 含む） | GPUI 側 projection（`synvelis-workspace` view model） | `暫定`（native 側に対応 projection があるものは撤去可、ないものは parity 欠落として記録） |
| components/views/adapters/main | `ui/src/components/`・`ui/src/views/`・`ui/src/adapters/`・`ui/src/main.tsx` | SolidJS 画面（project panel・transport bar・console fader・viz view） | `synvelis-native` GPUI 画面 | `legacy 専用` |
| test | `ui/test/` | webview2 backend を mock する TS 試験 | headless `tests/`・GPUI scenario | `legacy 専用` |
| 設定・scripts | `ui/package.json`・`ui/tsconfig.json`・`ui/vite.config.ts` 等 | vite/tsconfig の `@ruviform/ui-*` → `vendor/ruviform` alias | なし | `legacy 専用` |
| 生成物 ignore 行 | `.gitignore:82-91`・`.worktreeinclude:51-61` | `ui/node_modules/`・`ui/vendor/`・`frontend/dist` の ignore/copy 定義 | 経路撤去時に行削除 | `legacy 専用` |

### 5. `mise.toml` lane 面

| 項目 | 行 | 使用形態 | 暫定分類 |
| --- | --- | --- | --- |
| `sync-ui-deps` | `mise.toml:190` 付近 | `tools/sync-ruviform-ui.mjs` で vendor を lock rev に同期 | `legacy 専用` |
| `install-ui` / `setup-ui` | `:200`・`:206` 付近 | ui npm 依存の install/setup | `legacy 専用` |
| `check-ui` / `test-ui` / `build-ui` | `:218`・`:226`・`:234` 付近 | frontend typecheck/test/build | `legacy 専用` |
| `check-app-webview` / `lint-app-webview` / `test-app-webview` | `:245-255` | `--features webview` の cargo lane | `legacy 専用` |
| `dev-ui` / `build-app` | `:334`・`:343` 付近 | webview 経路の dev/build 入口 | `legacy 専用`（native 経路入口は別 lane） |
| `check-ruviform-rev` | `:210` 付近 | 単一 revision guard lane（sv#230） | `要維持` |
| `check-headless-deps` | `:214` 付近 | headless 依存 guard | `共有（保護対象）`（curated webview 名リスト部分は行単位で更新） |

### 6. CI workflow 面

| 項目 | パス・行 | 使用形態 | 暫定分類 |
| --- | --- | --- | --- |
| `ui-verify` job | `.github/workflows/ci.yml:853-963` 付近 | frontend headless TS lane（ubuntu+windows、required check） | `暫定`（`ui/` 撤去と同時に job 撤去。撤去までは現行維持 — 撤去は後続 slice） |
| `ui-verify-macos` job | `ci.yml:966-1057` 付近 | 同上（非ブロッキング） | `暫定` |
| `needs` 参照・コメント | `ci.yml` `needs` 行・`:300` 付近コメント | downstream job の `needs` と lane 説明 | `暫定`（job 撤去時に整合） |
| `dep-guard`（manifest・lockguard） | `ci.yml` dep-guard job | 依存 guard の CI 検査 | `共有（保護対象）` |
| `daily-ci.yml` | `.github/workflows/daily-ci.yml` | webview 参照ゼロを実測（上流 SV-006 と一致） | `共有（保護対象）`（変更不要） |

### 7. `tools/` 面

| 項目 | パス | 使用形態 | 暫定分類 |
| --- | --- | --- | --- |
| `sync-ruviform-ui.mjs` | `tools/sync-ruviform-ui.mjs` | `ui/vendor/ruviform` を lock rev の `ui/*` 5 package で再生成 | `legacy 専用` |
| `check-ruviform-rev.mjs` | `tools/check-ruviform-rev.mjs` | `SYNVELIS_RUVIFORM_REV` 単一 revision guard（sv#230 由来） | `要維持`（`ui/vendor` 消失後も `Cargo.lock` 単一 rev 検査として機能） |
| `check-headless-deps.mjs` | `tools/check-headless-deps.mjs` | headless 依存 guard の curated リストに webview 系名を含む | `共有（保護対象）`（行単位で webview 名を更新） |
| `ci-summary.mjs` | `tools/ci-summary.mjs`（`:60-61`・`:72`・`:88` 付近）+ 対応 test | CI lane 名の集計対象に ui 系 lane を含む | `共有（保護対象）`（lane 撤去時に行単位で整合） |

### 8. mockups・assets・README・packaging 面

| 項目 | パス | 使用形態 | 暫定分類 |
| --- | --- | --- | --- |
| DAW 画面 mockup | `docs/mockups/synvelis-daw.html` | スタンドアロンの設計 mockup。README から参照。webview 経路の稼働物ではない | `共有（保護対象）`（維持 — 設計資料） |
| 製品 asset | `assets/`（branding・themes・workspaces） | driver-neutral の製品資産 | `共有（保護対象）` |
| README | `README.md` | mockup への参照のみ。webview 経路の稼働参照なし | `共有（保護対象）`（変更不要） |
| release packaging | 該当定義なし | webview 固有の packaging 定義は列挙で検出されず | — |

### 9. 共有保護対象の明示（撤去禁止の再確認）

- `build.rs` + `tools/check-ruviform-rev.mjs` + `src/revision.rs` の
  `SYNVELIS_RUVIFORM_REV`/`HOST_REVISION` 単一 revision guard（sv#230 由来。
  `ui/vendor` 消失後も `Cargo.lock` 単一 rev 検査として要維持 —
  「用途の消失を検証するまで維持」）。
- `src/host.rs` の `WindowHandle` port（`synvelis-workspace` consumer —
  上流 SV-004 provisional。RF 側 WindowHandle 撤去を block する共有契約）。
- `src/app.rs` の `BackendReport` 受理経路・`src/error.rs`・`src/host.rs`・
  `src/composition.rs`・`src/shell.rs`・`src/ui_ops.rs`・`src/binding.rs` の
  driver-neutral 共有意味論。
- `service.rs` の `syn.*` op surface（約 187 登録）と全 driver-neutral 試験。
- `docs/`・`evidence/` の歴史文書 — E1 の 762 ヒットの大部分は記録文書であり、
  「歴史的 ADR/PoC/Issue/数値は保管」（RF#465 §5 D9 と同根拠）により撤去対象外。
- `.agents/skills/verify-synvelis` — #232 撤去予定を honest blocked として
  既に記録済みのエージェント資産。

## AC2 — 機能対応表

webview 経路が提供していた機能と、native/headless 側の被覆状況の対応。
GPUI 側の比較基盤は `evidence/sv114/l1-contract-comparison.md`・
`evidence/sv114/svt-headless-l1.md`（SVT-01〜09 headless Pass map）・
`evidence/sv176/pd-headless-contract.md`・`evidence/sv176/
pd-01-07-case-definitions.md`（`view-mount` headless Pass のみ、
PD-01〜07 実機 lane は全て `NotRun`）。

記号: `Pass` = headless L1 で証拠済み、`Scenario` = GPUI Fake/シミュレート
scenario で証拠済み、`NotRun` = 定義済みだが未実行、`—` = 該当 lane なし、
`未提供` = webview frontend が機能を提供していなかった（parity 欠落ではなく
非対象）。

| 機能 | webview 経路が提供 | Headless UI | GPUI Fake Scenario | RT | Session 契約 |
| --- | --- | --- | --- | --- | --- |
| Project | project.new/open/save + project panel（`ui/src/ipc.ts` op 面） | Pass（SVT-01 create/open→view、SVT-08 save/reload oracle） | — | RT 非依存 | Pass（`syn.project.*` op、`service_ops.rs` 等） |
| Transport | transport bar の play/stop/seek（`syn.control.*` receipt 連動） | Pass（SVT-05 Req/Applied/Observed・seek receipt） | Scenario pass（`seek_parity` + `invoke_scenario`） | Pass（rt_* contract 試験、`test-rt` lane） | Pass（`syn.transport.*`/`syn.control.*` op） |
| Fader | console view の fader strip + knob（gesture→receipt→UI） | Pass（SVT-03 fader gesture→Applied→Observed） | Scenario pass（`fader_parity_scenario`） | Pass（control surface は RT 非ブロッキング設計、drain gated） | Pass（`syn.control.*` op） |
| Mixer | console view の限定的 strip（meter 連動。フル mixer ではない） | Pass（SVT-06 meter/viz bounded + stale resync） | — | Pass（meter push_frame/snapshot は bounded contract） | Pass（`syn.viz.*`/`syn.mixer` 系 op） |
| Undo | 未提供（frontend `OpName` union に `syn.project.undo`/`redo` を含まず — E12 で実測） | Pass（SVT-04 undo/redo + rejected truth、project-layer CAS 試験） | — | RT 非依存 | Pass（`syn.project.undo`/`redo` op、`view_mount.rs` 等他窓 undo 試験） |
| 2Window | 未提供（単一 window の workspace mount のみ） | Pass（SVT-07 second window adopt・close ownership・shared session） | Scenario pass（secondary seek parity） | RT 非依存 | Pass（`syn.view.attach`/`detach`/`list` op。実 OS 2Window/Focus/native dock = GUI-L2 `NotRun`） |
| Plugin Panel | 未提供（plugin editor/dock UI なし） | Pass（`plugin_dock_contract` Fake 系試験、`rf631.plugin-dock` `view-mount` headless Pass — PD 証拠 §115-124） | Scenario pass（`plugin_dock_contract_gpui`） | `NotRun`（実 plugin ABI/RT は PD-01〜07 gate） | Pass（`syn.app.info` plugin surface unavailable を正直に報告 — 製造しない。PD-01〜07 実機 = 全て `NotRun`、EXT-L3 gate） |

### parity 欠落の記録（未被覆セル）

- webview frontend が提供していた全 7 機能区分のうち、frontend 側で実際に
  UI 提供されていたのは Project・Transport・Fader・Mixer（限定）・Viz の
  表示面。Undo・2Window・Plugin Panel は frontend に UI がなかったため、
  webview 撤去による parity 欠落は存在しない（撤去で失われる製品機能なし）。
- native/headless 側では 7 行全てに headless L1 被覆（Pass）がある。
  GUI-L2/EXT-L3 のみが `NotRun` であり、これは Issue の「L1 成功を実 GUI
  パリティ保証としない」「実 GUI/実 Plugin Dock/OS 実 Window L2/L3 は一律
  撤去 Blocker から除く」規則と整合する。
- `viewmodel` の signal-flow/probe projection は frontend 側に存在するが
  未 mount の状態（SV 系試験で wire 契約は headless 被覆済み）。native 側の
  対応画面実装状況は撤去実行 slice で再確認する暫定項目とする。

## AC5 — 責務・rollback・未削除例外の分類規則（初版）

RF#419（CLOSED）・RF#465（OPEN、GitHub 上の正式 blockedBy だが意味方向は逆の
循環依存 — rf#465 の上流撤去は SV#232 等の consumer 撤去解消後という関係）・
SV#28・RF#294（CLOSED）・SV#230（CLOSED）との責務分界と、撤去実行時の
rollback・例外ルールの初版。最終台帳は撤去実行時に確定する。

### 責務分界

- **SV#232（本 Issue）**: Synvelis 側 consumer 面の撤去 — bin・feature・
  frontend・CI lane・tool・dep。上流撤去の前提となる consumer 解消を担う。
- **RF#465（上流）**: ruviform 側の legacy webview 共有 adapter 撤去。
  consumer（SV 側）の撤去完了が前提。削除判定は RF#465 の 4 条件
  （legacy consumer 不使用・独立 Oracle PASS・same-SHA tuple PASS・
  rollback 可能）に従う。
- **RF#419（CLOSED）**: 上流の IPC/binding 契約整理。共有 `syn.*`/`syn.control`
  wire 意味論の正本は上流契約であり、SV 側 frontend の撤去は契約自体を
  消さない。
- **SV#28**: Synvelis 側の native 移行系の前段。本棚卸しの native 移行先列は
  その系譜に従う。
- **RF#294（CLOSED）**: 上流の driver-neutral 抽象化。`host.rs` の
  `WindowHandle` port 等の共有契約はこの系譜の成果物であり保護対象。
- **SV#230（CLOSED）**: 単一 revision guard の導入。`build.rs`/
  `check-ruviform-rev.mjs` は撤去後も維持する統治機構。

### rollback 規則（初版）

- 撤去は consumer 側（本 Issue 面）を先に、上流共有 adapter（RF#465 面）を
  後に行う順序。各段で same-SHA tuple（lock rev と検査対象の一致）が
  PASS することを確認してから次段へ進む。
- 撤去コミットは論理単位で分割し、各 commit が独立して revert 可能な形に
  する（bin/feature → frontend → CI/tool → dep の順を目安）。
- `Cargo.lock` の再生成は撤去 commit に含め、単一 rev guard（sv#230）が
  fail-closed で検出できる状態を常に保つ。

### 未削除例外（初版・暫定）

- `build.rs`・`tools/check-ruviform-rev.mjs`・`src/revision.rs` の
  `HOST_REVISION` — 単一 revision guard として撤去後も維持（用途の消失を
  検証するまで維持）。
- `src/host.rs` の `WindowHandle` port — `synvelis-workspace` consumer が
  残る間は例外（上流 SV-004 provisional と同期）。
- `check-headless-deps.mjs` の guard 本体 — webview 名の更新のみで機構は維持。
- `docs/mockups/synvelis-daw.html`・`assets/` — 設計資料・製品資産として例外。
- `docs/`・`evidence/` の歴史記録 — 記録として保管（撤去対象外）。
- 上記以外に撤去実行時に発覚した未削除項目は、例外として本節に追加して
  確定する（本 slice では初版の凍結まで）。

## 棚卸しの範囲外・残件

- AC3 物理撤去 — external gate（sv#114 Windows 実 GUI L2 受入・sv#176
  Plugin Dock L3 受入・rf#630 Test Native Editor・rf#631 実 GPUI Window
  Runner — いずれも OPEN/NotRun）達成後の後続 slice。
- AC4 撤去後の 3OS 既存 CI 証拠 — 撤去実行後に対象 SHA で取得。
- AC5 最終台帳 — 撤去実行時に未削除例外を確定。
- AC6-AC8 最終受入 — gate 達成後。
- `synvelis-native` bin の `compose_product_routing` 共有 helper への
  呼出統合（local 変種からの移行）は撤去後の整理事項（`tools/repair.mjs`
  記録済みの後続 slice）。
- `docs/discovery/README.md`（生成 index）の同期は本 slice の範囲外 —
  Coordinator が統合時に整備する。
