# Evidence — synvelis#127 TIM-03 instrument direct L1 musical transport oracle

slice `tim03-instrument-transport-oracle-s1`（Refs #127。Issue close ではない）。
ruviform#630 の正規 ABI `test_instrument` VST3/CLAP fixture を Synvelis 所有の
test-only L1 runner 経由で scan/instantiate/activate/start-processing し、direct L1
endpoint に caller supplied `TransportInfo` を与えて、plugin-side musical transport
click law を independent PCM oracle で検証した。

正本ドキュメント:

- `docs/specs/tim-03-instrument-transport-oracle.md`
- `docs/designs/tim-03-instrument-transport-oracle.md`
- `docs/plans/tim-03-instrument-transport-oracle.md`
- `docs/tasks/tim-03-instrument-transport-oracle.md`
- `docs/test-specs/tim-03-instrument-transport-oracle.md`
- `docs/impl/tim-03-instrument-transport-oracle.md`

machine-readable 実測値: `evidence/sv127/instrument-transport-oracle.jsonl`

## Anchor — 対象 SHA・環境

- branch: `feat/127-tim03-instrument-transport-oracle`
- tested HEAD recorded by the oracle JSON: `9889b2ea0ba935476858efab448f22cd247a2483`
  （implementation commit。evidence/doc-only commit はこの検証後に追加する）
- base `origin/main`: `2e32d871da081463e04571fa38c50d7170843dec`
  （最終検証前に `git fetch origin main` し、main 側の新規 commit がないことを確認）
- OS/Arch: Windows x86_64（Orca worktree
  `G:\orca\workspaces\synvelis\sv127-instrument-transport`）
- toolchain: rustc 1.99.1（mise 管理）
- ruviform resolved revision: `0479fa62e77e669a7bf84320d3d5418ffb949fc6`
  （`Cargo.lock` pin。pin 更新なし）

## 実測 artifact と verdict

| format | fixture SHA-256 | runner SHA-256 | lifecycle | verdict |
| --- | --- | --- | --- | --- |
| CLAP | `02a9c2d6c60a160a70e47c6f0e54b0e9fa2d26af1a79aa0ee9501aaf401847f7` | `7d31d45cc6479bc6a7eae3059f42a209cb563eea98763d569260a14d3d854b72` | scan → instantiate → initialize → activate → start-processing → endpoint prepare/drop → stop/deactivate/destroy | Pass（direct L1 endpoint oracle） |
| VST3 | `815f36bb1aa147ce6ed3831b31dae667844db5a2744183d957fcc1af598f0df5` | `9dec1b704299fa6846eb590f5f1ef90b257ed31059a0cdb8e3a4864b703e4e85` | scan → instantiate → initialize → activate → start-processing → endpoint prepare/drop → stop/deactivate/destroy | Pass（direct L1 endpoint oracle） |

runner は `mise run test-tim03-instrument-transport` が build する Synvelis 所有
test-only artifact。fixture/runner 解決と sibling staging は PR #668/#672/#683 の
contract を再利用し、staged fixture SHA-256 は resolved fixture SHA-256 と一致した。
JSONL の fixture path は cargo build が生成した hashed deps artifact、staged fixture path は
runner sibling に staging した canonical filename artifactであり、両者の SHA-256 が一致した。

## click law と判定律

独立計算式は fixture law:

```text
ceil((boundary_tick - position_ticks) * sample_rate * 60000 / (bpm_milli * ppqn))
```

ここで sample rate は 48 kHz、PPQN は 960、block は 512 frames。expected click frame は
observation の後付けではなく上式から計算した。click impulse は amplitude bit-exact 0.5、
stereo identical、click frame 以外は IEEE silent。`-0.0` は IEEE 比較で silent とみなす
判定であり、stereo identical 判定は channel 間 bit pattern まで一致することを要求した。

