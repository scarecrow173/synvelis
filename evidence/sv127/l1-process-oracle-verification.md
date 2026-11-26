# Evidence — synvelis#127 TIM-03 L1 out-of-process production wiring + limited process oracle

slice `tim03-l1-oos-production-wiring-s1`（Refs #127。Issue close ではない）。
Synvelis production composition に typed な L1 route 登録 seam を追加し、rf#630
`test_gain` VST3/CLAP を out-of-process child 経由で起動して、初回の限定的 process
oracle を実測した。

正本ドキュメント:

- `docs/specs/tim-03-l1-out-of-process-hosting.md`
- `docs/designs/tim-03-l1-out-of-process-hosting.md`
- `docs/plans/tim-03-l1-out-of-process-hosting.md`
- `docs/tasks/tim-03-l1-out-of-process-hosting.md`
- `docs/test-specs/tim-03-l1-out-of-process-hosting.md`
- `docs/impl/tim-03-l1-out-of-process-hosting.md`

machine-readable 実測値: `evidence/sv127/l1-process-oracle.jsonl`

## Anchor — 対象 SHA・環境

- branch: `scarecrow173/sv127-tim03-l1`（Orca dispatch worktree の既存 branch。
  task spec の推奨名は `feat/127-tim03-l1-process-oracle`）
- base/HEAD before commit: `b9c331fd`（`origin/main` と同一。最終検証前に
  `git fetch origin main` し、`HEAD...origin/main` は `0 0`）
- OS/Arch: Windows x86_64（worktree `G:\orca\workspaces\synvelis\sv127-tim03-l1`）
- toolchain: mise 管理 Rust 1.99.x
- ruviform resolved SHA: `20dc41f71c47674b3e51d1eb0921124b94f8bfaf`
  （`Cargo.lock` pin。pin 更新なし）

## 実測 artifact と verdict

| format | artifact hash | descriptor count | `test_gain` descriptor | runner | production activation | endpoint oracle | verdict |
| --- | --- | --- | --- | --- | --- | --- | --- |
| CLAP | `114129c55a44176841b194441bcd66c79c1350d54fc42c4a8816b176565edb7a` | 6 | `org.ruviform-test.clap-test-gain` | `target/debug/synvelis-plugin-test-clap-l1-runner.exe` | `Active` generation 1、replacement generation 2 で retirement Pass | attach/detach Pass、15 matched / 7 substituted | initial re-evaluation complete, limited |
| VST3 | `f76cd12554edd747a7b58ab19b93c29d9601c44a27813a453f9f041d70fd4376` | 7 | `218a4f6d7c3b059ef1a24c68b9d37e51` | `target/debug/synvelis-plugin-test-vst3-l1-runner.exe` | `Active` generation 1、replacement generation 2 で retirement Pass | attach/detach Pass、15 matched / 7 substituted | initial re-evaluation complete, limited |

runner は `mise run test-tim03-l1` が先に build する Synvelis 所有 test-only artifact。
test は runner path を env override または target profile root から解決し、fixture
cdylib を runner sibling の un-hashed artifact として stage する。`ruviform-plugin-export`
は wire-v2 以降 argv を受け付けないため、runner 独自 argv は使わない。

## oracle 結果の解釈

- production side は `compose_routing_engine_with_out_of_process` を通り、project
  mutation → routing build → `RoutingActivationState::Active { generation: 1 }` に
  到達した。generation 1 に対応する plugin state handle も観測した。
- 同じ production session で `RemoveProcessor` を commit し、replacement generation 2
  の RT activation を観測した。activation 後、retired generation 1 の
  `plugin_state_handles` は空であり、production generation retirement が独立に
  観測できた。endpoint object 自体の detach は upstream API 上 ownership `Drop`
  であり detach receipt を返さないため、同じ instance の re-attach oracle としては
  断言しない。
- direct endpoint side は Scan → Instantiate → Initialize → Activate →
  StartProcessing → `prepare_endpoint` に成功し、endpoint drop 後に
  StopProcessing → Deactivate → Destroy が成功した。
- latency は両 format で `1024` frames（default lookahead 2 × 512 frames）。
  これは lookahead pipeline の latency であり、precision exact 証拠ではない。
- 両 format で bus report は `BusReport::Unavailable`。bus layout は unverified であり、
  in-process real ABI の `Reported([])` と同じ意味には扱っていない。
- normalized 0.25 の gain event は plain gain 0.5 に対応し、matched block の output は
  deterministic input 履歴に対する `0.5 * input` membership を bit-exact に満たした。
- `passthrough_submits` delta が 0 の block だけを matched delivery とし、substituted
  output は定義済み動作として記録したが delivery evidence に数えなかった。
  dropped submit delta は両 format で 0。

## evidence matrix

