# SV#28 ruviform pin 20dc41f7 → 87a7ba8f 差分精査メモ

実施日: 2026-11-26 / Worker worktree: `G:/orca/workspaces/synvelis/sv28-pin-current`
branch: `chore/28-ruviform-pin-current`（base `origin/main` = `01c99ea2b2f1b2509c9208b8d31c66fb6b3c7008`）

## candidate SHA 再解決

- dispatch 時点の ruviform `origin/main` HEAD を再解決し、candidate を
  `87a7ba8ff3fbe6d4502bea27d152d7f74c105899`（sha8 `87a7ba8f`）に固定した。
- before rev は synvelis `Cargo.lock` の実測値
  `20dc41f71c47674b3e51d1eb0921124b94f8bfaf`。
- `20dc41f7..87a7ba8f` は fast-forward で **71 commits**。
- candidate 固定後の 2026-11-26 再読取では upstream `origin/main` が
  `0fb97d1ba89e1fee154cdbbb72ceb2db15fdcd8c` へ前進していた。本 slice は
  dispatch 時点 candidate `87a7ba8f` を freeze し、後続分は次回 rev-watch slice。
- 調査コマンド（補助証拠、canonical = mise）:
  `git log --reverse 20dc41f7..87a7ba8f`、`git diff --stat/name-status`、
  crate 別 `git diff`、synvelis 側 grep、`git diff --numstat -- Cargo.lock`。

## upstream diff 規模（crates/）

- `git diff --stat 20dc41f7..87a7ba8f -- crates/`:
  **35 files changed, +7,685 / −543**。
- 変更 crate 13:
  `ruviform-audio-device`、`ruviform-audio-plugin-bridge`、
  `ruviform-audio-test`、`ruviform-io-archive`、`ruviform-midi-device`、
  `ruviform-plugin-clap`、`ruviform-plugin-core`、`ruviform-plugin-export`、
  `ruviform-plugin-host`、`ruviform-plugin-test-clap`、
  `ruviform-plugin-test-vst3`、`ruviform-plugin-vst3`、`ruviform-ui`。
- upstream `tools/` には ruviform #1022 の
  `Provision-RvfVirtualAudioEndpoint.ps1`・`Unprovision-RvfVirtualAudioEndpoint.ps1`・
  `RvfVirtualAudioProvisioning.psm1` が追加されている。これは `crates/` 集計外で、
  synvelis 依存木ではなく consumer workflow も本 slice では追加しない。

## synvelis 依存木との交差

synvelis `Cargo.lock` の ruviform package/source は **36**。変更 crate と
lockfile package の交差は 11。`ruviform-io-archive` と
`ruviform-midi-device` は lockfile に存在せず、直接 compile 影響はない。

| crate | upstream 差分の性質 | synvelis への影響 |
| --- | --- | --- |
| `ruviform-plugin-core` | `Plugin::buses` / `ErasedPlugin::buses` が `Option<&[BusDescriptor]>` へ変更。`drain_param_notices` default 追加。`ParamNotice`、`ParamNoticeQueue`、`RescanAspects`、`ClearAspects`、`PresetLocation`、wire v9 codec 追加。`WIRE_VERSION` 8→9 | **破壊的変更あり**。synvelis test fixture 36 件が旧 signature。production `src/` の `.buses()` 参照と `WIRE_VERSION` 参照は grep で 0 件。`ParamNotice` consumer も 0 件 |
| `ruviform-plugin-host` | `bus_report` projection を追加し、`Some(table)` → `BusReport::Reported`、`None` → `BusReport::Unavailable`。`drain_param_notices` を L0/L1 slot kind に routing | 公開 `BusReport` enum 自体は維持。synvelis tim03 oracle は `Unavailable` 分岐を既に扱う。`drain_param_notices` は未使用 |
| `ruviform-plugin-export` | export-side L1 dispatch（wire tag 29/30）、GUI/focus helper、runner/test 経路更新 | host/runner/test plugin が同一 lock から build されるため wire v9 mismatch は起きない設計。`test-tim03-l1` で検証 |
| `ruviform-plugin-clap` | host の `ParamNotice` queue/preset/rescan/clear/flush 実装、gesture notice 変換、focus helper、Cargo feature 調整 | additive host capability。synvelis 直接 consumer なし |
| `ruviform-plugin-vst3` | host focus helper と editor/host path 更新 | additive/internal host capability。synvelis 直接 consumer なし |
| `ruviform-plugin-test-clap` / `ruviform-plugin-test-vst3` | host path/L1 stateful/OS endpoint MIDI test 更新 | test fixture crate。synvelis production 影響なし。L1 artifact laneで間接確認 |
| `ruviform-ui` | `ComponentSpec::Input`/`InputSpec`/`InputValueSpec` と `A11yRole::TextBox`/`SpinButton` 等の input widget surface 追加 | additive。`ComponentSpec` は `#[non_exhaustive]`。synvelis 製品 UI での採用はなし。vendor は同一 rev へ同期 |
| `ruviform-audio-device` | `tests/virtual_endpoint.rs` のみ | library source 差分なし。compile 影響なし |
| `ruviform-audio-plugin-bridge` | upstream tests のみ（e2e/frame_base/midi_wiring/param_delivery） | library source 差分なし。consumer 影響なし |
| `ruviform-audio-test` | `tests/os_endpoint.rs` のみ | test helper crateの library source 差分なし |
| `ruviform-io-archive` | weak-FS cleanup 修正（`src/extract/fs.rs`/`session.rs`） | synvelis lockfile 依存木外。capability差分として記録のみ |
| `ruviform-midi-device` | Windows.Devices.Midi2 probe/example と windows-0.62 dev-dep 系 | synvelis lockfile 依存木外。examples/dev-dep中心で consumer compile 影響なし |

