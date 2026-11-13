---
id: "SPEC-l4FdiimUNULuodZsTBZq0k"
type: "spec"
status: "approved"
title: "Target Architecture — Synvelis↔Ruviform 共通契約・Production Dependency DAG・GUI 採用方針の設計レビュー契約（synvelis#22）"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/22"
  changes:
    added:
      - type: "file"
        path: "docs/specs/target-architecture.md"
      - type: "file"
        path: "docs/designs/target-architecture-design.md"
      - type: "file"
        path: "docs/plans/target-architecture.md"
      - type: "file"
        path: "docs/tasks/target-architecture-task.md"
      - type: "file"
        path: "docs/test-specs/target-architecture.md"
      - type: "file"
        path: "docs/impl/ir/target-architecture.md"
      - type: "file"
        path: "docs/impl/exp/target-architecture.md"
      - type: "dir"
        path: "evidence/sv22"
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on:
    - "docs/designs/synvelis-driven-ruviform-target-architecture-v07.md"
    - "docs/specs/playback-graph.md"
    - "docs/specs/project-model.md"
    - "docs/specs/project-persistence.md"
    - "docs/specs/recording-take-comping.md"
    - "docs/specs/musical-time.md"
    - "docs/specs/sv-gpui-01-native-app-shell.md"
    - "docs/specs/sv-gui-04-02-close-veto.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/target-architecture-design.md"
    - "docs/plans/target-architecture.md"
    - "docs/tasks/target-architecture-task.md"
    - "docs/test-specs/target-architecture.md"
    - "docs/impl/ir/target-architecture.md"
    - "docs/impl/exp/target-architecture.md"
    - "evidence/sv22/case-ledger.md"
    - "evidence/sv22/dependency-dag.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/synvelis-driven-ruviform-target-architecture-v07.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/target-architecture.md"
  references:
    - "evidence/sv22/case-ledger.md"
    - "evidence/sv22/dependency-dag.md"
    - "evidence/sv19/case-ledger.md"
    - "evidence/sv18/playback-graph-case-ledger.md"
    - "evidence/sv17/case-ledger.md"
    - "https://github.com/scarecrow173/synvelis/issues/17"
    - "https://github.com/scarecrow173/synvelis/issues/18"
    - "https://github.com/scarecrow173/synvelis/issues/19"
    - "https://github.com/scarecrow173/synvelis/issues/20"
    - "https://github.com/scarecrow173/synvelis/issues/21"
    - "https://github.com/scarecrow173/synvelis/issues/23"
    - "https://github.com/scarecrow173/synvelis/issues/83"
    - "https://github.com/scarecrow173/synvelis/issues/110"
    - "https://github.com/scarecrow173/synvelis/issues/113"
    - "https://github.com/scarecrow173/synvelis/issues/114"
    - "https://github.com/scarecrow173/synvelis/issues/175"
    - "https://github.com/scarecrow173/synvelis/issues/176"
    - "https://github.com/scarecrow173/synvelis/issues/177"
    - "https://github.com/scarecrow173/synvelis/issues/230"
    - "https://github.com/scarecrow173/synvelis/issues/294"
    - "https://github.com/scarecrow173/ruviform/issues/184"
    - "https://github.com/scarecrow173/ruviform/issues/186"
    - "https://github.com/scarecrow173/ruviform/issues/187"
    - "https://github.com/scarecrow173/ruviform/issues/188"
    - "https://github.com/scarecrow173/ruviform/issues/189"
    - "https://github.com/scarecrow173/ruviform/issues/190"
    - "https://github.com/scarecrow173/ruviform/issues/191"
    - "https://github.com/scarecrow173/ruviform/issues/192"
    - "https://github.com/scarecrow173/ruviform/issues/193"
    - "https://github.com/scarecrow173/ruviform/issues/219"
    - "https://github.com/scarecrow173/ruviform/issues/257"
    - "https://github.com/scarecrow173/ruviform/issues/262"
    - "https://github.com/scarecrow173/ruviform/issues/294"
    - "https://github.com/scarecrow173/ruviform/issues/384"
    - "https://github.com/scarecrow173/ruviform/issues/385"
    - "https://github.com/scarecrow173/ruviform/issues/386"
    - "https://github.com/scarecrow173/ruviform/issues/387"
    - "https://github.com/scarecrow173/ruviform/issues/388"
    - "https://github.com/scarecrow173/ruviform/issues/389"
    - "https://github.com/scarecrow173/ruviform/issues/390"
  defers:
    - "https://github.com/scarecrow173/ruviform/issues/192"
    - "https://github.com/scarecrow173/ruviform/issues/193"
    - "https://github.com/scarecrow173/ruviform/issues/219"
    - "https://github.com/scarecrow173/ruviform/issues/262"
    - "https://github.com/scarecrow173/ruviform/issues/294"
    - "https://github.com/scarecrow173/ruviform/issues/384"
    - "https://github.com/scarecrow173/ruviform/issues/388"
    - "https://github.com/scarecrow173/ruviform/issues/389"
    - "https://github.com/scarecrow173/synvelis/issues/23"
    - "https://github.com/scarecrow173/synvelis/issues/83"
    - "https://github.com/scarecrow173/synvelis/issues/114"
    - "https://github.com/scarecrow173/synvelis/issues/175"
    - "https://github.com/scarecrow173/synvelis/issues/176"
    - "https://github.com/scarecrow173/synvelis/issues/177"
    - "https://github.com/scarecrow173/synvelis/issues/230"
    - "https://github.com/scarecrow173/synvelis/issues/294"
    - "evidence/sv22/case-ledger.md"
    - "evidence/sv22/dependency-dag.md"
  deferred-by: []
