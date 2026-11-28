# Evidence — synvelis#127 TIM-03 core semantics（player fixture slice）

slice `tim03-core-semantics-s1`。Issue close ではなく `Refs #127` の前進 slice。対象は `synvelis-player` の fake/headless fixture が所有する core semanticsであり、real ABI plugin、production composition、synvelis-app L1 runner、GUI/EXT-L3、Plugin State Snapshot 本体は対象外。

machine-readable matrix: `evidence/sv127/core-semantics.jsonl`<br>
command/evidence logs: `evidence/sv127/core-semantics/`（`*.log` は gitignore 対象の raw evidence。永続サマリは本書と TSV）

## Anchor

- branch: `feat/127-tim03-core-semantics`
- base SHA during local verification: `3a7e3ed9b17aebb236acde9bdce125abc721cbe0`（検証開始時点の `origin/main` と一致。`git fetch origin main` exit 0、merge-base 確認済み）
- implementation / manual branch CI target SHA: `8395be14f659165041ffcf269a86c2c2340b44f7`（`feat/127-tim03-core-semantics` の implementation commit）
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

coordinator の completion-gate 指示により、`ci.yml`（`workflow_dispatch` 専用）を target SHA `8395be14f659165041ffcf269a86c2c2340b44f7` で手動 dispatch した。run は https://github.com/scarecrow173/synvelis/actions/runs/46122564406 であり、`ci-summary` の `verify-sha` step が `expected_sha == github.sha == checked-out HEAD` の3点一致を確認した。この evidence 追記 commit は Rust source/test semantics を変えない metadata-only の後続 commitであり、branch CI が検証した target SHA と混同しない。

run conclusion は **failure** であり、Pass として記録しない。decisive lane は `evidence`（required-context 互換の pr-mode gate）と `ci-summary`（macOS leg と `evidence` を含む Full CI verdict）で、両方とも failureだった。

| lane | conclusion | decisive/scope classification | 観測 |
| --- | --- | --- | --- |
| `dep-guard` | success | Pass（supporting） | ruviform dependency guard 成功 |
| `gw10-verify`（ubuntu/windows/macOS） | success ×3 | Pass（supporting） | GW-10 harness 3OS 成功 |
| `ui-verify`（ubuntu/windows）・`ui-verify-macos` | success ×3 | Pass（supporting） | UI check/test/build 系成功 |
| `verify (ubuntu-latest)` | failure | Fail / pre-existing out-of-scope infrastructure-harness | `mise run test-full` が `pc_c4_l1_vst3_real_abi_binding` / `pc_c4_l1_clap_real_abi_binding` で exit 101。両 test は `synvelis-plugin-test-*-l1-runner` artifact missing で panic |
| `verify (windows-latest)` | failure | Fail / pre-existing out-of-scope infrastructure-harness | Ubuntu と同じ `pc_c4_l1_*` artifact missing。Windows runner でも `target\ci\synvelis-plugin-test-*-l1-runner.exe` 不在 |
| `verify-macos` | failure | Fail / pre-existing out-of-scope infrastructure-harness | Ubuntu と同じ `pc_c4_l1_*` artifact missing。Full CI では required leg なので `ci-summary` failure に含まれる |
| `rt-verify` | failure | Fail / pre-existing out-of-scope infrastructure-harness | `mise run test-rt` が `vst3_instrument_transport_clicks` / `clap_instrument_transport_clicks` で exit 101。`target/debug/synvelis-plugin-test-*-l1-runner` artifact missing |
| `msrv-verify` | failure | Fail / pre-existing out-of-scope infrastructure-harness | Rust 1.89 build/test は `pc_c4_l1_*` artifact missing に到達するまで進行。MSRV compile failure は観測されなかったが、workspace test は `synvelis-app` failure で停止し、本 slice test 未到達のまま同じ artifact missing で exit 101 |
| `evidence` | failure | decisive lane failure | pr-mode gate が `verify (ubuntu-latest)`、`verify (windows-latest)`、`rt-verify`、`msrv-verify` を expected Pass / actual fail と照合 |
| `ci-summary` | failure | decisive lane failure | Full CI gate が上記4 lane、`verify-macos`、および `evidence` failure を expected Pass / actual fail と照合。SHA contract 自体は一致 |

### branch CI failure の因果切り分け

この failure は本 slice 起因ではないと判断した。根拠は次の通り。

