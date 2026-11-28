# Evidence — synvelis#127 TIM-03 core semantics（player fixture slice）

slice `tim03-core-semantics-s1`。Issue close ではなく `Refs #127` の前進 slice。対象は `synvelis-player` の fake/headless fixture が所有する core semanticsであり、real ABI plugin、production composition、synvelis-app L1 runner、GUI/EXT-L3、Plugin State Snapshot 本体は対象外。

machine-readable matrix: `evidence/sv127/core-semantics.jsonl`<br>
command/evidence logs: `evidence/sv127/core-semantics/`（`*.log` は gitignore 対象の raw evidence。永続サマリは本書と TSV）

## Anchor

- branch: `feat/127-tim03-core-semantics`
- base SHA during local verification: `3a7e3ed9b17aebb236acde9bdce125abc721cbe0`（検証開始時点の `origin/main` と一致。`git fetch origin main` exit 0、merge-base 確認済み）
- implementation commit: `8395be14f659165041ffcf269a86c2c2340b44f7`（`feat/127-tim03-core-semantics` の TIM-03 implementation commit）
- final manual branch CI target SHA: `6187ce12907652f5c61980c80813fa8e76573bf1`（`fix(ci): isolate artifact-dependent L1 tests (#696)` を含む `origin/main` `1ef341920cfd6b61ec4a27e54531f2c59611f6c6` を merge し、evidence whitespace fix を載せた pushed head。後続の evidence-only commit と混同しない）
- OS/Arch: Windows / x86_64
- worktree: `G:\orca\workspaces\synvelis\sv127-tim03-core-semantics`
- ruviform resolved revision: `511e1a7944c35797942ba065c092b9263067c48a`（pin/lockfile/vendor は未変更）
- verification identity: ローカル検証は未コミット作業ツリー上の変更範囲検証。統合受入は merge 後 main SHA の CI 証拠。evidence 追加自体は Rust semantics を変更しない。

## Scopeと判定規則

本 slice は negative case を成功へ読み替えない。typed refusal、honest drop、zero wrong delivery のいずれかを観測した場合だけ該当 semantic を Pass とし、未実行・範囲外・インフラ失敗は Pass にしない。`Dispatched` は plugin acknowledgement ではなく plan×span projection として扱い、receipt が示す live plan generation と staged plan generation が食い違う場合には projection しない。

## 既存 coverage 棚卸し

lease 内 6 つの TIM-03 test file と loop-boundary regression を棚卸しした結果、次は main 時点で credible に covered と判断し、重複実装しなかった。

| semantic axis | 既存 evidence | 判定 |
| --- | --- | --- |
| Capability unsupported | `tim03_cap_unsupported_typed_refusal`、attach/connect/driver activation の typed refusal | covered |
| Realtime degraded consent | oracle/scheduler/node/session の consent matrix、100→128 legal deferral | covered |
| Offline Exact fail-closed | illegal boundary、uncertified precision、consent non-rescue | covered |
| illegal process length 0 / min-max | zero window typed invalid、nonzero legal tiling、min/max fail-closed | covered |
| provenance I/N/CG/GG mismatch | `tim03_prov_stale_generations_zero_delivery`、driver/routing forged pins | covered |
| frame-base mismatch | 44.1k foreign dispatch / foreign plan install typed mismatch | covered |
| stale transport generation | bound stale TG zero delivery、unbound absolute event exactly once | covered |
| 100/128/256・boundary 0/1/100/last・loop/seek/tempo/param ordering | frame/boundary matrix、simultaneous ordering、loop regression | covered |
| RT Exact hot path | `tim03_process_boundary_rt.rs` allocation/deallocation 0 | covered（canonical `test-rt` で再確認） |

不足していたのは、clock epoch/device restart を driver core semantics として表現する軸、restart 前 pending plan/control の typed rejection evidence、post-restart explicit re-anchor/replan/resume policy、および receipt live plan generation と staged plan generation の mismatch を span evidence に残す経路だった。

## 実装した core invariant

1. `HeadlessDriver::restart_attached_clock` を追加し、upstream `PluginNodeControl::device_restart` を driver が所有する explicit discontinuity として公開した。restart は seek/transport generation bump/plan install/frame-base mismatch と混同しない。
2. `TransportMirror` は clock epoch を node ごとに追跡する。restart 成功 push では epoch を進め、stopped/position 0/occurrence 0 を予測し、receipt drain では scheduler truth を優先する。
3. restart 前に staged された control/plan は upstream FIFO で restart より先に scheduler pending へ入り、`device_restart` によって `StaleGeneration` rejection になる。driver はその batch の command-predicted transport state を引きずらない。
4. restart 自体は resume ではない。consumer が explicit に replan・re-anchor/resume するまで `Stopped` が param/MIDI/plan delivery を suppress する。
5. same-head install が block 内イベントより先に適用されるため、projection 入力は staged plan generation を使う。一方で stored span evidence は drain した receipt の live epoch/transport generation/plan generation に補正し、rejected install を live generation として報告しない。
6. `ParamDispatch` の receipt-generation gate を維持し、live generation mismatch の plan を `Dispatched` projection しない。receipt がまだ存在しない standalone/fabricated-span consumer の既存 contract は変更しない。
7. `WrapSim` も upstream と同じ restart reset（stopped、position 0、loop region は preserve）を行い、loop boundary derivation が旧 epoch の anchor を引きずらないようにした。