---

# Target Architecture — Synvelis↔Ruviform 共通契約・Production Dependency DAG・GUI 採用方針の設計レビュー契約（synvelis#22）

## What Should Be Built

Synvelis Issue #22「Synvelis-driven Ruviform Target
Architecture v0.7.1 — Ownership・DAG・API・Migration」の
設計・ADR Issue として、承認済み Target Architecture
（`docs/designs/synvelis-driven-ruviform-target-architecture-v07.md`、
以下 v0.7.1）の **TA-001〜TA-012 受入条件と GUI 採用方針
2 条件を、現行コード・依存 DAG・参照 Issue と突合する設計
レビュー** を `docs/` の文書チェーンと `evidence/sv22/`
の検証台帳として固定する。

対象は **設計レビューと証拠分類の正本化** であり、コード
実装・Ruviform 共通基盤の変更・旧 WebView ADR の再承認・
GPUI/実 Plugin OS Gate の完了宣言は行わない。実装は
機能ごとの Repo/Issue（Ruviform #184〜#193・#219・#294・
#384〜#390、Synvelis #23・#83・#110〜#114・#175〜#177 等）
へ委譲済みである。

本 spec が固定するもの:

1. **Owner/境界契約** — Ruviform = Generic Time/Graph/IO/
   Plugin Host/GUI Driver-neutral/Observability/Gateway 共通、
   Synvelis = DAW Project/MIDI 編集/Signal Flow/Recording
   UX/Native Plugin Panel、AudioLens = f64 解析/PTS/CLI/Job。
   Ruviform は両 Consumer を独立 Oracle として候補
   SHA/Feature で照合し、採用要求が片方に回帰を強制しない。
2. **Production Dependency DAG 契約** — 通常 Cargo 依存
   DAG の非循環性を、Dev/Optional/Feature/Target 別に
   分類した再現可能なコマンドと対象 Revision で記録する。
   `ruviform-plugin-host` → `ruviform-audio` は dev-dep の
   みで通常依存ではない（v0.7.1 §2 の訂正済み事実を維持）。
3. **型漏出禁止契約** — Ruviform 公開 API に Synvelis
   専用 Project/Track/Take 等の DAW 意味論型が侵入しない。
   共通契約（Graph/Time/Event/Job/Probe）は Consumer 非依存
   で独立利用可能でなければならない。
4. **検証分離契約** — Mock/Headless/Generic/Floating の
   成功を実 OS Device/実 Plugin/実 Native Dock/実 GUI の
   受入と混同しない。`Required`/`Deferred`/
   `NotApplicable`（要求状態）と `Pass`/`Fail`/`NotRun`/
   `Unsupported`（実行結果）を分離する（`POLICIES.md`
   §2/§3 準拠）。
