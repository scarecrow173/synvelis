# Evidence — synvelis#127 TIM-03 transport application minimal vertical

slice `tim03-transport-application-s1`（Refs #127。Issue close ではない）。
Synvelis app 所有の transport application seam で project musical time を一度だけ
media frame へ変換し、loop/seek/play と automation evaluation を production
composition 経由の実 VST3/CLAP `test_gain` L1 route へ適用した。control ledger、
folded transport span、PCM membership、inactive generation rejection を両 format で
実測した。

正本ドキュメント:

- `docs/specs/tim-03-transport-application.md`
- `docs/designs/tim-03-transport-application.md`
- `docs/plans/tim-03-transport-application.md`
- `docs/tasks/tim-03-transport-application.md`
- `docs/test-specs/tim-03-transport-application.md`
- `docs/impl/tim-03-transport-application.md`

machine-readable 実測値: `evidence/sv127/transport-application.jsonl`

## Anchor — 対象 SHA・環境

- branch: `feat/127-tim03-transport-application`
- base/HEAD before commit: `da5a2bc61e0f4362d8f96603a922bd8ea5f157a8`
  （最終検証前に `git fetch origin main` し、fast-forward で取り込んだ `origin/main`）
- OS/Arch: Windows x86_64（Orca worktree
  `G:\orca\workspaces\synvelis\sv127-transport-app`）
- toolchain: rustc 1.99.1（mise 管理）
- ruviform resolved SHA: `0479fa62e77e669a7bf84320d3d5418ffb949fc6`
  （`Cargo.lock` pin。pin 更新なし）

## 実測 artifact と verdict

| format | fixture SHA-256 | runner SHA-256 | production activation | verdict |
| --- | --- | --- | --- | --- |
| CLAP | `edbd8343225cd8a70cfe0812b620b61df8161b8cdf1802fbe69d5d9ba7a48b96` | `73913022d5380abaf6169e7d216928826fde57ab9d993b448fc99d13a5de48ca` | `Active` generation 1 | Pass（scoped host-side transport application oracle） |
| VST3 | `15d563c922d313c27d85c404ff42e91a4b8d05de6490538b988687e528591d5c` | `dcf16faa98989379e3f9a2975c0aa9be09a823dc637700153eea97760fc9298a` | `Active` generation 1 | Pass（scoped host-side transport application oracle） |

runner は `mise run test-tim03-transport` が build する Synvelis 所有 test-only
artifact。fixture/runner 解決と sibling staging は PR #668/#672 の contract を再利用し、
hash mismatch は fail-closed とした。

## conversion / transport oracle

tempo map は初期 120 BPM、tick 1 から 125 BPM の非 default map。両 format で同一の
conversion evidence を得た。

| role | beat input | media frame |
| --- | ---: | ---: |
| LoopStart | `1/60` | 385 |
| LoopEnd | `1/36` | 641 |
| Seek | `1/45` | 513 |
| Automation | `17/720` | 545 |

first window は `call_start 512`・256 frames。`SetLoop [385,641)`、`Seek 513`、`Play` は
`boundary_stage_window` staged order で due 0 に入り、policy は `Exact`、precision は
`FrameAccurateEvents`。second window `[768,1024)` では loop end 641 まで 128 frame のため、
`boundary_loop_wraps` 由来の `LoopWrap` boundary offset 128 を staging oracle へ供給した。

## control ledger / span oracle

automation write は両 format で `Dispatched` を観測した。media position は 545、
transport anchor は `512 - 513 = -1`、expected physical frame は 544、ledger projection は
`call_start 512 + frame_offset 32 = 544` を再構成した。tick evidence は
`evaluated=1, events=1, dispatch_errors=0, gated=false, stale=false`。

seek/loop 後の surface span は `start_frame 768`、`Playing`、`position_ticks 513`、
`loop_ticks [385,641]`、`loop_occurrence 1`、`transport_generation 4`、
`plan_generation 1` であり、folded position・loop range・generation を surface evidence
として観測した。

## PCM oracle

`test_gain` は transport を消費しないため、PCM は plugin-side transport oracle ではなく
host-side application と automation value law の oracle である。input `(+0.25,-0.25)`、
automation normalized 0.375、plain gain 0.75 に対し、両 format で blocks 4..11 が
`input*0.75` membership を満たした。

fresh delivery に数えない block は次のとおり分類した。

- silence / lookahead: blocks 0..1
- pass-through unity: block 2（`input*1.0`）
- transition/mixed: block 3（`other`）

normalized value を plain gain と誤認した場合の `input*0.25` block は 0 件だった。
production PCM surface だけでは substituted/pass-through を物理的に分離できないため、
silence/unity/transition block は automation delivery に算入せず、matched block のみを
`input*0.75` fresh delivery とした。