## pub API 破壊スキャン

- 削除行側の `pub` item scan で顕在化した公開契約変更は
  `pub const WIRE_VERSION: u16 = 8` の置換（v9 へ）と、
  `Plugin::buses` / `ErasedPlugin::buses` signature 変更。
- synvelis 側 `WIRE_VERSION` 参照は 0 件。`.buses()`/`fn buses()` の
  production `src/` 参照も 0 件で、必要適応は test fixture trait impl のみ。
- `ParamNotice` 関連 API、`drain_param_notices`、Input widget API は additive。
  `ParamNotice|drain_param_notices` の synvelis consumer grep は 0 件。
  `ComponentSpec::Input` / `A11yRole::TextBox` / `SpinButton` の製品 UI consumer も 0 件。

## 新 API / capability の採用判断

| API / capability | 判断 | 理由 |
| --- | --- | --- |
| ruviform #912 wire v9 `ParamNotice` / `drain_param_notices` | **未採用・候補** | additive。DAW automation/live UI の設計を伴うため pin slice を超える |
| ruviform #999 Input widget / textbox/spinbutton a11y role | **未採用・候補** | additive UI primitive。製品 UI 採用は別 slice |
| ruviform #1022 virtual endpoint provisioning primitive | **未採用・候補** | synvelis #115 Slice B の前提。manual/provisioned endpoint 実走は本 slice 非対象 |
| ruviform #630 系 L1/editor/focus/test-plugin path | **pin の一部として採用** | 新規 consumer workflow は追加しないが、同一 lock の host/runner 更新として受け取り、`test-tim03-l1` で非回帰確認 |
| `ruviform-io-archive` weak-FS cleanup、`ruviform-midi-device` probe | **非影響として記録** | synvelis lockfile 依存木外 |

## synvelis 適応 diff

- `Cargo.lock`: **36 insertions / 36 deletions**。36 ruviform source fragmentの
  `#20dc41f7…` → `#87a7ba8f…` 置換のみ。無関係な version drift・新規 package・
  依存リスト変更なし。
- test 適応: **37 files, +111 / −90**。
  - fixture 36 files（`crates/synvelis-app/tests` 5 件、
    `crates/synvelis-player/tests` 31 件）では signature を
    `fn buses(&self) -> Option<&[BusDescriptor]>` へ合わせ、返り値を
    `Some(...)` にした（各 file +2/−2）。
  - `crates/synvelis-app/tests/tim03_real_abi_process_oracle.rs` は範囲拡張許可後に
    pin 起因 oracle 適応を行った（+39/−18）。in-process real ABI adapter が
    stereo main input/output を authoritatively report するようになったため、
    旧 `Reported([])` / bus incompatibility 前提を、exact stereo bus report と
    `ProcessorConnectError::Unavailable` / `RoutingFault::PluginUnavailable` の
    endpoint refusal oracle へ更新した。assertion/comment/report のみで、
    production 実装や oracle 緩和はない。
- 全 fixture は static/held bus table（`BUSES`、`STEREO_BUSES`、
  `STATE_BUSES`、`MONO_IN_BUSES`、または `self.buses`）を返す declared table で、
  `None` = unavailable に相当する fixture はない。`Some(&[])` と `None` の
  意味混同はない。
- `crates/*/src/`・`Cargo.toml`・`mise.toml`・tools・CI・docs index は変更なし。