5. **GUI 採用方針契約** — 現行 GUI 採用方針は Ruviform
   #384（Driver-neutral 契約層 + GPUI 標準 Driver・仮決定）
   と Synvelis #175 後継 ADR
   （`docs/adr/sv175-native-shell-driver-neutral-gpui.md`、
   accepted）に整合する。旧 WebView/SolidJS ADR
   （`docs/adr/sv-gui-01-...`）は `superseded` として歴史
   保持し、再承認しない。通常画面へ GPUI 型を露出しない。
6. **Native Plugin GUI Dock 契約** — Native Plugin GUI
   Dock は Ruviform #388（NativeSurfaceSlot）/#389
   （PluginEditorSession）・Synvelis #176（製品 E2E）の
   実 OS/Plugin 受入に接続する。Floating/Generic/Mock
   だけで GPUI 最終採用を PASS としない（#384 Native
   Dock 採用 Gate・#262 採否判定に従う）。

## Requirements

### R-TA-01 Production Dependency DAG 非循環（TA-001）

- Synvelis Workspace + Ruviform（`Cargo.lock` resolved
  rev）の **通常依存** DAG に循環がないことを、
  `cargo metadata --format-version 1 --locked` の resolve
  グラフに対する決定的 cycle 検査で記録する。
- Dev-dependency・optional dependency・feature・
  target-specific の各辺を分類して明示する
  （dev-dep の循環は Cargo が許容するが、通常依存の
  循環は許容しない）。
- `check-headless-deps`（GUI framework 混入 guard）と
  `check-ruviform-rev`（lock/vendor/manifest の rev 一致）
  を対象 SHA で実行し結果を記録する。

### R-TA-02 型漏出禁止（TA-002）

- Ruviform 公開 API（`pub` item）に Synvelis 専用
  Project/Track/Take/Comp/Automation Write/Undo/Render
  Recipe/Asset Provider 意味論の型が存在しないことを、
  resolved rev のソース走査で確認する。
- 「synvelis」の文字列出現は consumer 名を冠した
  theme/fixture データや doc 例示であり、DAW 意味論型の
  侵入とは区別して記録する（観測事項として分離）。
- 共通契約が AudioLens 等の他 Consumer から独立利用可能
  であることは依存方向（Consumer → Ruviform の一方向）
  で担保される — Ruviform が Consumer crate に依存しない
  ことを DAG で確認する。

### R-TA-03 Device/Host 非依存 Graph 検証（TA-003）

- `ruviform-graph`（Typed Graph IR/Validator）が
  OS Device・実 Plugin Host なしで構築・検証・失敗診断
  できることを、依存集合（通常依存 = `ruviform-audio-core`
  のみ）と fixture（`tests/poc_ex.rs`）で確認する。
- Graph 検証の失敗は `ValidationError`/`ValidationErrors`
  の typed 診断として顕在化し、暗黙成功しない。

### R-TA-04 Fan-in/Fan-out・Sidechain・PDC・無効拒否（TA-004）

- Typed Fan-in（merge-point scratch slot）・channel map
  結合・宣言 delay の拒否・PDC（latency 差の merge 点
  補償）・`PortRole::Sidechain` の port role 表現・
  plugin bridge の Main+Aux/Sidechain bus fixture を
  突合する。
- 無効 Port/Channel（MissingNode/MissingPort/
  PortNotOnNode/InvalidDirection/DomainMismatch/
  FormatMismatch/ImplicitConversionRequired/
  InvalidChannelMap/Multiplicity/Cycle）は Validator の
  typed `ValidationError` で拒否・診断される。

### R-TA-05 PluginHost↔AudioEngine 境界（TA-005）

- `ruviform-plugin-host` の通常依存に `ruviform-audio`
  （Audio Engine 実装）がなく、`ruviform-audio` の通常
  依存に `ruviform-plugin-host`/`ruviform-audio-plugin-bridge`
  がないことを依存 DAG で確認する。
- `ruviform-plugin-host` → `ruviform-audio` の
  dev-dependency（`tests/plugin_graph_e2e.rs` の AC3
  契約: endpoint→node→Graph 接着は host 側からのみ試験）
  は Cargo が許容する test 専用辺であり、通常依存の
  循環ではないことを明示分類する。
- 接着は `ruviform-audio-plugin-bridge`（Engine/Host 双方
  への正方向依存）が担う設計と一致することを確認する。

### R-TA-06 単一 RT Owner（TA-006）