| format | case | input musical transport | expected frame | matched fresh blocks | click blocks | silent blocks | verdict |
| --- | --- | --- | ---: | ---: | ---: | ---: | --- |
| CLAP | stopped negative | Stopped, 120 BPM, tick 956 | none | 12 | 0 | 12 | Pass |
| CLAP | paused negative | Paused, 120 BPM, tick 956 | none | 10 | 0 | 10 | Pass |
| CLAP | missing musical negative | Playing, tick media position only | none | 12 | 0 | 12 | Pass |
| CLAP | 120 BPM base click | Playing, 120 BPM, tick 956 | 100 | 11 | 11 | 0 | Pass |
| CLAP | seek/position jump | Playing, 120 BPM, tick 940 | 500 | 10 | 10 | 0 | Pass |
| CLAP | active loop range | Playing, 120 BPM, tick 950, loop `[0,48000)` frames | 250 | 10 | 10 | 0 | Pass |
| CLAP | non-120 tempo | Playing, 150 BPM, tick 950 | 200 | 8 | 8 | 0 | Pass |
| VST3 | stopped negative | Stopped, 120 BPM, tick 956 | none | 9 | 0 | 9 | Pass |
| VST3 | paused negative | Paused, 120 BPM, tick 956 | none | 10 | 0 | 10 | Pass |
| VST3 | missing musical negative | Playing, tick media position only | none | 11 | 0 | 11 | Pass |
| VST3 | 120 BPM base click | Playing, 120 BPM, tick 956 | 100 | 9 | 9 | 0 | Pass |
| VST3 | seek/position jump | Playing, 120 BPM, tick 940 | 500 | 11 | 11 | 0 | Pass |
| VST3 | active loop range | Playing, 120 BPM, tick 950, loop `[0,48000)` frames | 250 | 9 | 9 | 0 | Pass |
| VST3 | non-120 tempo | Playing, 150 BPM, tick 950 | 200 | 9 | 9 | 0 | Pass |

click case では全 matched fresh block で観測 click frame が expected frame と一致し、
amplitude 0.5、stereo identical、non-click silence を満たした。negative control では全
matched fresh block が silent だった。seek/position jump case は click@100 phase の後に
click@500 を供給し、前 block state の click@100 を引きずらず block-start snapshot から
再導出された。active loop range でも per-block derivation は乱れなかった。150 BPM case は
120 BPM と異なる frame 200 を独立計算どおり示し、tempo依存性を確認した。

## latency / freshness の読み方

L1 endpoint latency は両 format で 1024 frames（512 frames/block × lookahead 2 blocks）。
output call `c` が fresh plugin evidence のとき、その source submitted block は
`c - 2` と attribution した。pass-through/substituted output は以前の output の replay で
あり fresh click evidence に数えない。初期 lookahead 内で matched output として数えられない
blockは今回 0 件だった。CLAP では 39 blocks、VST3 では 44 blocks を substituted/pass-through
として除外した。submit queue drop counter の増加は CLAP 19 calls、VST3 21 calls として
記録したが、内容 assertion は matched fresh output のみに適用した。各 case 最低 8 fresh
blocksを観測し、upstream reference oracle と同じく「少数の matched block を後付けで拾う」
判定ではない。

## upstream handoff gap と honesty boundary

resolved ruviform source `crates/ruviform-audio-plugin-bridge/src/node.rs:657` は
`call_transport_info(&span.ctx, rel, None, None)` を呼び、production graph node 経路で
tempo map/PPQN を plugin process へ渡していない。upstream tracker は
`https://github.com/scarecrow173/ruviform/issues/1040`
（`[不具合修正/plugin-bridge] PluginNode が musical transport を plugin process へ handoff できない`、OPEN）を確認した。

したがって production composition → `PluginNode` → plugin end-to-end musical transport は
この slice で **NotRun/Blocked upstream** であり、Pass と記録しない。今回 Pass したのは
Synvelis 所有 direct L1 endpoint API に explicit `TransportInfo` を供給する consumer-side
oracleだけである。`musical=None` negative control は production graph の欠陥を合格扱い
するものではなく、fixture が musical timing を捏造しないことと upstream handoff 欠落時に
audio oracle が false positive にならないことの証拠である。Synvelis 側 production code や
ruviform pin に恒久 workaround は追加していない。

## evidence matrix

