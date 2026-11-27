# TIM-03 frame/boundary matrix verification evidence

Issue: `scarecrow173/synvelis` `#127` / slice `tim03-frame-boundary-matrix-s1`
Branch: `feat/127-frame-boundary-matrix`
OS/Arch: Windows / x86_64

## Baseline and dependency pins

| item | value |
| --- | --- |
| base | `origin/main` `0954a19f74711dfe4b4b54dcdcf87f2ba992a940` |
| production musical transport baseline | PR `#686` merge commit `82095ea5c9545345ff193156525243bd8ca0fcc4` |
| representative dynamic refresh baseline | PR `#687` merge commit `0954a19f74711dfe4b4b54dcdcf87f2ba992a940` |
| ruviform resolved revision | `511e1a7944c35797942ba065c092b9263067c48a`（変更なし） |
| lock source count | 36 lockfile sources, lockfile-primary, REV match |
| `Cargo.toml` / `Cargo.lock` / `ui/vendor/ruviform/` | 変更なし |

`ui/vendor/ruviform` は worktree コピーが stale であったため `mise run sync-ui-deps` を実行した。これは gitignore された生成物の再同期であり、vendor の tracked diff はない。最終 `check-ruviform-rev` は REV match で exit 0。

## 判定律と evidence の読み方

- exact physical process-length sequence の authoritative proof は headless recording-plugin matrix に限定する。
- production real ABI の click position は independent PCM oracle による primary evidence だが、physical call length そのものは PCM から直接分離できないため indirect evidence と明記する。
- click law は `ceil((boundary_tick - position_ticks) * sample_rate * 60000 / (bpm_milli * adapter_ppqn)) + lookahead_frames` をテスト外の arithmetic として計算する。`sample_rate=48000`、host/project PPQN は 480、VST3/CLAP adapter projection PPQN は 960。fixed 960 fallback ではなく、adapter が project PPQN 480 を projection した値である。
- L1 latency は `lookahead_blocks * activated max_frames`。本 evidence では `lookahead_blocks=2`、geometry 100/128/256 に対して latency 200/256/512 frame。256 frame や 2 block の固定値とは扱わない。
- freshness では initial silence、pass-through、substituted block、duplicate block を positive click evidence に数えない。observed click frame は run aggregate 内の新規 PCM position として記録する。
- fixture click は amplitude 0.5、stereo identical、非 click frame silence を確認する。fixture DSP law を production source に複製していない。

## Matrix coverage summary

| layer | W=100 | W=128 | W=256 | verdict |
| --- | --- | --- | --- | --- |
| headless policy exact matrix | B0 pass, B1 pass, B100 typed invalid/out-of-window, B99 pass | B0/B1/B100/B127 pass | B0/B1/B100/B255 pass | Pass |
| headless scheduler matrix | valid cells exact span sequence; B100 invalid negative | valid cells exact span sequence | valid cells exact span sequence | Pass |
| headless node exact process lengths | `[100]`, `[1,99]`, `[99,1]` | `[128]`, `[1,127]`, `[100,28]`, `[127,1]` | `[256]`, `[1,255]`, `[100,156]`, `[255,1]` | Pass |
| wired session matrix | valid cells exact physical lengths、fresh instance | valid cells exact physical lengths、fresh instance | valid cells exact physical lengths、fresh instance | Pass |
| param/frame-offset ordering | offsets 0/1/middle/last-in-call、stable `(position_ticks, source_sequence)`、same-param last-wins | 同左 | 同左 | Pass |
| routing loop derivation | head wrapとinterior cutを区別 | interior derivation pass | interior derivation pass | Pass |
| driver loop derivation | window-end wrap deferred / interior geometry pass | pass | pass | Pass |
| production real ABI public seam | B0 seek/tempo pass-primary PCM、natural loop pass-primary PCM | 同左 | 同左 | Pass-with-declared-unsupported-cells |
| production arbitrary interior seek/tempo/param | unsupported-by-current-production-seam | unsupported-by-current-production-seam | unsupported-by-current-production-seam | Unsupported（fake passへ昇格しない） |
| production W100/B100 | invalid-out-of-window honest negative | N/A（B100はvalid interior） | N/A（B100はvalid interior） | Negative pass |