| lane / scope | verdict | evidence |
| --- | --- | --- |
| L1 route registration seam | Pass | `OutOfProcessSpec` を `PluginHostProcessors` / production composition から登録 |
| VST3 production activation | Pass | `Active` generation 1、state handle present |
| CLAP production activation | Pass | `Active` generation 1、state handle present |
| VST3 production generation retirement | Pass | `RemoveProcessor` → replacement generation 2 Active、retired generation 1 handles empty |
| CLAP production generation retirement | Pass | `RemoveProcessor` → replacement generation 2 Active、retired generation 1 handles empty |
| VST3 direct endpoint attach/detach | Pass | endpoint latency 1024、drop 後 teardown Pass |
| CLAP direct endpoint attach/detach | Pass | endpoint latency 1024、drop 後 teardown Pass |
| production endpoint object detach | NotRun/unobservable in current API | detach receipt なし。generation/application teardown の ownership `Drop` で行使されるが、same-instance re-attach oracle は teardown 後に成立しない |
| deterministic defined-output membership | Pass（limited） | VST3 15、CLAP 15 matched blocks。bit-exact membership |
| bus layout verification | NotRun/unverified | L1 `BusReport::Unavailable` |
| tempo/loop/automation sample accuracy | NotRun | 後続 slice |
| MIDI/transport precision (`test_instrument`) | NotRun | 後続 slice |
| crash/hang/fault isolation (`test_fault`) | NotRun | 後続 slice |
| state quality (`test_stateful`) | NotRun | sv#128/TIM-04 側境界 |
| GUI/Native Dock/実 Window | NotRun | 本 slice 範囲外 |
| Windows EXT-L3 最終受入 | NotRun | crash/hang isolation 等が残る |
| macOS/Linux 実走 | NotRun | 本 slice は Windows local のみ |
| 3OS CI・AudioLens cross-consumer | NotRun | 外部 gate |

## 検証コマンドと結果（canonical = mise）

すべて Windows x86_64 local、exit code 0。

| コマンド | 結果 |
| --- | --- |
| `mise run fmt-check` | Pass |
| `mise run check synvelis-app` | Pass |
| `mise run lint synvelis-app` | Pass（warnings denied） |
| `mise run check synvelis-plugin-test-l1-runner` | Pass |
| `mise run test-tim03-l1` | Pass（2 passed / 0 failed。VST3・CLAP oracle JSON 出力） |
| `mise run test synvelis-app` | Pass |
| `mise run test synvelis-player` | Pass（既存 TIM-03/session/attach/capability suites 非回帰） |
| `mise run test-rt` | Pass（workspace `rt_` filter lane） |
| `mise run check-headless-deps` | Pass（12 headless crates に GUI framework dependency なし） |
| `mise exec -- prek run --all-files` | Pass（whitespace/EOL/YAML/TOML/line-ending/BOM/private-key/actionlint） |

ローカル検証は変更範囲の検証であり、統合検証は merge 後の対象 SHA に対する必須 CI
証拠として別途確認する必要がある。

## pstack Verification

- Runtime Verification: NotRun（pstack project verify skill としての GUI runtime
  drive は未実施）。この slice は headless な production routing seam と out-of-process
  plugin hosting が対象で、GUI/Native Dock は scope 外のため。代替の canonical runtime
  evidence は `mise run test-tim03-l1` が実 VST3/CLAP fixture binary・実 runner child
  process・production routing activation/generation retirement・direct endpoint PCM
  oracleを駆動した exit 0 の結果である。
- Formal Verification: NotRun。formal verification 対象の journal wire codec /
  authorization boundary（`docs/specs/formal-verification.md`）や concurrency/model
  検証対象を変更していないため。代替の canonical 検証は `check`/`lint`/`test`/`test-rt`
  と typed lifecycle/activation oracle であり、formal proof を主張しない。

## root `Cargo.toml` / `Cargo.lock` の限定変更

Coordinator 許可どおり、root `Cargo.toml` の変更は次の 2 点だけである。

1. workspace members に `crates/synvelis-plugin-test-l1-runner` を追加。
2. workspace dependencies に `ruviform-plugin-export` git dependency を追加。

`Cargo.lock` は新 workspace member package entry の追加だけで、ruviform pin
`20dc41f71c47674b3e51d1eb0921124b94f8bfaf` や他 dependency version は更新していない。

## 残件

- tempo/loop/automation sample accuracy matrix、MIDI/transport precision、
  `test_instrument` oracle。
- crash/hang/timeout/fault isolation、`test_fault`、RT/Crash の最終判定。
- `test_stateful`/state quality（sv#128/TIM-04 側との lease 境界に従う）。
- GUI/Native Dock、実 Window、Windows EXT-L3 最終受入。
- AC4/AC7 CI/cross-consumer gate、AudioLens f64/native PCM 非回帰、macOS/Linux 実走。
- production generation retirement は `RemoveProcessor` → replacement generation 2
  activation → retired generation handles empty として独立観測済み。ただし endpoint
  object detach 自体は upstream API に detach receipt がなく、same-instance re-attach
  oracle は lifecycle teardown 後に成立しないため NotRun/unobservable。direct endpoint
  detach は Pass。active generation 中に endpoint object drop だけを分離観測するには、
  upstream か Synvelis engine に専用の観測 hook が必要であり後続 slice の論点とする。
- docs index（`docs/**/README.md`）は生成物のため編集せず、Coordinator が統合時に整備する。