- Stateful Processor/Graph の RT 実行 Owner が単一で
  あることを、`ruviform-audio::typed` の runtime
  （generation-tagged SPSC command・`active_generation`
  AtomicU64・install/remove は plan generation 一致時のみ
  適用）と activation fixture（stable slot vs plan-owned
  の比較・`g2_04_*`/`g2_05_*`）で確認する。
- 旧/新 Plan での二重 Process・二重所有が typed 契約と
  fixture で拒否されることを記録する
  （`g2_05_stale_generation_events_rejected`・
  `install_rejects_same_generation_different_plan`・
  `g2_04_rt_runtime_is_send_not_sync` 等）。

### R-TA-07 RT 禁止事項（TA-007）

- RT コールバックの allocation/blocking/危険な Drop が
  禁止されていることを、`rt_*` fixture
  （`rt_typed_process_no_alloc`・`rt_run_callback_no_alloc`・
  `rt_device_restart_no_alloc`・`rt_queries_no_alloc`・
  Synvelis 側 `rt_*` suite — `mise run test-rt` =
  `cargo test --workspace --locked rt_`）と
  `assert_no_alloc` 計測・`ruviform-rt` の retire/
  collector（deferred destruction・saturation leak-safe
  flag）で確認する。
- **検証不能な経路を PASS にしない**: 本 slice は
  fixture の存在をソース pin し、実測は先行 slice/CI の
  記録を SHA・環境付きで参照する。本 slice で実走して
  いない項目は `NotRun` と記録する。

### R-TA-08 Generation/時点契約（TA-008）

- Graph Plan/Target/Event の Generation および
  Commit/Applied/Observed の時点が型付き契約で区別
  されることを、`ruviform-media-core::transport_schedule`
  （`ClockEpochId`/`TransportGeneration`/`PlanGeneration`/
  `TempoMapRevision`/`LoopOccurrence`/`CommandId`/
  `EngineFrame`/`AppliedReceipt`/`ApplyStatus`/
  `RejectReason`/`ApplyDegradation`/`DeliveryPolicy`/
  `ScheduleBudget`/`InBlockTransportSupport`）と
  `ruviform-graph::GraphProject`（requested vs published
  revision）・`ruviform-audio::typed::runtime`（plan
  generation の SPSC 適用）で確認する。
- stale generation event が配送されず fault として顕在化
  すること（`stale_transport_generation_event_is_faulted_not_delivered`・
  `g2_05_stale_generation_events_rejected`）を記録する。

### R-TA-09 Musical Time → Sample Event（TA-009）

- Variable PPQN/Rational Beat/Transport Span が Sample
  Event 位置へ精密変換されることを、`ruviform-media-core::music`
  （`Ppqn` validated `1..=u16::MAX`・`PPQN=960` 互換定数・
  `BeatPosition` 正規化有理数・`TickPosition` resolution
  結合・`TempoMap`）と `transport_schedule` の span
  scheduling（loop wrap・occurrence・same-frame source
  order・min_frames violation の exact-wrap/degraded
  区別）で確認する。
- 量子化誤差は `Error::TimeOverflow`/明示 policy で
  顕在化し、Loop/Seek は `loop_wrap_splits_at_end`・
  `loop_events_fire_once_per_occurrence`・
  `play_then_seek_receipt_lifecycle`・
  `device_restart_vs_seek_distinct_epoch_generation` で
  区別検証される。Synvelis 側は sv19 chain
  （`tim_01_ppqn`・`tim_02_smf`・`tim02_midi_playback`）が
  consumer 契約を被覆する。

### R-TA-10 WAV 実 Encode/Finalize/Reopen（TA-010）

- 実 WAV 出力が Encode→Finalize→Reopen→別 Reader Decode
  で検証されることを、`ruviform-io-wav` の
  `tests/roundtrip.rs`（`roundtrip_*` — write→decode→
  sample 比較）と `tests/contract.rs`
  （`staged_seal_readback_publish_e2e`・
  `sink_error_at_finalize_is_io_not_completion`・
  `staged_drop_after_write_preserves_destination`・
  `write_after_finish_and_double_finish_error`）で確認する。
- Drop 時の best-effort（`staged_drop_after_write_*`）は
  完了証拠としない — Finalize の明示的成功と Reopen
  検証が完了条件であることを契約として固定する。

### R-TA-11 Window Close ≠ Job Cancel（TA-011）

