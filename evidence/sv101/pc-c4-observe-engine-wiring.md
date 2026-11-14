# PC-C4 observe engine wiring verification results (Issue #101 / synvelis#101)

OS: Windows 11 (worktree `sv101-observewire`)。toolchain は
`mise.toml` 管理（Rust stable、edition 2024、CI pin 1.99.0）。
canonical = `mise run` 経由。cargo 直接実行は補助証拠。

slice: `observe_block` の engine-driver 配線
（`docs/specs/pc-c4-observe-engine-wiring.md`）。

## 実行結果

| コマンド | OS | toolchain | exit | 結果 |
| --- | --- | --- | --- | --- |
| `mise run fmt-check` | Windows | mise rust stable | 0 | Passed（`cargo fmt --all -- --check`） |
| `mise run check synvelis-app` | Windows | 同上 | 0 | Passed（`--locked --all-targets`） |
| `mise run check synvelis-player` | Windows | 同上 | 0 | Passed（同上。`routing_engine.rs` 変更のため必須 lane） |
| `mise run lint synvelis-app` | Windows | 同上 | 0 | Passed（clippy `-D warnings`。`must_use`/`single_match_else`/`float_cmp`/`semicolon_if_nothing_returned` 指摘を修正済み） |
| `mise run lint synvelis-player` | Windows | 同上 | 0 | Passed（同上） |
| `mise run test synvelis-app` | Windows | 同上 | 0 | Passed（全 test target green。`pc_c4_observe_engine_wiring` 4 test・更新後 `pc_c4_06_control_op_surface` 4 test を含む） |
| `mise run test synvelis-player` | Windows | 同上 | 0 | Passed（全 test target green。`pc_c4_07_*` 非回帰を含む） |

## 補助証拠

- `cargo test --locked -p synvelis-app --test
  pc_c4_observe_engine_wiring`: 4/4 pass
  （`run_buffers_one_cycle_per_block`・
  `engine_observations_feed_reported_to_workspace`・
  `ui_pump_drives_observation_feed`・
  `status_observe_section_tracks_node_cursor_domain`）。
- `cargo test --locked -p synvelis-app --test
  pc_c4_06_control_op_surface`: 4/4 pass（receipt assert を
  drain-once 意味へ更新後）。

## 実装時に判明した意味変化（記録）

- scheduler receipt は drain-once: `SessionRoutingEngine::run` の
  engine-driven `observe_block` が block 毎に node report channel
  を消費するため、明示 `syn.control.observe` の `receipts` は
  自身の call が drain した分のみを返す（通常は空）。applied
  plan generation の証拠は `control_drain_observations` の
  buffered cycle に残る。`pc_c4_06` test と op doc に明記
  （exp `docs/impl/exp/pc-c4-observe-engine-wiring.md` EXP-2）。
- `observe_block` の `call_start` は engine `cursor()` ではなく
  node cursor domain（attach/generation-anchored、generation
  切替で 0 restart、epoch `ClockEpochId(0)`）— engine 側で
  post-process `active_generation` を読んで head を追跡
  （同 exp EXP-1）。

## 未検証（NotRun/Unsupported）

- `*-full`・`*-msrv`・Feature Matrix・3OS 横断・Golden/長時間
  回帰: 日常検証の対象外（Daily/Manual Full CI の責務）。
- 実機 audio device・実 plugin (VST3/AU/CLAP)・実 OS backend・
  plugin GUI・AudioLens f64・wire-schema/L1・oracle（synvelis#103）:
  `external-gates.md` 参照。headless fixture の成功を実機 Pass に
  しない。
- `seek_observed` の app/UI 配線: `SessionRoutingEngine` は現状
  `run` のみ駆動。app 層に seek 経路が無いため opt-in API として
  残置（IR Follow-ups 参照）。