## negative evidence

clip 追加で surface generation を 2 へ進めた後、RT-observed active generation 1 のまま
transport application を呼び、両 format で typed
`InactiveGeneration { surface: 2, active: 1 }` を観測した。silent success はなかった。

## evidence matrix

| lane / scope | verdict | evidence |
| --- | --- | --- |
| app-owned beat→media conversion boundary | Pass | conversion evidence 4 roles、同一 beat cache、verified frame base `1/48000` |
| boundary staging / Exact policy | Pass | first/second staged commands、second boundary offset 128 |
| VST3 control ledger projection | Pass | `512 + 32 = 544` |
| CLAP control ledger projection | Pass | `512 + 32 = 544` |
| folded span / loop discontinuity evidence | Pass | position 513、loop `[385,641)`、occurrence 1 |
| VST3/CLAP PCM defined-output membership | Pass（limited） | matched blocks 4..11、non-fresh blocks excluded |
| inactive generation rejection | Pass | `InactiveGeneration { surface: 2, active: 1 }` |
| plugin-side musical transport oracle (`test_instrument`) | NotRun | `test_gain` は transport を消費しない。後続 slice |
| dynamic tempo edit / full frame-boundary matrix | NotRun | 後続 slice |
| fault/state isolation (`test_fault`/`test_stateful`) | NotRun | 後続 slice |
| GUI/Native Dock/EXT-L3/provisioned endpoint | NotRun | 本 slice 範囲外 |
| macOS/Linux runtime / 3OS CI | NotRun | 本 slice は Windows local。merge 後 CI で追跡 |

## 検証コマンドと結果（canonical = mise）

| コマンド | 結果 |
| --- | --- |
| `mise run fmt-check` | Fail（`cargo fmt --all -- --check` が rustfmt 起動前に `ファイル名または拡張子が長すぎます。 (os error 206)`。後述の infrastructure note） |
| `mise run check synvelis-app` | Pass（exit 0） |
| `mise run lint synvelis-app` | Pass（warnings denied、exit 0） |
| `mise run test-tim03-transport` | Pass（2 passed / 0 failed。VST3・CLAP oracle JSON 出力） |
| `mise run test-tim03-l1` | Pass（exit 0） |
| `mise run test synvelis-app` | Pass（exit 0） |
| `mise run test synvelis-player` | Pass（exit 0） |
| `mise run test-rt` | Pass（exit 0） |
| `mise run check-headless-deps` | Pass（12 headless crates に GUI framework dependency なし） |
| `mise exec -- prek run --all-files` | Pass（whitespace/EOL/YAML/TOML/line-ending/BOM/private-key/actionlint） |

補助証拠（canonical = mise ではない）: `cargo fmt -p synvelis-app -- --check` は exit 0。
変更した package の Rust formatting 自体は pass している。

### `fmt-check` infrastructure note

`mise run fmt-check` はこの Orca worktreeでのみ rustfmt 起動前に os error 206 を返す。
同一コマンドは短い main checkout `G:\repositry\audio-products\synvelis` では exit 0、
別 Orca worktree `G:\orca\workspaces\synvelis\sv122-ctl02-persist` では同じ exit 1 を
確認した。`cargo fmt -v` は全 workspace target を `\?\G:\orca\workspaces\...` の
長い絶対パスで列挙しており、worktree パス長に依存する cargo fmt/Windows の環境制約と
判断した。検証成功条件は弱めていない。package-scoped な rustfmt check は pass、workspace-wide
`fmt-check` は out-of-scope infrastructure failure として PR に記録する。

ローカル検証は変更範囲の検証であり、統合検証は merge 後の対象 SHA に対する必須 CI
証拠として別途確認する必要がある。

## pstack Verification

- Runtime Verification: NotRun（pstack project verify skill としての GUI runtime
  drive は未実施）。この slice は headless production routing seam と L1 plugin process
  oracle が対象で、GUI/Native Dock は scope 外。代替の canonical runtime evidence は
  `mise run test-tim03-transport` が実 VST3/CLAP fixture binary・実 runner child
  process・production composition・boundary staging・control ledger/span/PCM oracleを
  駆動した exit 0 の結果である。
- Formal Verification: NotRun。formal verification 対象の journal wire codec /
  authorization boundary や concurrency/model 検証対象を変更していないため。代替の
  canonical 検証は `check`/`lint`/`test`/`test-rt` と typed boundary/session oracle であり、
  formal proof を主張しない。

## 残件

- docs index（`docs/**/README.md`）は Worker 契約どおり未編集。Coordinator が統合時に整備する。
- workspace-wide `fmt-check` の os error 206 は Orca worktree パス長に依存する infrastructure
  failure として残る。コード側の formatting は package-scoped rustfmt check で pass。