| lane / scope | verdict | evidence |
| --- | --- | --- |
| VST3 real ABI fixture discovery/lifecycle | Pass | `test_instrument` scan visible、lifecycle 全 pass |
| CLAP real ABI fixture discovery/lifecycle | Pass | `test_instrument` scan visible、lifecycle 全 pass |
| direct L1 endpoint caller supplied `TransportInfo` | Pass | click@100/500/250/200、negative controls、両 format exit 0 |
| stopped/paused negative controls | Pass | matched fresh blocks は全 silent |
| missing musical negative control | Pass | Playing でも musical なしなら全 silent |
| seek/position jump freshness | Pass | click@100 の後 click@500、stale click なし |
| active loop range | Pass | click@250、per-block derivation 維持 |
| non-120 BPM | Pass | 150 BPM click@200 |
| pass-through/substituted exclusion | Pass | CLAP 39、VST3 44 blocks を fresh evidence から除外 |
| production composition → `PluginNode` → plugin musical end-to-end | NotRun/Blocked upstream | ruviform issue 1040、`node.rs:657` で tempo map/PPQN non-handoff |
| dynamic tempo edit / full frame-boundary matrix | NotRun | 後続 slice |
| fault/state isolation (`test_fault`/`test_stateful`) | NotRun | 後続 slice |
| GUI/Native Dock/EXT-L3/provisioned endpoint | NotRun | 本 slice 範囲外 |
| macOS/Linux runtime / 3OS CI | NotRun | 本 slice は Windows local。merge 後 CI で追跡 |
| AudioLens cross-consumer | NotRun | 本 slice 範囲外 |

## 検証コマンドと結果（canonical = mise）

| コマンド | exit | 結果 |
| --- | ---: | --- |
| `mise run fmt-check` | 1 | Infrastructure failure（`cargo fmt --all -- --check` が rustfmt 起動前に `ファイル名または拡張子が長すぎます。 (os error 206)`。後述） |
| `mise run check synvelis-app` | 0 | Pass |
| `mise run lint synvelis-app` | 0 | Pass（warnings denied） |
| `mise run test-tim03-instrument-transport` | 0 | Pass（2 passed / 0 failed。VST3・CLAP oracle JSON 出力） |
| `mise run test-tim03-l1` | 0 | Pass（2 passed / 0 failed） |
| `mise run test-tim03-transport` | 0 | Pass（2 passed / 0 failed） |
| `mise run test synvelis-app` | 0 | Pass |
| `mise run test synvelis-player` | 0 | Pass |
| `mise run test-rt` | 0 | Pass |
| `mise run check-headless-deps` | 0 | Pass（12 headless crates に GUI framework dependency なし） |
| `mise exec -- prek run --all-files` | 0 | Pass |

command/exit code の machine-readable summary は
`evidence/sv127/instrument-transport-oracle/canonical-results.tsv` に記録した。

補助証拠（canonical mise task ではない）: `mise exec -- cargo fmt -p synvelis-app -- --check`
は exit 0。変更した package の Rust formatting 自体は pass している。

### `fmt-check` infrastructure note

`mise run fmt-check` はこの Orca worktreeで rustfmt 起動前に os error 206を返した。
エラーは rustfmt の差分ではなく Windows/cargo fmt の path 列挙段階で発生しており、
検証成功条件は弱めていない。workspace-wide `fmt-check` は out-of-scope infrastructure
failure として記録し、package-scoped rustfmt check を補助証拠として併記する。

ローカル検証は変更範囲の検証であり、統合検証は merge 後の対象 SHA に対する必須 CI
証拠として別途確認する必要がある。

## pstack Verification

- Runtime Verification: NotRun（pstack project verify skill としての GUI runtime drive は
  未実施）。この slice は headless direct L1 endpoint oracle が対象で、GUI/Native Dock は
  scope 外。代替の canonical runtime evidence は `mise run test-tim03-instrument-transport`
  が実 VST3/CLAP fixture binary・実 runner child process・L1 wire・caller supplied
  transport snapshot・independent PCM oracle を駆動した exit 0 の結果である。
- Formal Verification: NotRun。formal verification 対象の wire codec / authorization
  boundary / concurrency model を変更していないため。代替の canonical 検証は
  `check`/`lint`/`test`/`test-rt` と integration oracle であり、formal proof を主張しない。

## 残件

- docs index（`docs/**/README.md`）は Worker 契約どおり未編集。Coordinator が統合時に整備する。
- workspace-wide `fmt-check` の os error 206 は Orca worktree パス長に依存する infrastructure
  failure として残る。コード側の formatting は package-scoped rustfmt check で pass。
- production graph end-to-end musical transport は upstream ruviform issue 1040 の解消後に
  後続 slice で検証する。