headless negative regression（min/max illegality、deferral/coalescing、offline exact fail-closed、stale generation zero delivery）は既存 test を弱めず、`mise run test synvelis-player` で実行された。simultaneous combined caseは既存 256-frame representative testが seek + natural loop wrap + tempo-plan install + param eventの exactly-once / tie rule / post-seek loop collapseを固定し、本sliceの matrix rowsはその ordering semanticsを consumer evidenceとして参照する。

## Production real ABI primary/secondary/NotRun

Primary evidence は VST3/CLAP 両 format の ruviform `test_instrument` click path。production composition seam `compose_routing_engine_with_out_of_process_and_boundaries`、out-of-process L1 runner、sibling staging、hash mismatch fail-closed を使用し、direct endpoint process API、caller-supplied `TransportInfo`、test-only seeding、fake plan direct install、direct endpoint injection は使用していない。

各 geometry で少なくとも次を independent PCM oracle で確認した。

1. seek-derived positive（public batch `Seek`+`Play`）
2. natural loop-wrap positive（public batch `SetLoop`+`Seek`+`Play`。same-head batch順序を記録し、loop内first clickとpost-wrap second clickの両方を観測して wrap crossingを判別。W=100ではblock-grid latticeを満たす25-host-tick tailを使用）
3. tempo revision / plan-boundary positive（`ProjectOp::AddTempoEvent` による tempo-map/tempo-revision discontinuity。存在しない `TransportCommand::Tempo` は捏造していない）

Param/frame-offset の `test_instrument` primary row は、deterministic voice/click lawから独立分離できないため NotRun。既存 `tim03_l1_transport_application` の `test_gain` laneは secondary corroboration として位置付け、primary matrix Passへ昇格していない。

## Canonical verification commands

`canonical-verify*.log`、`lint-*-rerun.log`、`check-ruviform-rev-final.log`、`rustfmt-package-check.log` に raw output を保存した。final status は次表のとおり。途中の scoped lint failure は修正済みであり、final rerun を canonical verdict とする。

| command | final exit | classification / notes |
| --- | ---: | --- |
| `mise run fmt-check` | 1 | Infrastructure failure: Windows Orca worktree path length dependency `os error 206` (`ファイル名または拡張子が長すぎます。`). 成功条件は弱めていない。補助証拠として package-scoped rustfmt を実行。 |
| `mise exec -- cargo fmt -p synvelis-player -p synvelis-app -- --check` | 0 | 補助証拠（canonical = mise）。fmt-checkの代替合格とはみなさない。 |
| `mise run check-ruviform-rev` | 0 | `511e1a7944c35797942ba065c092b9263067c48a`, 36 lockfile sources, lockfile-primary, REV match |
| `mise run check synvelis-player` | 0 | Pass |
| `mise run lint synvelis-player` | 0 | Pass（scoped clippy diagnostics修正後のfinal rerun） |
| `mise run test synvelis-player` | 0 | Pass |
| `mise run check synvelis-app` | 0 | Pass |
| `mise run lint synvelis-app` | 0 | Pass（scoped clippy diagnostics修正後のfinal rerun。最終post-review rerunもexit 0） |
| `mise run test synvelis-app` | 0 | Pass |
| `mise run test-tim03-frame-boundary-matrix` | 0 | Pass（VST3/CLAP両format。final post-review rerunのraw stdoutから machine rowsを抽出） |
| `mise run test-tim03-dynamic-musical-transport` | 0 | Pass（既存 production real ABI回帰） |
| `mise run test-tim03-transport` | 0 | Pass（既存 transport application回帰） |
| `mise run test-rt` | 0 | Pass（workspace RT no-alloc/no-block contract tests） |
| `mise run check-headless-deps` | 0 | Pass |
| `mise exec -- prek run --all-files` | 0 | Pass |