## 新規 semantic cases

| case | test | 入力 | 期待 | 実観測 | wrong delivery |
| --- | --- | --- | --- | --- | --- |
| clock restart rejects pending plan/control | `tim03_sw_11_driver_clock_restart_rejects_stale_pending_plan` | epoch 0 で param 160/224 + MIDI 176 を含む plan gen 1→2 を pending のまま stageし、frame 128 で restart | old epoch pending apply が typed `StaleGeneration`、param/MIDI delivery 0、`Dispatched` 捏造 0 | epoch 1 の `ApplyStatus::Rejected` / `RejectReason::StaleGeneration` receipt を複数観測。endpoint param/MIDI 0。span は epoch 1・Stopped・position 0・rejected plan gen 2 を live としない | 0 |
| explicit replan/re-anchor/resume | `tim03_sw_12_driver_clock_restart_requires_explicit_replan_resume` | epoch 0 で 160 delivery 済み、live plan の 320 は未delivery。frame 256 restart 後、416/448 param + 420 MIDI を explicit replanし、384 で resume | restart だけでは resume しない。explicit policy 後だけ post-restart event が exactly once | 320 は suppress。160 duplicate なし。416/448/420 が1回ずつ delivery。span は epoch 1・Playing・position 384・post-restart plan gen を live で報告 | 0 |

MIDI endpoint recorder を `tim03_session_wiring.rs` の fake plugin に追加し、MIDI zero-delivery / exactly-once を param と同じ endpoint evidence で観測できるようにした。

## Canonical verification（local）

| command | OS | exit | result | raw log |
| --- | --- | ---: | --- | --- |
| `mise run fmt-check` | Windows x86_64 | 1 | InfrastructureFailure（rustfmt 起動前に `os error 206`。Pass にしない） | `core-semantics/fmt-check.log` |
| `mise exec -- cargo fmt -p synvelis-player -- --check` | Windows x86_64 | 0 | 補助証拠 Pass（canonical = mise。代替合格とはみなさない） | `core-semantics/fmt-package.log` |
| `mise run check synvelis-player` | Windows x86_64 | 0 | Pass | `core-semantics/check-synvelis-player.log` |
| `mise run lint synvelis-player` | Windows x86_64 | 0 | Pass | `core-semantics/lint-synvelis-player.log` |
| `mise run test synvelis-player` | Windows x86_64 | 0 | Pass（`tim03_session_wiring` 12/12 を含む） | `core-semantics/test-synvelis-player.log` |
| `mise run test-rt` | Windows x86_64 | 0 | Pass（初回は L1 runner artifact 未構築で exit 101。補助 setup `cargo build --locked -p synvelis-plugin-test-l1-runner` exit 0 後の同一 canonical rerun で Pass。`rt_process_boundary_hot_path_alloc_free` ok） | `core-semantics/test-rt.log` |
| `mise run check-headless-deps` | Windows x86_64 | 0 | Pass | `core-semantics/check-headless-deps.log` |

machine-readable command summary: `evidence/sv127/core-semantics/canonical-results.tsv`（最終結果で更新）。

## Branch CI（manual workflow_dispatch）

### final target SHA（`#696` merge 後）

coordinator の completion-gate follow-up に従い、`fix(ci): isolate artifact-dependent L1 tests (#696)` を含む `origin/main` `1ef341920cfd6b61ec4a27e54531f2c59611f6c6` を conflict なしで merge し（merge commit `1b119ef0a3a59dcb79426f288985a84cc41f54fe`）、軽量検証 `cargo check --locked --all-targets -p synvelis-player` exit 0（canonical ではなく merge artifact の補助確認）のうえ、evidence trailing whitespace fix を含む pushed head `6187ce12907652f5c61980c80813fa8e76573bf1` を target に `ci.yml` を手動 dispatch した。run は https://github.com/scarecrow173/synvelis/actions/runs/46125438476 である。

この evidence 更新自体は Rust source/test semantics を変えない metadata-only の後続 commitであり、branch CI が検証した target SHA `6187ce12907652f5c61980c80813fa8e76573bf1` と混同しない。

