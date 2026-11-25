# sv653 — RCD-05 RT monitor node bounded first slice 検証記録

`scarecrow173/synvelis#653` の bounded first slice（RT-safe monitor node
primitive + bounded typed-graph fixture）の検証記録。本 slice は production
graph への接続・実 OS audio backend・可聴 monitoring の製品受入を行わない。
headless typed-graph fixture 上の Pass を実 device / 実 OS / 可聴 monitoring
の Pass に昇格しない。可聴 monitoring の製品受入は `scarecrow173/synvelis#114`
系および `scarecrow173/ruviform#629` 仮想 endpoint 経路の受入側 gate で
追跡される。

- Date: 2026-11-25 (UTC)
- OS/Arch: Windows 11 x86_64（`MINGW64_NT-10.0-26300`、MSYS2 環境）
- Rust: `rustc 1.99.1 (58cf8f9c3 2026-10-13)`（msrv lane は mise 管理の
  `rust@1.89.0`）
- Base SHA: `7f15b5bc7f2ba3376dc7b050e5f3de3b6d0b613e`（`origin/main`。
  着手前に `git fetch origin` で確認。branch はこの SHA から派生）
- Branch: `scarecrow173/sv653-rt-monitor-node`
- ruviform pin: `20dc41f7`（`Cargo.lock` 解決 revision。本 slice は
  ruviform 側変更なし）
- ローカル検証 = 変更範囲の検証。統合検証は merge 後の対象 SHA に対する
  必須 CI 結果が証拠であり、本記録はそれを代替しない。

## 検証 lane 一覧（canonical = mise）

| Command | OS | Exit | Verdict | 備考 |
| --- | --- | --- | --- | --- |
| `mise run fmt-check` | Windows x86_64 | 1 | Fail（環境不能） | `cargo fmt --all` が Windows コマンド長制限 `os error 206` で起動不能。差分検査不能であり未実施 Pass にはしない。補助証拠で代替（下行） |
| `mise exec -- cargo fmt --package synvelis-player -- --check` | Windows x86_64 | 0 | Pass（補助証拠・canonical = mise） | 変更対象 package の rustfmt 差分なし。空ログ = 差分なし（`fmt-check-package-aux.txt` / `.meta`） |
| `mise run check synvelis-player` | Windows x86_64 | 0 | Pass | `lane-logs/check-synvelis-player.{txt,meta}` |
| `mise run lint synvelis-player` | Windows x86_64 | 0 | Pass | clippy 警告なし |
| `mise run test synvelis-player` | Windows x86_64 | 0 | Pass | `monitor_rt_node` 9/9 pass + 独立 RT binary `rt_monitor_node_process_hot_path_alloc_free` pass |
| `mise run check-full` | Windows x86_64 | 0 | Pass | workspace 全体 |
| `mise run lint-full` | Windows x86_64 | 0 | Pass | workspace 全体 |
| `mise run test-full` | Windows x86_64 | 0 | Pass | workspace 全 test binary pass |
| `mise run test-rt`（初回） | Windows x86_64 | 101 | Fail（lease 外 flaky・後述） | lease 外の `synvelis-app` test `provider_switch_scopes_import_surface` が `recoverable`/`stale-revision` 不一致で fail。`lane-logs/test-rt.{txt,meta}` |
| `mise run test-rt`（再実行） | Windows x86_64 | 0 | Pass | workspace 全 rt_ test pass。`rt_monitor_node_process_hot_path_alloc_free ... ok` を test 名で確認（`lane-logs/test-rt-rerun.{txt,meta}`） |
| `mise run test-msrv` | Windows x86_64 | 0 | Pass | rust@1.89.0 で workspace 全 test pass（`lane-logs/test-msrv.{txt,meta}`） |
| `mise exec -- prek run --all-files` | Windows x86_64 | 0 | Pass | trailing-whitespace / end-of-file / yaml / toml / mixed-line-ending / BOM / private-key / actionlint 全 pass |

## slice 受入条件ごとの記録