Note: `canonical-verify3.log` preserves the first full canonical batch. During review the natural-loop production oracle was strengthened from a single in-loop click to first in-loop click plus post-wrap second click; the final authoritative matrix output is therefore the later `final-app-and-dedicated.log` and the regenerated `frame-boundary-matrix.jsonl`.

## Artifact/negative evidence

Machine-readable rows are in `frame-boundary-matrix.jsonl`, with these row families:

- `headless_policy_matrix`
- `headless_scheduler_matrix`
- `headless_node_matrix`
- `headless_param_order_matrix`
- `wired_session_matrix`
- `driver_loop_derivation_matrix`
- `routing_loop_derivation_matrix`
- `production_real_abi_matrix`（VST3/CLAP 2 rows）
- `negative_and_constraint_evidence`

Negative/constraint evidence:

- W100/B100 は 99 へ clampせず typed invalid/out-of-window。
- arbitrary interior seek/tempo-plan/param scheduling は public seam未対応として `unsupported-by-current-production-seam`。
- fake evidence を production ABI Passへ昇格していない。
- fixed 960 fallback、test-only seeding、fake direct install、direct endpoint injection、caller-supplied `TransportInfo` はなし。
- stale plan/current のすり替えは追加していない。既存 stale generation regressionは `synvelis-player` canonical testで実行。

## Environment/infrastructure incidents

1. `fmt-check`: `cargo fmt --all -- --check` が rustfmt出力前に `os error 206`。package-scoped rustfmtは成功したが、あくまで補助証拠。
2. `check-ruviform-rev` initial failure: worktreeの gitignored vendor copy が `20a24a66...` で stale。`mise run sync-ui-deps` 後に lockfile resolved `511e1a79...` と一致。tracked vendor diffなし。
3. scoped clippy failures: newly added tests内の `doc_markdown` / `too_many_arguments` / `needless_pass_by_value`。いずれも実装側を修正し、final lint rerun exit 0。

## Pass/Fail/NotRun/Unsupported/Blocked separation

- Pass: 上記 headless matrix、routing/driver derivation、production real ABI seek/loop/tempo primary positives、canonical pass lanes。
- Fail: 最終状態で未解決の scoped verification failureはない。
- NotRun: `test_instrument` param/frame-offset primary oracle。full dynamic tempo edit matrix、広い automation matrix、fault/state/crash isolation、GUI/EXT-L3、macOS/Linux runtime、3OS CI、AudioLens cross-consumer。
- Unsupported: current public production seam が表現できない arbitrary interior command/plan/param scheduling cells。
- Blocked: なし。`fmt-check` は infrastructure failureとして分類し、Pass扱いにしない。

## Raw evidence inventory

- `frame-boundary-matrix.jsonl` — machine-readable rows
- `frame-boundary-matrix/canonical-verify.log` — first canonical batch including fmt/checkrev/player lanes
- `frame-boundary-matrix/canonical-verify2.log` — player lint final and app check/lint/test batch
- `frame-boundary-matrix/canonical-verify3.log` — dedicated lane, regression lanes, RT/headless/prek batch
- `frame-boundary-matrix/final-app-and-dedicated.log` — final post-review app lint, `test synvelis-app`, and dedicated matrix lane rerun used for JSONL extraction
- `frame-boundary-matrix/prek-final.log` — final prek run after evidence/doc updates
- `frame-boundary-matrix/lint-player-rerun.log` — final player lint evidence
- `frame-boundary-matrix/lint-app-rerun.log` — final app lint evidence
- `frame-boundary-matrix/check-ruviform-rev-final.log` — final ruviform pin evidence
- `frame-boundary-matrix/rustfmt-package-check.log` — supplemental package-scoped rustfmt evidence
- `frame-boundary-matrix/sync-ui-deps.log` — generated vendor resync command evidence