- Window Close が Project/Application 所有 Job を暗黙
  Cancel しないことを、`ruviform-ui` の driver-neutral
  multi-window 経路（`close_requested_does_not_cancel_jobs`・
  `subwindow_close_leaves_jobs_and_loop_running`・
  `job_events_route_to_owner_window`・opt-in
  `UiRuntime::cancel_window_jobs`）と Synvelis 側
  （`close_veto.rs` の `syn.window.close_guard` decision
  query・veto fixture・`shell.rs` の `Destroyed`→
  `cancel_window_jobs` 明示呼出）で GUI・Headless 両側
  から確認する。
- legacy 単一 window 経路（`HostEvent::CloseRequested`→
  `close_requested_cancels_jobs_and_emits_lifecycle`）は
  close=アプリ終了の意味論であり、multi-window driver
  経路と区別して記録する。

### R-TA-12 Consumer 互換（TA-012）

- Audio/Video/Plugin/CLI/UI の各 Consumer 互換性を、
  別々の責務・回帰 Suite・NotRun 根拠で記録する。
- Ruviform 側 Consumer 面: `ruviform-cli-sample`（CLI）・
  `ruviform-edit`（NLE）・`ruviform-video*`（Video）・
  `ruviform-plugin-*`（Plugin Host/format/export）・
  `ruviform-ui*`/`ruviform-viz`（UI）・
  `ruviform-poc-native-dock`（Native Dock PoC）。
- AudioLens は別 repo の独立 Consumer であり、本 slice
  では Synvelis 側証拠のみを記録する。両 Consumer の
  独立 Oracle と 3repo SHA tuple の提出は Ruviform
  #219/#294 の責務として `defers` で接続する。

### R-TA-13 GUI 採用方針整合

- 現行 GUI 採用方針が Ruviform #384（Driver-neutral 契約層
  + GPUI 標準 Driver・status: proposed の仮決定）と
  Synvelis #175 後継 ADR（`sv175-native-shell-driver-neutral-gpui.md`、
  status: accepted）に整合することを文書照合で確認する。
- 旧 WebView/SolidJS ADR（`sv-gui-01-...`）は
  `status: superseded` + `superseded-by` リンクで歴史
  保持されており、本 Issue で再承認・復活させない。
- 通常画面（Synvelis の View/Domain/Headless）へ GPUI 型を
  露出しないことを、`synvelis-app` の `native` feature
  分離（GPUI 進入は composition root のみ）と
  `check-headless-deps`（10 headless crate に GUI framework
  非含有）の実測で確認する。
- #384 ADR は「契約設計の開始」と「GPUI 技術採用の最終
  承認」を分離しており、最終承認は #262/#388/#389 の実機
  証拠後 — 本レビューは採用方針の**整合**を確認するので
  あって、GPUI 最終採用の PASS 宣言ではない。

### R-TA-14 Native Plugin GUI Dock Gate 分離

- Native Plugin GUI Dock は Ruviform #388
  （NativeSurfaceSlot）/#389（PluginEditorSession）・
  Synvelis #176（製品 E2E 実機受入）に接続する。
- Floating/Generic/Mock・GPUI-simulated scenario
  （`sv-gui-03-05` lane）・headless fixture の成功は
  実 Native Dock 受入の代用にしない。#384 Native Dock
  採用 Gate（実 nativeEditor が GPUI メイン Dock の
  layout/clip/input/focus 管理下で動作）と #262 採否
  判定を `defers` で接続する。

## 受入条件写像（Issue #22 転記 → Case ID）

