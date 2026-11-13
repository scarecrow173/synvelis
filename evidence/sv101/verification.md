# PC-C4 plugin binding verification results (Issue #101 / synvelis#101)

OS: Windows 11 (worktree `sv101-plugin-binding`)。toolchain は
`mise.toml` 管理（Rust stable、edition 2024、CI pin 1.99.0）。
canonical = `mise run` 経由。cargo 直接実行は補助証拠。

## 実行結果

| コマンド | OS | toolchain | exit | 結果 |
| --- | --- | --- | --- | --- |
| `mise run fmt-check` | Windows | mise rust stable | 0 | Passed（`cargo fmt --all -- --check`） |
| `mise run check synvelis-project` | Windows | 同上 | 0 | Passed |
| `mise run check synvelis-player` | Windows | 同上 | 0 | Passed |
| `mise run lint synvelis-project` | Windows | 同上 | 0 | Passed（clippy `-D warnings`） |
| `mise run lint synvelis-player` | Windows | 同上 | 0 | Passed（同上。初回 `doc_markdown`/`too_many_lines` 指摘を修正済み） |
| `mise run test synvelis-project` | Windows | 同上 | 0 | Passed（全 test target green。`plugin_parameter_instance_binding_resolves_persisted_instance` 含む） |
| `mise run test synvelis-player` | Windows | 同上 | 0 | Passed（全 test target green。`pc_c4_02_instance_authority_bind_gate`・`rt_pc_c4_session_bound_dispatch_alloc_free` 含む） |
| `mise run test-rt` | Windows | 同上 | 0 | Passed（`cargo test --workspace --locked rt_` — 222 suite 全 ok、RT no-alloc/no-block 契約） |

## 補助証拠

- `cargo test -p synvelis-player --test pc_c4_binding --test
  pc_c6_oracle_pl`（mise test の内訳確認として個別実行）:
  pc_c4_binding 8 tests・pc_c6_oracle_pl 8 tests 全 pass。
- `cargo test -p synvelis-player --test pc_c4_binding_rt`:
  `rt_pc_c4_session_bound_dispatch_alloc_free` pass（stats_alloc
  で measured window の allocations/deallocations = 0）。

## 未検証（NotRun/Unsupported）

- `*-full`・`*-msrv`・Feature Matrix・3OS 横断・Golden/長時間
  回帰: 日常検証の対象外（Daily/Manual Full CI の責務）。
- 実機 audio device・実 plugin (VST3/AU/CLAP)・実 OS backend・
  plugin GUI・AudioLens f64・wire-schema/L1・oracle（synvelis#103）:
  `external-gates.md` 参照。headless fixture の成功を実機 Pass に
  しない。
