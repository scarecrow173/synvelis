# Evidence — synvelis#127 TIM-03 dynamic musical transport

slice `tim-03-dynamic-musical-transport`（Refs #127。Issue close ではない）。baseline PR #686 の production musical transport の上に、committed project revision後の tempo map / tempo revision / project PPQN を production control plane が dynamic refresh することを、real out-of-process VST3/CLAP `test_instrument` で検証した。

正本ドキュメント:

- `docs/specs/tim-03-dynamic-musical-transport.md`
- `docs/designs/tim-03-dynamic-musical-transport.md`
- `docs/plans/tim-03-dynamic-musical-transport.md`
- `docs/tasks/tim-03-dynamic-musical-transport.md`
- `docs/test-specs/tim-03-dynamic-musical-transport.md`
- `docs/impl/tim-03-dynamic-musical-transport.md`

machine-readable 実測値: `evidence/sv127/dynamic-musical-transport.jsonl`
raw logs / command summary: `evidence/sv127/dynamic-musical-transport/`

## Anchor — 対象 SHA・baseline・環境

- branch: `feat/127-dynamic-musical-transport`
- tested target SHA: `833db0de36bc6d74fe028088fa7c80de507152c4`（implementation + dynamic docs + test entrypoint。evidence/doc-only commit はこの検証後に追加する）
- implementation commit: `91738435266b7b28a2b164d27e543e0b33af9d2b`（production semantics / oracle / propagation。Coordinator review済み）
- baseline PR #686 merge commit: `82095ea5c9545345ff193156525243bd8ca0fcc4`
- OS/Arch: Windows x86_64（Orca worktree `G:\orca\workspaces\synvelis\sv127-dynamic-transport`）
- ruviform pin before: `511e1a7944c35797942ba065c092b9263067c48a`
- ruviform pin after: `511e1a7944c35797942ba065c092b9263067c48a`（unchanged）
- ruviform lock source count: 36（`check-ruviform-rev` も 36 sources / REV match を出力）
- `Cargo.lock` / `Cargo.toml`: baseline PR #686 から変更なし

## 実測 artifact と dynamic verdict

production seam は `compose_routing_engine_with_out_of_process_and_boundaries`。direct endpoint process API / caller supplied `TransportInfo` は使用していない。両 format で initial/final tempo revision は 2/4、project PPQN は 480、adapter projection PPQN は 960、refresh errors は空、plan current-source identity は true だった。

| fact | VST3 | CLAP |
| --- | --- | --- |
| fixture SHA-256 | `1b4db1098711d5020efa9456157095858998b08d08648cda1886def4d5d0dfcf` | `c127d3d86f6ef3424c6d59bd0b435f98e4d2ea467bb209e80e6cee61aa8b78d8` |
| runner SHA-256 | `1a00d1121153d3e483f13f130cac461cf1445d16727e541418c192dce72b2a58` | `3c29adca78f8c4026a2f4eccc973e104f7cffe89e6f197dfa8802901d9be6b13` |
| before-measured edit | tick 4800, 75.000 BPM, revision 3 | tick 4800, 75.000 BPM, revision 3 |
| before-measured click | expected 672, observed `[672]` | expected 672, observed `[672]` |
| while-playing edit | tick 9600, 100.000 BPM, revision 4 | tick 9600, 100.000 BPM, revision 4 |
| while-playing click | expected 632, observed `[632]` | expected 632, observed `[632]` |
| baseline seek positives | 704/640/576 exact | 704/640/576 exact |
| baseline loop positives | `[1024,1792]` exact | `[1024,1792]` exact |
| stopped/paused negatives | pass/pass | pass/pass |

独立計算式は fixture lawに L1 lookaheadを加えたものであり、observationの後付けではない。

```text
ceil((boundary_tick - position_ticks) * sample_rate * 60000
     / (bpm_milli * adapter_ppqn)) + lookahead_frames
```

48 kHz・adapter PPQN 960・lookahead 512 framesで、75.000 BPM delta2は 672、100.000 BPM delta2は 632。click impulseは amplitude bit-exact 0.5、stereo identical、non-click frames silentだった。loop evidence後に `ClearLoop` と追加blocksで測定窓を分離し、旧loop eventがdynamic positionへ混入しないようにした。各 dynamic caseの RT install receipt最終状態は対応plan generationの `Applied` で、requested/pending transitional rowsもJSONLに残している。

## Player/control proof

`crates/synvelis-player/tests/tim03_musical_transport_plan.rs` は 4/4 pass。representative mixed-plan testは staged MIDI 2件、unbound fast-control event、`Some(TransportGeneration(1))` eventを同じplanに共存させ、musical refresh後も event position/source sequence/payload/generation binding/staged MIDIが保持されることを検証した。PPQN mismatchを注入した cloned projectは typed unavailableとなり、両 musical halfが空で fixed 960が出ないことを検証した。

`FastControlWrite::transport_generation` の expanded compile-propagation files差分は mechanical `None` 追加のみだった。production behaviorは `None` unbound compatibilityを保ち、`Some` のみ upstream `bound_event` semanticsを使う。test-only seeding API、fake plan direct install、direct endpoint injectionはない。

## Canonical verification（local）

ローカル検証は変更範囲の検証である。統合検証は merge後の対象SHAに対する必須CI証拠として区別する。