1. **RT-safe primitive** — Pass（headless typed-graph 範囲）。
   `MonitorRtNode` が `ruviform_audio::typed::GraphProcessor` として
   playback input / monitor input / monitor output の port contract で
   typed graph に compile され、`RtRuntime::process` の RT callback
   contract section で PCM mixing が駆動されることを
   `crates/synvelis-player/tests/monitor_rt_node.rs` の fixture で確認
   （9/9 pass）。実 OS audio callback への接続は本 slice 対象外（NotRun。
   後述）。
2. **route 判定の単一権威** — Pass（構造）。RT node は
   `synvelis-recording::decide` / `MonitorProbe` / gate/transport/arm/
   monitor/mute/solo 判定を持たず、非 RT 側が構築した bounded block
   schedule（`MonitorRtSchedule` の tile 列）を適用するだけである。
   `monitor_rt.rs` に route 導出ロジックが存在しない（設計文書
   `docs/designs/rcd-05-realtime-monitor-node-design.md` 参照）。
3. **sample-accurate bounded schedule** — Pass。固定容量・block 内
   半開区間 `[start, end)` の連続・非重複・空区間なし tile を constructor
   と push 時に検証し、未供給・順序違反・gap/overlap・範囲外・容量超過で
   fail-closed mute + atomic fault bit を立てることを
   `monitor_rt_node_schedule_contract_violations_fail_closed` /
   `monitor_rt_node_control_overflow_mutes_next_observed_block` /
   `monitor_rt_node_constructor_rejects_invalid_config` で確認。
   `NodeEvent` は sample-accurate 境界に使っていない（block 先頭配送で
   frame offset を持たないため。設計文書に記録）。
4. **input missing の正直な区別** — Pass。monitor input edge は
   direct edge（`channel_map: None`・`EdgeDelay::Zero`・single incoming
   edge）契約で、compile 後の graph 構造で scratch materialization が
   ないことを `monitor_rt_node_compiled_monitor_input_is_a_direct_edge`
   で確認。未供給と供給された無音 PCM の区別を
   `monitor_rt_node_distinguishes_supplied_silence_from_missing_input`
   で、route Input で使われない missing input を fault としないことを
   `monitor_rt_node_unused_missing_monitor_input_is_not_input_missing`
   で確認。
5. **passthrough と非干渉** — Pass（fixture 範囲）。Playback 安定区間の
   bit-exact passthrough（`-0.0` 保持を含む `==` 照合）と、monitor node
   有無で非 monitor path PCM が不変であることを
   `monitor_rt_node_playback_passthrough_preserves_bits_and_nonmonitor_path`
   で確認。raw capture / offline export には接続していない（production
   graph 未接続のため structural に非干渉。設計文書に記録）。
6. **block 中途境界** — Pass。block 途中の Input/Playback/Muted 境界で
   出力 frame 数が block 長と一致し gap/overlap がないことを、独立 oracle
   （node 実装も `MonitorAdapter::process_block` も呼ばず schedule と
   input/playback PCM から frame ごとに期待値を計算する test 内参照
   実装）との照合で
   `monitor_rt_node_mid_block_boundaries_match_independent_oracle` が
   確認。
7. **gain ramp の責務限定** — Pass。ramp は DSP のみで native
   placement・latency/PDC 補正・リサンプル・frame offset 補正を行わない。
   `monitor.rs` は変更せず、RT node 内に fixed-scratch の DSP を実装した
   （重複の理由と後続統合候補は設計文書に記録）。`MonitorAdapter` の
   Vec 出力 pattern は持ち込んでいない。
8. **RT 安全性の根拠** — Pass（文書）。allocation-free・deallocation-free・
   no blocking lock・no OS I/O・bounded work・drop discipline・atomics
   ordering（RT 側 `Relaxed` の理由）・control channel capacity/overflow
   policy・direct edge 契約・`NodeEvent` を使わない理由・generation
   rebuild での ramp state reset click リスク（後続 transport-control
   slice へ deferred）を
   `docs/designs/rcd-05-realtime-monitor-node-design.md` に記録。