run の最終 conclusion は **success**（run attempt 2。初回 attempt で失敗した 2 job のみ `gh run rerun --failed` で再実行し、同一 SHA・同一 source diff で success に転じた）。`ci-summary` job log の `verify-sha` は `EXPECTED_SHA=6187ce12907652f5c61980c80813fa8e76573bf1` / workflow revision / checked-out HEAD の3点一致を確認した。decisive lane は両方 success であり、これが本 slice の exact target SHA に対する branch CI verdict である。

| lane | final conclusion | job | 観測 |
| --- | --- | --- | --- |
| `evidence` | **success** | https://github.com/scarecrow173/synvelis/actions/runs/46125438476/job/140932183033 | required-context 互換 pr-mode gate Pass |
| `ci-summary` | **success** | https://github.com/scarecrow173/synvelis/actions/runs/46125438476/job/140932239574 | Full CI gate Pass。`verify-sha` 3点一致 |
| `verify (ubuntu-latest)` | success | https://github.com/scarecrow173/synvelis/actions/runs/46125438476/job/140924740878 | `test-full` Pass。`tim03_sw_11` / `tim03_sw_12` が ok |
| `verify (windows-latest)` | success（failed-job rerun 後） | https://github.com/scarecrow173/synvelis/actions/runs/46125438476/job/140924708422 | `test-full` Pass。`tim03_sw_11` / `tim03_sw_12` が ok。初回 flake test `pending_commit_unreadable_manifest_degrades_recoverable` も ok |
| `verify-macos` | success | https://github.com/scarecrow173/synvelis/actions/runs/46125438476/job/140924746516 | macOS leg は PR-mode aggregation から物理分離されているが Full CI accounting では Pass |
| `rt-verify` | success（failed-job rerun 後） | https://github.com/scarecrow173/synvelis/actions/runs/46125438476/job/140924708132 | `rt_process_boundary_hot_path_alloc_free` ok。初回 flake test `restart_replays_pending_commit_to_committed_live` も ok |
| `msrv-verify` | success | https://github.com/scarecrow173/synvelis/actions/runs/46125438476/job/140924709648 | MSRV lane Pass |
| `dep-guard` | success | https://github.com/scarecrow173/synvelis/actions/runs/46125438476/job/140924729794 | ruviform dependency guard Pass |
| `gw10-verify`（ubuntu/windows/macOS） | success ×3 | https://github.com/scarecrow173/synvelis/actions/runs/46125438476/job/140924709708 / https://github.com/scarecrow173/synvelis/actions/runs/46125438476/job/140924709509 / https://github.com/scarecrow173/synvelis/actions/runs/46125438476/job/140924708999 | GW-10 harness 3OS Pass |
| `ui-verify`（ubuntu/windows）・`ui-verify-macos` | success ×3 | https://github.com/scarecrow173/synvelis/actions/runs/46125438476/job/140924733080 / https://github.com/scarecrow173/synvelis/actions/runs/46125438476/job/140924745362 / https://github.com/scarecrow173/synvelis/actions/runs/46125438476/job/140924748139 | UI check/test/build 系 Pass |

branch CI の `verify` success は artifact-dependent real ABI/L1 lane の positive evidence ではない。`#696` により artifact-dependent tests は通常 lane から分離または ignore されており、real ABI/app L1/GUI/EXT-L3 は従来どおり本 slice の scope-out / NotRun である。

### 初回 attempt failure の因果切り分け（superseded by successful rerun）

同一 run の初回 attempt では次の 2 job が failure だった。これらを隠さず flake として記録するが、最終 verdict には借用しない。

1. `verify (windows-latest)` job `140918885428`: `crates/synvelis-app/tests/sv646_pending_commit_replay.rs:127` の `pending_commit_unreadable_manifest_degrades_recoverable` が `import 24 manifest never reached committed` で failure。failed-job rerun 後の job `140924708422` では同一 test が `ok`。
2. `rt-verify` job `140918885525`: `crates/synvelis-app/tests/sv643_jobs_ledger.rs:255` の `restart_replays_pending_commit_to_committed_live` が manifest stage `published` vs expected `committed` で failure。failed-job rerun 後の job `140924708132` では同一 test が `ok`。