## rollback 記録

- before rev: `20dc41f71c47674b3e51d1eb0921124b94f8bfaf`
- target rev: `87a7ba8ff3fbe6d4502bea27d152d7f74c105899`
- synvelis base: `01c99ea2b2f1b2509c9208b8d31c66fb6b3c7008`
- rollback 手順（未コミット時）:
  1. `mise exec -- cargo update -p ruviform-media-core --precise 20dc41f71c47674b3e51d1eb0921124b94f8bfaf`
     で lock を before rev へ戻す。必要なら ruviform package を scoped に追加指定する。
  2. それでも生成 diff が残る場合に限り、未コミット作業内で
     `git checkout -- Cargo.lock` を使う。
  3. fixture 適応 diff は未コミットなら `git checkout -- crates/synvelis-app/tests crates/synvelis-player/tests`、
     push 後なら force push せず revert PR / dependency rollback PR とする。
  4. `mise run sync-ui-deps` と `mise run check-ruviform-rev` で一致確認する。
- rollback 実行: **NotRun（採用のため不要）**。canonical lane 結果が採用条件を
  満たした場合はこの記録のままとする。失敗が残る場合は PR/evidence に失敗分類を
  正直に追記する。

## 実行しない gate

- `mise run test-recording-real-input`: 実 audio device。NotRun。
- `mise run test-recording-virtual-endpoint`: provisioned endpoint 環境。NotRun。
- 実 GUI、実 OS Window、実 Plugin Editor/Native Dock、物理 Audio/MIDI 機材、
  市販 Plugin: NotRun。
- ruviform #219/#294 cross-consumer tuple、AudioLens #127 側検証、ruviform CI 収集:
  NotRun / 後続 slice。
- merge 後 main SHA に対する 3OS CI / Daily Full CI: 本 slice のローカル検証ではなく、
  Coordinator による統合検証。

## 検証 lane 記録（Windows 11、canonical = mise）

| lane | exit | 結果 / 備考 |
| --- | ---: | --- |
| `mise run sync-ui-deps` | 0 | Pass。`ruviform ui/* already synced @ 87a7ba8ff3` |
| `mise run check-ruviform-rev` | 0 | Pass。`resolved 87a7ba8f… (36 lockfile sources, lockfile-primary, REV match)` |
| `mise run test-tim03-l1` | 0 | Pass。CLAP/VST3 L1 oracle 各 1、計 2 passed。runner artifact を build |
| `mise run check-headless-deps` | 0 | Pass。`no GUI framework deps in 12 headless crates` |
| `mise run check-ui` | 0 | Pass。vendor check + `tsc --noEmit` |
| `mise run test-ui` | 0 | Pass。vitest 8 files / 73 tests passed |
| `mise run build-ui` | 0 | Pass。vite build + manifest 生成 |
| `mise run test-rt` | 0 | Pass（最終 tree で再実行） |
| `mise run check-app-native` | 0 | Pass（最終 tree で再実行） |
| `mise run lint-app-native` | 0 | Pass（最終 tree で再実行） |
| `mise run build-app-native` | 0 | Pass。test-only oracle 適応は native bin 入力外 |
| `mise run check-app-gpui` | 0 | Pass（最終 tree で再実行） |
| `mise run lint-app-gpui` | 0 | Pass（最終 tree で再実行） |
| `mise run test-app-gpui` | 0 | Pass（最終 tree で再実行。oracle 修正後 2/2 通過を含む） |
| `mise run lint-recording-real-input` | 0 | Pass。compile/lint lane で実 device 実行なし |
| `mise exec -- prek run --all-files` | 0 | Pass（docs 追加後に最終再実行する） |
| `mise exec -- cargo test --locked -p synvelis-app --test tim03_real_abi_process_oracle -- --test-threads=1` | 0 | 補助証拠（canonical = mise）。real ABI oracle 2 passed |

`mise run verify-full` は 3 段階で切り分けた。初回 exit 101 は L1 runner
artifact 未 build（oracle の指示どおり `test-tim03-l1` が必要）、2 回目 exit 101 は
real ABI oracle の旧 `Reported([])` 前提（本 slice で修正）、3 回目 exit 101 は
並行 feature build が残した複数 hash の test plugin DLL による artifact uniqueness
fail。DLL は `cargo clean -p ruviform-plugin-test-vst3 -p ruviform-plugin-test-clap`
で生成物のみ除去し、最終 `mise run verify-full` は exit 0 / Pass だった。`verify-full` 内訳は fmt-check・check-full・lint-full・test-full すべて exit 0。