9. **RT 実測** — Pass。独立 test binary
   `crates/synvelis-player/tests/monitor_rt_rt.rs` で `stats_alloc` により
   warm-up 後の `RtRuntime::process` hot path（schedule 適用と PCM mixing
   を含む）の allocation/deallocation 0 を実測し、測定中に monitor 出力が
   実際に生成された behavioural evidence（status・出力 sample）を残す。
   `rt_monitor_node_process_hot_path_alloc_free` pass。
10. **正規検証** — 上記 lane 表のとおり。`test-rt` の初回失敗は lease 外
    test の flaky 疑いであり再実行結果を記録する（後述）。Linux/macOS
    依存 lane は Windows ローカルで偽装せず、branch への push 後に
    `ci.yml` workflow_dispatch（branch CI 反復方式）で確認し、結果は
    PR 本文に記録する。
11. **製品受入 gate の追跡** — Pass（記録）。可聴 monitoring の製品受入は
    本 slice では実施しない旨を spec・design・本 evidence・PR 本文に
    明記し、`scarecrow173/synvelis#114` 系・`scarecrow173/ruviform#629`
    仮想 endpoint 経路で追跡されることを記録した。offline render
    support は検証していないため `Unknown` のまま。

## NotRun / Unsupported

| 項目 | Verdict | 理由と後続 |
| --- | --- | --- |
| 実 device input・実 OS audio backend（WASAPI/cpal）経由の monitoring | NotRun | 本 slice 対象外。後続 EXT-L3 / 製品受入 gate（sv#114 系） |
| 可聴 monitoring の製品受入（実機での確認） | NotRun | 同上。headless Pass を昇格しない |
| 仮想 endpoint 経路（rf#629）での end-to-end 受入 | NotRun | 受入側 gate で追跡 |
| production graph への搭載・`synvelis-app` 接続 | NotRun | 後続 integration slice。primitive のみ |
| GUI/GPUI での確認 | NotRun | headless primitive slice のため対象外 |
| Linux/macOS ローカル検証 | Unsupported（Windows 環境） | branch CI 反復方式で確認し PR 本文に記録 |
| offline render（sequential offline での PCM 一致） | NotRun | `offline_support` は `Unknown` のまま宣言 |

## test-rt 初回失敗の調査（lease 外 flaky）

`mise run test-rt`（`cargo test --workspace --locked rt_`）の初回実行で
`synvelis-app` の `provider_switch_scopes_import_surface` が
`left: "recoverable" / right: "stale-revision"` で fail した（exit 101、
`lane-logs/test-rt.{txt,meta}`）。この test は `rt_` filter に
`import_` 内の部分文字列 `rt_` が一致して workspace lane に巻き込まれる
lease 外の既存 test であり、本 slice の変更（synvelis-player への新規
module 追加のみで既存コード路径の変更なし）は synvelis-app の挙動に
影響し得ない。切り分けとして同 test を単独で 3 回・test binary 全体を 3 回再実行し
全て pass（補助証拠）したうえで、`mise run test-rt` を再実行したところ
exit 0 で workspace 全 rt_ test（当該 `provider_switch_scopes_import_surface`
を含む）が pass した。初回失敗は fail を隠さず上表に残し、再実行の pass と
切り分け経緯を併記する。タイミング依存の flaky と判断し、lease 外のため
本 slice では修正しない（main CI は直近 dispatch で success）。

## fmt-check の環境不能（os error 206）

`mise run fmt-check` は `cargo fmt --all -- --check` を実行するが、本
Windows 環境では workspace 全 package のファイル列がコマンド長制限
`os error 206`（「ファイル名または拡張子が長すぎます。」）で rustfmt
起動不能となり exit 1。これは環境不能であって整形差分の検出ではない。
sv643 等の先例どおり canonical の失敗を隠さず記録し、補助証拠として
`mise exec -- cargo fmt --package synvelis-player -- --check`（exit 0、
差分なし）を併記する。なお commit 時の prek hook（trailing-whitespace /
end-of-file 等）と rustfmt は `mise run fmt` で適用済み。

## docs index

`docs/**/README.md` の index は編集・再生成していない（共通契約どおり
Coordinator が統合時に整備）。index 未同期は許容済み。