| Case | 受入条件 | 検証の形態 |
| --- | --- | --- |
| SV22-C01 | TA-001: Cargo production dependency DAG を検査し循環なしを再現可能なコマンド・対象 Revision とともに記録 | 実コマンド実行（cargo metadata + cycle 検査） |
| SV22-C02 | TA-002: Ruviform 公開 API に Synvelis 専用型が侵入せず共通契約の独立利用を型検査 | resolved rev ソース走査 + DAG 方向確認 |
| SV22-C03 | TA-003: Mock/Headless で Generic Graph の構築・実行・失敗を検証 | 依存集合 + fixture ソース pin（実走は先行 slice 参照） |
| SV22-C04 | TA-004: Typed Fan-in/Fan-out・Sidechain・PDC・無効 Port/Channel 拒否・診断 | fixture ソース pin |
| SV22-C05 | TA-005: PluginHost→Audio Engine 不要な通常依存なしを Cargo DAG で検証 | 依存 DAG 実測（cargo metadata） |
| SV22-C06 | TA-006: RT 実行 Owner 単一・二重 Process/所有をテストで拒否 | fixture ソース pin |
| SV22-C07 | TA-007: RT alloc/block/危険 Drop 禁止・検証不能経路を PASS にしない | fixture ソース pin + rt_* suite 参照 |
| SV22-C08 | TA-008: Generation/Commit/Applied/Observed の時点を型付き契約とテストで区別 | 型・fixture ソース pin |
| SV22-C09 | TA-009: Variable PPQN/Rational Beat/Transport Span→Sample Event 精密変換・量子化誤差・Loop/Seek 区別 | fixture ソース pin（ruviform + synvelis 両層） |
| SV22-C10 | TA-010: 実 WAV Encode→Finalize→Reopen→別 Reader Decode・Drop best-effort を完了証拠としない | fixture ソース pin |
| SV22-C11 | TA-011: Window Close≠Project/Job 暗黙 Cancel・Window 所有 Job との区別を GUI・Headless 両側で検証 | fixture ソース pin（ruviform-ui + synvelis close_veto） |
| SV22-C12 | TA-012: Audio/Video/Plugin/CLI/UI Consumer 互換を別責務・回帰 Suite・NotRun 根拠で記録 | crate 棚卸し + Issue 接続記録 |
| SV22-C13 | GUI 採用方針は #384 GPUI 標準 Driver + #175 後継 ADR に整合・旧 WebView ADR は歴史保持・通常画面へ GPUI 型非露出 | ADR/依存 guard 文書照合 + check-headless-deps 実測 |
| SV22-C14 | Native Plugin GUI Dock は #388/#389/#176 実 OS/Plugin 受入に接続・Floating/Generic/Mock のみで GPUI 最終採用を PASS としない | Gate 接続記録（実機は Deferred） |

## Out of Scope / Deferred

- **コード実装・crate 変更** — TA-001〜012 の実装側
  ギャップは各 Owner Issue（Ruviform #184〜#193・
  Synvelis #23 等）へ委譲。本 slice はレビューと
  証拠分類のみ。
- **Ruviform 共通基盤の実装変更** — 別 repo・別 Issue
  の責務。`F:/repositry/audio-products/ruviform` は
  読み取り参照のみ。
- **AudioLens 製品コード・受入** — 別 repo の独立
  Consumer。#219/#294 への独立 Oracle/3repo SHA tuple
  提出は Ruviform 側 Owner の責務。
- **旧 WebView/SolidJS ADR の再承認・GPUI/実 Plugin OS
  Gate の完了宣言** — 本 Issue で行わない。#384 ADR の
  最終採否は #262/#388/#389 の実機証拠後。
- **`Cargo.toml`/`Cargo.lock`/`mise.toml`/CI/本番
  crate** — 設計レビュー範囲外。変更が必要な場合は
  実装 Owner の独立 PR。
- **sv#20/#21 の docs ファイル** — 並行 Worker の占有
  範囲。参照のみで編集・作成しない。
- **merge 後対象 SHA の CI 証拠** — 本 slice は
  docs/evidence のみ。統合受入は Coordinator が対象
  SHA CI で確認。

## Evidence 分類ルール

- `Required` = 本契約で要求される項目。`Deferred` =
  別 Owner/将来 lane。`NotApplicable` = 本 slice 範囲外。
- `Pass` = 本 slice で実際に実行した検証
  （`cargo metadata` cycle 検査・`mise run fmt-check`・
  `mise run check-ruviform-rev`・`mise run
  check-headless-deps`・文書照合）の成功、または
  先行 slice が SHA/環境付きで記録した実測の参照
  （その旨を明記）。
- `NotRun` = fixture/契約は存在するが本 slice で実行
  していない（先行実測を参照する場合はその旨を明記）、
  または構成済みテストがない契約。
- `Unsupported` = 環境上利用不可、または明示的対象外。
- Mock/fake/headless/Generic/Floating の PASS は
  contract 検証の証拠であり、実 OS Device・実 Plugin・
  実 Native Dock・実 GUI の証明ではない。
