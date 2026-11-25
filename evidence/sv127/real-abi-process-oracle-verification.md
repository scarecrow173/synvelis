# Evidence — synvelis#127 TIM-03 real ABI process oracle slice

Issue #127（TIM-03: Plugin Process境界／Loop・Tempo・Automation精度
Capability/Degraded Policy）の real ABI process oracle 前進 slice。本 slice は
ruviform#630 の実 ABI test plugin を使い、artifact discovery・descriptor 列挙・
lifecycle・production fail-closed 境界を headless で実測した。PCM/Event process
oracle 本体は、in-process endpoint が upstream 設計上利用できず、synvelis に
L1 out-of-process hosting 配線がまだないため `NotRun` である。

正本 spec/design/test-spec:

- `docs/specs/tim-03-real-abi-process-oracle.md`
- `docs/designs/tim-03-real-abi-process-oracle.md`
- `docs/test-specs/tim-03-real-abi-process-oracle.md`

machine-readable 実測値: `evidence/sv127/real-abi-process-oracle.jsonl`

## Anchor — 対象 SHA・環境

- branch: `scarecrow173/sv127-tim03-real-abi`（worktree 名。task spec の
  推奨名は `feat/127-tim03-real-abi-oracle` だったが、dispatch 済み branch を採用）
- base/HEAD before commit: `af159f30eef33a74220a2c45614deb7d4e6278d5`
  （`origin/main` と同一）
- OS/Arch: Windows x86_64（worktree
  `G:\orca\workspaces\synvelis\sv127-tim03-real-abi`）
- toolchain: rustc 1.99.1（mise 管理）
- ruviform resolved SHA: `20dc41f71c47674b3e51d1eb0921124b94f8bfaf`
  （`Cargo.lock` pin。ruviform#630 test plugin crate を含む）

## 実測 artifact と verdict

| format | artifact | sha256 | descriptor count | `test_gain` descriptor / factory class id | scan | lifecycle to processing | direct endpoint prepare | production adapter connect | production activation fault | verdict |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| VST3 | `target/debug/deps/ruviform_plugin_test_vst3-1ac2c390d2654dc3.dll` | `3755a7a800d12b48c64f9158624da707ecb7b627be6250f1945226ce1a572c41` | 7 | `218a4f6d7c3b059ef1a24c68b9d37e51` | Pass | Pass | `EndpointUnavailable` | `Incompatible` | `UnsupportedFormatOrTimeMap` | discovery/lifecycle/fail-closed evidence Pass; process oracle NotRun |
| CLAP | `target/debug/deps/ruviform_plugin_test_clap-b8085fbbc02de038.dll` | `94e12c43d05905ddfb15d0e60fbbb2ad1ebf08962e4b65a01daaef072799cfbd` | 6 | `org.ruviform-test.clap-test-gain` | Pass | Pass | `EndpointUnavailable` | `Incompatible` | `UnsupportedFormatOrTimeMap` | discovery/lifecycle/fail-closed evidence Pass; process oracle NotRun |

補助観測:

- 両 format で `create_sendable_available: false`。
- 両 format で host adapter が観測した bus report は `Reported([])`。
- VST3 descriptor id は upstream host contract に従う raw component TUID bytes の
  lowercase hex であり、textual GUID ではない。
- CLAP descriptor id は plugin factory instantiation id。
- `test_instrument` descriptor は両 format で discovery できたが、process oracle
  本体が endpoint 境界より先へ進めないため独立 event/click oracle は `NotRun`。

`tim03_real_abi_process_oracle.rs` の直接実行結果（補助証拠）:

```text
running 2 tests
test tim03_real_abi_vst3_discovery_lifecycle_and_fail_closed_boundary ... ok
test tim03_real_abi_clap_discovery_lifecycle_and_fail_closed_boundary ... ok
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

## なぜ process oracle を Pass にしないか

### upstream endpoint 境界

ruviform pin `20dc41f71c47674b3e51d1eb0921124b94f8bfaf` では、
`crates/ruviform-plugin-core/src/plugin.rs:398-417` の
`PluginFactory::create_sendable` default は `None` を返す。real VST3/CLAP bridge
factory は thread-affine/`!Send` object を作るため sendable instance を提供せず、
`crates/ruviform-plugin-test-vst3/tests/l1_real_test_plugin_e2e.rs:1316-1326`
（CLAP 側も `crates/ruviform-plugin-test-clap/tests/l1_real_test_plugin_e2e.rs:1281-1291`）
は in-process real plugin は attach できず、`prepare_endpoint` は L1 route でだけ
成功すると明記する。
今回の実測でも両 format で `create_sendable_available: false`、direct
`prepare_endpoint` は `EndpointUnavailable` であった。

これは test artifact の欠損ではなく、upstream が L1 out-of-process hosting を
typed real ABI 経路として使う設計境界である。

### production で先に立つ bus 境界

production composition では direct endpoint prepare より前に bus 検証が失敗する。
upstream `crates/ruviform-audio-plugin-bridge/src/attach.rs` の attach ordering は
`validate_buses`（line 118）→ `host.buses` 取得と reported table cross-check
（lines 123-125）→ endpoint prepare（lines 139-141）である。`crates/ruviform-plugin-host/src/host.rs`
では `BusReport::Reported` が empty table を「declared none」として保持し
（lines 158-178）、in-process Plain slot では `plugin.buses().to_vec()` を返す
（lines 1004-1014）。今回の real ABI
instance が返した report は両 format で `Reported([])` であり、synvelis factory の
宣言 `stereo_in/stereo_out` と upstream validator の non-empty main bus requirement
に合わないため、endpoint prepare 前に `Incompatible` で fail-closed する。

したがって、今回の production evidence は 2 層に分ける。

1. direct endpoint measurement: real ABI object が sendable endpoint を提供せず
   `EndpointUnavailable`。
2. production composition measurement: bus table validation が先に失敗し、
   adapter connect は `Incompatible`、production activation fault は
   `UnsupportedFormatOrTimeMap`。

`ProcessorConnectError::Incompatible` は deterministic に
`RoutingFault::UnsupportedFormatOrTimeMap` へ map される
（`crates/synvelis-player/src/routing_engine.rs`）。一方 `EndpointUnavailable` 系は
`PluginUnavailable` へ map されるが、現行 production path では bus validation が
それを mask する。test はこの 2 層を混同せず、direct measurement と production
measurement を別々に記録する。

### architecture gap

synvelis production の `compose_routing_engine` は in-process `ProductPlugin`
factory を `PluginHostProcessors::product` へ登録する経路であり、ruviform L1
out-of-process hosting を production composition へ接続する配線は現状ない。
このため、real ABI process oracle を in-process production composition で Pass に
することはできない。解除条件は **L1 out-of-process hosting が synvelis production
composition に配線された後に再評価する** ことである。

## independent oracle の結果

PCM/Event oracle 本体は `NotRun`。理由は process call に到達する前に production
composition が fail-closed し、観測 PCM/control observation を得られないためである。
したがって次の項目は real ABI では評価されていない。

- expected/observed PCM・crc 照合
- normalized/plain mapping（`plain = normalized * 2`）
- sample-accurate parameter/frame offset
- loop wrap/seek/tempo revision 跨ぎの実 plugin output/event timing
- illegal process length・誤配送・stale generation の real ABI における 0 件確認

これらを 0 件 Pass と記録しない。既存 Fake/headless lane は非回帰確認として
Pass だが、real ABI process oracle の代替にはしない。

## evidence matrix

| lane / scope | verdict | evidence |
| --- | --- | --- |
| Windows headless real ABI artifact discovery（VST3/CLAP） | Pass | scan Pass、descriptor count/hash/id を JSONL に記録 |
| Windows headless real ABI lifecycle to processing state | Pass | `lifecycle_to_processing: pass` |
| production composition fail-closed boundary | Pass（否定的境界の実測） | adapter `Incompatible`、activation fault `UnsupportedFormatOrTimeMap` |
| direct endpoint boundary | Pass（否定的境界の実測） | `EndpointUnavailable`、`create_sendable_available:false` |
| real ABI PCM/Event independent oracle | NotRun | L1 out-of-process hosting wiring 後に再評価 |
| `test_instrument` event/transport click oracle | NotRun | 同上。descriptor discovery のみ確認 |
| crash/hang/timeout isolation L1 | NotRun | 本 slice 範囲外 |
| `test_fault`/`test_stateful`/State/Unload 最終 EXT-L3 | NotRun | 本 slice 範囲外 |
| GUI/Native Dock/実 Window | NotRun | 本 slice 範囲外 |
| macOS/Linux | NotRun | 本 slice は Windows headless のみ |
| 3OS CI・AudioLens f64/native PCM cross-consumer | NotRun | 外部 gate |

## 検証コマンドと結果（canonical = mise）

| コマンド | 結果 |
| --- | --- |
| `mise run fmt-check` | Pass（exit 0） |
| `mise run check synvelis-app` | Pass（exit 0） |
| `mise run lint synvelis-app` | Pass（exit 0） |
| `mise run test synvelis-app` | Pass（exit 0。`tim03_real_abi_process_oracle` 2 test を含む全 test pass） |
| `mise run test synvelis-player` | Pass（exit 0。既存 TIM-03 boundary/session/loop wiring 非回帰） |
| `mise run test-rt` | Pass（exit 0。workspace `rt_` filter lane） |
| `mise run check-headless-deps` | Pass（exit 0。12 headless crates に GUI framework dependency なし） |
| `mise exec -- prek run --all-files` | Pass（exit 0。whitespace/EOL/YAML/TOML/line-ending/BOM/private-key/actionlint hooks 全 pass） |

記録面の follow-up commit（ruviform#1012 起票番号と AC5/AC6 字義の緊張関係の残件化）
に対する再検証: `mise run fmt-check` exit 0、`mise run test synvelis-app` exit 0
（59 test binaries / 471 passed / 0 failed、`tim03_real_abi_process_oracle` 2 tests 含む）、
`mise exec -- prek run --all-files` exit 0。変更は evidence 本文のみで test/コード差分は
ないが、指示どおり fmt-check と関連 test lane を再実行した。

補助証拠（canonical ではない）:

- `target/debug/deps/tim03_real_abi_process_oracle-6b3a7d8b3525175c.exe --nocapture` — exit 0、
  2 tests passed。JSONL 出力を `real-abi-process-oracle.jsonl` に抽出した。
- `sha256sum target/debug/deps/ruviform_plugin_test_*.dll` — 上表の hash と一致。

## 残件

- L1 out-of-process hosting を synvelis production composition に配線し、real ABI
  PCM/Event oracle を再評価する。
- upstream real ABI bridge から有効な bus layout が見えない問題を切り分ける。
  現状 `Reported([])` が endpoint 以前の `Incompatible` を引き起こし、production
  fault を `UnsupportedFormatOrTimeMap` にしている。ruviform 側 Issue として
  `scarecrow173/ruviform#1012` を起票済み（本 evidence の実測ログへの言及を含む）。
- Issue #127 受入条件の字義上の緊張関係: AC5 は実 CLAP/VST3/AU の L2/L3 を
  「現行必須でなく利用OS/Format確定時にだけ Activation する」と条件付きで書く一方、
  AC6（Windows GUI-L2/EXT-L3）は「2026-11-15現行 Required」と Windows の L2/L3 を
  事実上必須化しており、両者は矛盾し得る。本 slice はどちらの解釈でも Pass を
  主張しない（Windows L2/L3 は NotRun）が、字義の矛盾そのものは Issue 側で
  解消すべき残件として記録する。
- crash/hang/timeout isolation、`test_fault`、`test_stateful`、State/Unload、
  editor/GUI/Native Dock、Windows EXT-L3 最終実走。
- desktop product で実 plugin scan/registry を提供する owner 判断、
  `compose_product_routing` の空 registry、`composition.rs` stale comment。
- 3OS CI と AudioLens cross-consumer evidence の収束。
- docs index（`docs/**/README.md`）は生成物のため本 slice では更新せず、
  Coordinator が統合時に整備する。