1. 本 slice の diff は `synvelis-player` の driver/dispatch/process-boundary、lease 内 session-wiring test、および `evidence/sv127/` に限定される。失敗した test はいずれも lease外の artifact-dependent `synvelis-app` real ABI/L1 test で、panic message も product behavior ではなく runner executable 不在を示す。
2. base SHA `3a7e3ed9b17aebb236acde9bdce125abc721cbe0` に対する既存 main run https://github.com/scarecrow173/synvelis/actions/runs/46102592025 でも、本 sliceを含まない同一 base で `verify` 3OS・`rt-verify`・`msrv-verify`・`evidence`・`ci-summary` が同じ artifact missing 系 failure だった。
3. 現在の `origin/main` は `1ef341920cfd6b61ec4a27e54531f2c59611f6c6`（`fix(ci): isolate artifact-dependent L1 tests (#696)`）へ進んでおり、artifact-dependent L1 tests の CI 分離が別 slice で main へ入っている。target SHA `8395be14f659165041ffcf269a86c2c2340b44f7` はこの fix を祖先に含まない（`git merge-base --is-ancestor 1ef341920cfd6b61ec4a27e54531f2c59611f6c6 HEAD` exit 1）。
4. branch CI の workspace test log には `tim03_session_wiring` / SW-11 / SW-12 の実行行が出ていない。`synvelis-app` の artifact-dependent test failure で workspace test が先に停止するためであり、branch CI を player test の Pass evidence には計上しない。player test の Pass evidence は local canonical `mise run test synvelis-player` exit 0 のみに限定する。
5. 失敗は sv#655 で追跡される CI harness/artifact 前提の領域であり、本 lease では `.github/**`、`mise.toml`、`synvelis-app/**`、L1 runner を変更できない。恒久 workaround を player 側へ入れていない。

したがって branch CI は exact target SHA に対する honest negative evidence であり、slice の implementation/test failure ではない。統合時は `#696` を含む最新 main を取り込んだ head の再 dispatch、または merge 後 main SHA の CI evidence が必要である。この run の red を緑として扱わない。

## Review status

`review-protocol-equivalent blocker 0`（本 slice が所有する `synvelis-player` implementation/test/evidence 差分に残存 blocker はない）。ただし branch CI 自体は上記の lease外 artifact-dependent harness failure により red であり、この注意書きは success criteria を弱めるものではなく、統合時に必要な upstream CI fix の取り込みと再検証を明示するものである。


## Runtime Verification / Formal Verification

| verification class | verdict | basis |
| --- | --- | --- |
| Runtime Verification | Pass（local headless/player scope） | `mise run test synvelis-player` exit 0、`mise run test-rt` exit 0。`fmt-check` は infrastructure failureとして Pass に含めない |
| Formal Verification | NotRun | 本 slice は headless behavioral invariant の実装・検証であり、形式検証 artifacts/tactic は追加していない |

## Pass/Fail/NotRun/Unsupported/Blocked separation

- Pass: 上表で exit 0 と記録した local lane のみ。
- Fail: 本 slice 起因の未解消 code/test failure はなし。branch CI run `46122564406` は lease外 artifact-dependent L1 harness failure により red であり、Pass にしていない（詳細は Branch CI 節）。
- InfrastructureFailure: workspace-wide `mise run fmt-check` は Windows Orca worktree path length 依存の `os error 206`。成功条件は弱めず、package-scoped rustfmt exit 0 を補助証拠として併記する。
- NotRun（local）: macOS/Linux/3OS/MSRV、artifact-dependent synvelis-app L1/real ABI lane、GUI-L2/EXT-L3。理由は lease外または統合 CI evidence に委譲されるため。ただし manual branch CI run `46122564406` では 3OS/MSRV lane が対象 SHA で実走され、結果は Pass に昇格させず Branch CI 節へ記録した。
- Unsupported: `routing_control.rs` 側の generation/epoch propagation は lease外。本 slice の Pass は driver/headless core semantics に限定する。
- Blocked: なし。

## 残存する注意点

- upstream `attach()` は contract の `InBlockTransportSupport` を現状 `Unknown` として構成する。本 slice は Synvelis 側 declaration/certification seam を変えず、capability unsupported/uncertified semantics を既存 coverage で確認するに留めた。ruviform pin・vendor・lockfile は変更していない。
- `RuntimeClockEpochRef` は routing-build identity であり、plugin scheduler `ClockEpochId` ではない。routing engine/control surface の restart propagation は lease外であり、本 evidence に Pass として計上しない。
- docs index は生成物かつ lease外なので更新していない。Coordinator が統合時に整備する。