これらは本 slice 起因ではないと判断した。根拠は、(1) 本 slice の net diff が `synvelis-player` と `evidence/sv127/` に限定され、失敗 test が lease外の synvelis-app asset-library import replay / jobs ledger timing testであること、(2) `#696` merge commit `1ef341920cfd6b61ec4a27e54531f2c59611f6c6` 単体の main run https://github.com/scarecrow173/synvelis/actions/runs/46122126803 が全 lane success で、同じ test が main log でも `ok` だったこと、(3) 同一 SHA・同一 source diff の failed-job rerun で両 lane が success に転じたこと、(4) 同種の sv#646 polling timeout flake が `evidence/sv127/real-abi-vst3-tuid-platform-byte-order.md` にも既記録であることである。`rt-verify` で `restart_replays...` が選ばれるのは broad substring filter `rt_` が `restart_` に部分的に一致する test-selection 上の影響でもあり、TIM-03 player RT semantics の failure ではない。

### previous target SHA（`#696` merge 前、superseded）

initial completion-gate では target SHA `8395be14f659165041ffcf269a86c2c2340b44f7` を https://github.com/scarecrow173/synvelis/actions/runs/46122564406 で検証したが、この head は `#696` を祖先に含まず、`verify` 3OS / `rt-verify` / `msrv-verify` / `evidence` / `ci-summary` が artifact-dependent `synvelis-app` L1 runner executable missing で failure だった。base SHA `3a7e3ed9b17aebb236acde9bdce125abc721cbe0` の main run https://github.com/scarecrow173/synvelis/actions/runs/46102592025 でも同じ failure だったため、slice 起因ではない pre-existing harness failure と切り分けた。この negative evidence は `#696` を取り込んだ final target SHA `6187ce12907652f5c61980c80813fa8e76573bf1` の success run によって統合判断上 supersede される。旧 run を green として扱わない一方、新 run の green も artifact-dependent real ABI lane の Pass を意味しない。

## Review status

`review-protocol-equivalent blocker 0`（本 slice が所有する `synvelis-player` implementation/test/evidence 差分に残存 blocker はない）。final branch CI target SHA `6187ce12907652f5c61980c80813fa8e76573bf1` では decisive lane `evidence` / `ci-summary` がともに successであり、初回 attempt の lease外 flake だけでなく artifact-dependent L1 harness failure も `#696` merge 後の exact target SHA で解消されている。なお統合受入自体は merge 後 main SHA の CI evidence に委ねる。


## Runtime Verification / Formal Verification

| verification class | verdict | basis |
| --- | --- | --- |
| Runtime Verification | Pass（local headless/player scope） | `mise run test synvelis-player` exit 0、`mise run test-rt` exit 0。`fmt-check` は infrastructure failureとして Pass に含めない |
| Formal Verification | NotRun | 本 slice は headless behavioral invariant の実装・検証であり、形式検証 artifacts/tactic は追加していない |

## Pass/Fail/NotRun/Unsupported/Blocked separation

- Pass: 上表で exit 0 と記録した local lane、および final branch CI target SHA `6187ce12907652f5c61980c80813fa8e76573bf1` の run `46125438476` で success となった CI lane。
- Fail: 本 slice 起因の未解消 code/test failure はなし。pre-`#696` target SHA `8395be14f659165041ffcf269a86c2c2340b44f7` の run `46122564406` は lease外 artifact-dependent L1 harness failure により red だったが、final target SHA `6187ce12907652f5c61980c80813fa8e76573bf1` の success run に supersede された。run `46125438476` 初回 attempt の synvelis-app import replay/jobs ledger failure は lease外 flake であり、同一 SHA rerun で success。いずれも Pass へ読み替えず、Branch CI 節の通り分類する。
- InfrastructureFailure: workspace-wide `mise run fmt-check` は Windows Orca worktree path length 依存の `os error 206`。成功条件は弱めず、package-scoped rustfmt exit 0 を補助証拠として併記する。
- NotRun（local）: macOS/Linux/3OS/MSRV、artifact-dependent synvelis-app L1/real ABI lane、GUI-L2/EXT-L3。3OS/MSRV は final branch CI target SHA `6187ce12907652f5c61980c80813fa8e76573bf1` の run `46125438476` で実走・successしたが、artifact-dependent synvelis-app L1/real ABI lane と GUI-L2/EXT-L3 は `#696` による分離後も本 slice の positive evidence に昇格させない。
- Unsupported: `routing_control.rs` 側の generation/epoch propagation は lease外。本 slice の Pass は driver/headless core semantics に限定する。
- Blocked: なし。

## 残存する注意点

- upstream `attach()` は contract の `InBlockTransportSupport` を現状 `Unknown` として構成する。本 slice は Synvelis 側 declaration/certification seam を変えず、capability unsupported/uncertified semantics を既存 coverage で確認するに留めた。ruviform pin・vendor・lockfile は変更していない。
- `RuntimeClockEpochRef` は routing-build identity であり、plugin scheduler `ClockEpochId` ではない。routing engine/control surface の restart propagation は lease外であり、本 evidence に Pass として計上しない。
- docs index は生成物かつ lease外なので更新していない。Coordinator が統合時に整備する。