| command | kind | OS | exit | result | raw log |
| --- | --- | --- | ---: | --- | --- |
| `mise run sync-ui-deps` | canonical | Windows x86_64 | 0 | Pass | `dynamic-musical-transport/sync-ui-deps.log` |
| `mise run check-ruviform-rev` | canonical | Windows x86_64 | 0 | Pass | `dynamic-musical-transport/check-ruviform-rev.log` |
| `mise run fmt-check` | canonical | Windows x86_64 | 1 | InfrastructureFailure | `dynamic-musical-transport/fmt-check.log` |
| `mise exec -- cargo fmt --package synvelis-player --package synvelis-app -- --check` | auxiliary | Windows x86_64 | 0 | Pass | `dynamic-musical-transport/rustfmt-package-aux.log` |
| `mise run check synvelis-player` | canonical | Windows x86_64 | 0 | Pass | `dynamic-musical-transport/check-synvelis-player.log` |
| `mise run lint synvelis-player` | canonical | Windows x86_64 | 0 | Pass | `dynamic-musical-transport/lint-synvelis-player.log` |
| `mise run test synvelis-player` | canonical | Windows x86_64 | 0 | Pass | `dynamic-musical-transport/test-synvelis-player.log` |
| `mise run check synvelis-app` | canonical | Windows x86_64 | 0 | Pass | `dynamic-musical-transport/check-synvelis-app.log` |
| `mise run lint synvelis-app` | canonical | Windows x86_64 | 0 | Pass | `dynamic-musical-transport/lint-synvelis-app.log` |
| `mise run test synvelis-app` | diagnostic | Windows x86_64 | 101 | InfrastructureFailure | `dynamic-musical-transport/test-synvelis-app-initial-artifact-failure.log` |
| `mise run test synvelis-app` | canonical | Windows x86_64 | 0 | Pass | `dynamic-musical-transport/test-synvelis-app.log` |
| `mise run test-tim03-dynamic-musical-transport` | canonical | Windows x86_64 | 0 | Pass | `dynamic-musical-transport/test-tim03-dynamic-musical-transport.log` |
| `mise run test-tim03-production-transport` | canonical | Windows x86_64 | 0 | Pass | `dynamic-musical-transport/test-tim03-production-transport.log` |
| `mise exec -- prek run --all-files` | diagnostic | Windows x86_64 | 1 | FailFixedAndRerun | `dynamic-musical-transport/prek-all-files-initial-evidence-normalization.log` |
| `mise exec -- prek run --all-files` | canonical | Windows x86_64 | 0 | Pass | `dynamic-musical-transport/prek-all-files.log` |

machine-readable command summary: `dynamic-musical-transport/canonical-results.tsv`

`mise run fmt-check` は rustfmt差分ではなく、workspace path列挙段階で `ファイル名または拡張子が長すぎます。 (os error 206)` を返した。成功条件は弱めていない。workspace-wide `fmt-check` は infrastructure failureとして記録し、package-scoped `mise exec -- cargo fmt --package synvelis-player --package synvelis-app -- --check` exit 0 を補助証拠として併記する。

`mise run test synvelis-app` の初回実行は、test-only L1 runner artifactがまだ buildされていない環境で `pc_c4_l1_real_abi_binding` が fail-closedになり exit 101 だった。これは production code failureではなく test artifact provisioning/order問題として raw log `dynamic-musical-transport/test-synvelis-app-initial-artifact-failure.log` に保存した。dedicated laneがrunnerをbuildした後、同じ canonical command `mise run test synvelis-app` を再実行し exit 0 を確認した。初回失敗を隠さず、final rowと初回diagnostic rowの両方を記録する。

evidence raw logsを staged 状態へ追加した後の `prek run --all-files` 初回実行は、2つの `.log` 末尾 newline normalizationで exit 1 だった。hookが修正した内容をstageし、同じ commandを再実行して exit 0 を確認した。normalization前後の raw log referenceを両方残す。

## pstack Verification

- Runtime Verification: Pass（required runtime scope）。pstack GUI runtime driveは NotApplicable（このsliceはGUI/Native Dockではなくheadless production control/plugin ABI境界）。canonical runtime evidenceは `mise run test-tim03-dynamic-musical-transport` が実VST3/CLAP fixture binary・実runner child process・production composition・dynamic project edit・control receipts・independent PCM oracleを駆動して exit 0 だったことである。
- Formal Verification: NotApplicable。formal verification対象の journal wire codec / authorization boundary / concurrency/model検証対象を変更していない。`check`/`lint`/`test` と typed boundary oracleは canonical検証だが、formal proofとは主張しない。
- Review-protocol-equivalent: Pass、remaining P0/P1 blocker 0。follow-up差分を確認し、production semantics/test oracle/transport_generation propagationを変更していないこと、expanded compile-propagation filesが mechanical `None` のみであること、docs index未編集であることを確認した。review evidence: `dynamic-musical-transport/review-protocol-equivalent.log`。残る非スコープは下記の明示的NotRun/follow-upのみである。

## Supplemental non-canonical lane

`mise run test-rt` はこのslice specのcanonical commandではない。前回implementation状態で開始され後に完了したsupplemental runは Windows x86_64で exit 0 だったが、required laneの代替ではない。raw log: `dynamic-musical-transport/supplemental-test-rt.log`。

## Honest non-scope / remaining work

full dynamic tempo edit matrix、automation parameter matrix expansion、full frame/boundary matrix、fault/state/crash isolation、GUI/Native Dock/EXT-L3、macOS/Linux runtime、3OS CI、AudioLens cross-consumerは NotRun/follow-up。docs index（`docs/**/README.md`）は編集・再生成していない。Coordinatorが統合時に整備する。
