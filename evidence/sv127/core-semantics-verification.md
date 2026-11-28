# Evidence — synvelis#127 TIM-03 core semantics（player fixture slice）

slice `tim03-core-semantics-s1`。Issue close ではなく `Refs #127` の前進 slice。対象は `synvelis-player` の fake/headless fixture が所有する core semanticsであり、real ABI plugin、production composition、synvelis-app L1 runner、GUI/EXT-L3、Plugin State Snapshot 本体は対象外。

machine-readable matrix: `evidence/sv127/core-semantics.jsonl`  
command/evidence logs: `evidence/sv127/core-semantics/`（`*.log` は gitignore 対象の raw evidence。永続サマリは本書と TSV）

## Anchor

- branch: `feat/127-tim03-core-semantics`
- base/target source SHA during local verification: `3a7e3ed9b17aebb236acde9bdce125abc721cbe0`（`origin/main` と一致。`git fetch origin main` exit 0、merge-base 確認済み）
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

## Runtime Verification / Formal Verification

| verification class | verdict | basis |
| --- | --- | --- |
| Runtime Verification | Pass（local headless/player scope） | `mise run test synvelis-player` exit 0、`mise run test-rt` exit 0。`fmt-check` は infrastructure failureとして Pass に含めない |
| Formal Verification | NotRun | 本 slice は headless behavioral invariant の実装・検証であり、形式検証 artifacts/tactic は追加していない |

## Pass/Fail/NotRun/Unsupported/Blocked separation

- Pass: 上表で exit 0 と記録した local lane のみ。
- Fail: 現時点で未解消の code/test failure なし。
- InfrastructureFailure: workspace-wide `mise run fmt-check` は Windows Orca worktree path length 依存の `os error 206`。成功条件は弱めず、package-scoped rustfmt exit 0 を補助証拠として併記する。
- NotRun: macOS/Linux/3OS/MSRV、artifact-dependent synvelis-app L1/real ABI lane、GUI-L2/EXT-L3。理由は lease外または統合 CI evidence に委譲されるため。
- Unsupported: `routing_control.rs` 側の generation/epoch propagation は lease外。本 slice の Pass は driver/headless core semantics に限定する。
- Blocked: なし。

## 残存する注意点

- upstream `attach()` は contract の `InBlockTransportSupport` を現状 `Unknown` として構成する。本 slice は Synvelis 側 declaration/certification seam を変えず、capability unsupported/uncertified semantics を既存 coverage で確認するに留めた。ruviform pin・vendor・lockfile は変更していない。
- `RuntimeClockEpochRef` は routing-build identity であり、plugin scheduler `ClockEpochId` ではない。routing engine/control surface の restart propagation は lease外であり、本 evidence に Pass として計上しない。
- docs index は生成物かつ lease外なので更新していない。Coordinator が統合時に整備する。
