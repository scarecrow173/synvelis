---
id: "TSPEC-DvNh4xQwTvsBbMkBb9PPmy"
type: "test-spec"
status: "approved"
title: "Target Architecture 設計レビュー検証仕様 — TA-001〜012 + GUI 方針の oracle・fixture・証拠分類"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "docs/specs/target-architecture.md"
    - "docs/designs/target-architecture-design.md"
  changes:
    added:
      - type: "file"
        path: "docs/test-specs/target-architecture.md"
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on:
    - "docs/specs/target-architecture.md"
    - "docs/designs/target-architecture-design.md"
    - "docs/plans/target-architecture.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/tasks/target-architecture-task.md"
    - "docs/impl/ir/target-architecture.md"
    - "docs/impl/exp/target-architecture.md"
    - "evidence/sv22/case-ledger.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/target-architecture-design.md"
    - "docs/specs/target-architecture.md"
  derived-by: []
  verifies:
    - "docs/specs/target-architecture.md"
  verified-by: []
  references:
    - "evidence/sv22/case-ledger.md"
    - "evidence/sv22/dependency-dag.md"
  defers: []
  deferred-by: []
---

# Target Architecture 設計レビュー検証仕様

本 test-spec は spec `docs/specs/target-architecture.md` の
R-TA-01〜14 を検証する oracle・fixture・証拠分類を固定する。
対象は **設計レビューの証拠** であり、コードの unit/integration
テスト仕様ではない。実走していない項目を Pass にしない
（spec の Evidence 分類ルールに従う）。

## Feature SV22-F01: TA-001〜012 設計レビュー + GUI 方針

### Rule SV22-F-TA-01（TA-001/SV22-C01 DAG 非循環）

- **Given** Synvelis workspace と Ruviform resolved rev
  `fd1d4d5` の `cargo metadata --format-version 1 --locked`
  resolve グラフ
- **When** 通常依存（dev/build 以外）辺のみで DFS cycle 検査
- **Then** Synvelis subgraph・Ruviform 全体とも cycle = 0。
  dev/optional/feature/target 辺は分類して `dependency-dag.md`
  に記録。`mise run check-ruviform-rev`・`mise run
  check-headless-deps` が exit 0。
- **Oracle**: `evidence/sv22/dependency-dag.md`・case-ledger C01。

### Rule SV22-F-TA-02（TA-002/SV22-C02 型漏出禁止）

- **Given** resolved rev `fd1d4d5` の Ruviform `src/` 全域
- **When** `synvelis` および DAW 意味論型名を走査
- **Then** Synvelis crate 依存・Project/Track/Take/Comp/
  Automation/Undo/RenderRecipe 型の公開 API 侵入がない。
  `synvelis_dark` theme fixture・doc 例示は DAW 型ではない
  観測として分離記録。
- **Oracle**: case-ledger C02・design §3。

### Rule SV22-F-TA-03（TA-003/SV22-C03 Device/Host 非依存 Graph）

- **Given** `ruviform-graph` の通常依存集合と `tests/poc_ex.rs`
- **When** 依存を確認し fixture 存在を pin
- **Then** 通常依存は `ruviform-audio-core` のみ、device/
  実 plugin host 非依存。`poc_ex_001`〜`006` が構築・検証・
  失敗・revision mapping をカバー。実走は NotRun（先行参照）。
- **Oracle**: case-ledger C03・design §4。

### Rule SV22-F-TA-04（TA-004/SV22-C04 Fan-in/Fan-out/Sidechain/PDC/拒否）

- **Given** `ruviform-graph` compiler fixtures と
  `ruviform-audio-plugin-bridge` bus fixture
- **When** 各契約要素の実在を pin
- **Then** Fan-in scratch slot・channel map・宣言 delay 拒否・
  PDC 補償・`PortRole::Sidechain`・`ValidationError` typed
  variants（InvalidDirection/DomainMismatch/FormatMismatch/
  ImplicitConversionRequired/InvalidChannelMap/Missing*/
  PortNotOnNode/Cycle/Multiplicity）が実在。
- **Oracle**: case-ledger C04・design §5。

### Rule SV22-F-TA-05（TA-005/SV22-C05 PluginHost↔Engine 境界）

- **Given** resolve グラフの plugin-host/audio/bridge 辺
- **When** 通常依存と dev-dep を分類
- **Then** plugin-host→audio は dev-dep のみ、audio→
  plugin-host/bridge なし、bridge→両者が正方向。通常依存
  循環なし。
- **Oracle**: case-ledger C05・`dependency-dag.md`・design §6。

### Rule SV22-F-TA-06（TA-006/SV22-C06 単一 RT Owner）

- **Given** `ruviform-audio::typed::runtime` と
  `typed_activation.rs` fixtures
- **When** generation-tagged SPSC・active_generation・
  install/remove 契約・`g2_04_*`/`g2_05_*` fixture を pin
- **Then** RT 実行 Owner が単一で、二重 Process/所有が
  generation 契約と fixture で拒否される構造が実在。
- **Oracle**: case-ledger C06・design §7。

### Rule SV22-F-TA-07（TA-007/SV22-C07 RT 禁止事項）

- **Given** `rt_*` fixture 群・`assert_no_alloc`・
  `ruviform-rt` retire/spsc/alloc_counter
- **When** alloc/blocking/危険 Drop 禁止の契約を pin
- **Then** deferred destruction + collector drain +
  saturation leak-safe flag が実在。検証不能経路は Pass
  にしない（NotRun/先行参照を明記）。
- **Oracle**: case-ledger C07・design §8。

### Rule SV22-F-TA-08（TA-008/SV22-C08 Generation/時点契約）

- **Given** `ruviform-media-core::transport_schedule` 型群・
  `ruviform-graph::project` revision・`typed::runtime` SPSC
- **When** Plan/Target/Event Generation と
  Commit/Applied/Observed の型を pin
- **Then** `ClockEpochId`/`TransportGeneration`/`PlanGeneration`/
  `TempoMapRevision`/`LoopOccurrence`/`CommandId`/`EngineFrame`/
  `AppliedReceipt`/`ApplyStatus`/`RejectReason`/`ApplyDegradation`/
  `DeliveryPolicy`/`ScheduleBudget`/`InBlockTransportSupport` と
  stale generation 拒否 fixture が実在。
- **Oracle**: case-ledger C08・design §9。

### Rule SV22-F-TA-09（TA-009/SV22-C09 Musical Time→Sample Event）

- **Given** `ruviform-media-core::music` 型群・transport_schedule
  span scheduling・Synvelis sv19 chain
- **When** Variable PPQN/Rational Beat/Transport Span→Sample
  Event 精密変換・量子化誤差・Loop/Seek 区別を pin
- **Then** `Ppqn`/`BeatPosition`/`TickPosition`/`TempoMap`・
  `variable_ppqn_media_roundtrip`/`beat_position_*`/
  `tick_position_*`/`conversion_overflow_is_explicit`・
  Loop/Seek の epoch/generation 区別 fixture が実在。
  PPQN 範囲差（Ruviform 65535 vs Synvelis 32767）は
  部分集合で整合として記録。
- **Oracle**: case-ledger C09・design §10。

### Rule SV22-F-TA-10（TA-010/SV22-C10 WAV 実 Encode/Finalize/Reopen）

- **Given** `ruviform-io-wav` `tests/roundtrip.rs`・
  `tests/contract.rs`
- **When** Encode→Finalize→Reopen→別 Reader Decode と
  Drop best-effort の区別を pin
- **Then** `roundtrip_*`・`staged_seal_readback_publish_e2e`・
  `sink_error_at_finalize_is_io_not_completion`・
  `staged_drop_*` が実在し、Drop は完了証拠でないことが
  契約として固定される。
- **Oracle**: case-ledger C10・design §11。

### Rule SV22-F-TA-11（TA-011/SV22-C11 Window Close≠Job Cancel）

- **Given** `ruviform-ui` multi_window/runtime tests・
  `synvelis-app` close_veto/shell・sv-gui-04-02 spec
- **When** GUI・Headless 両側で Window Close と Job Cancel の
  区別を pin
- **Then** `close_requested_does_not_cancel_jobs`・
  `subwindow_close_leaves_jobs_and_loop_running`・
  `job_events_route_to_owner_window`・opt-in
  `cancel_window_jobs`・synvelis close_guard veto・
  `Destroyed`→`cancel_window_jobs` 明示呼出が実在。
  legacy 経路（close=終了）を区別記録。
- **Oracle**: case-ledger C11・design §12。

### Rule SV22-F-TA-12（TA-012/SV22-C12 Consumer 互換）

- **Given** Ruviform consumer crate 棚卸し・Synvelis crates・
  AudioLens 別 repo
- **When** Audio/Video/Plugin/CLI/UI の責務・回帰 Suite・
  NotRun 根拠を分離記録
- **Then** 各 consumer が別行で記録され、AudioLens は
  #219/#294 の独立 Oracle/3repo SHA tuple に Deferred 接続。
- **Oracle**: case-ledger C12・design §13。

### Rule SV22-F-TA-13（SV22-C13 GUI 採用方針整合）

- **Given** `sv175` ADR（accepted）・`sv-gui-01` ADR
  （superseded）・Ruviform #384 ADR（proposed）・
  `synvelis-app` native feature・`check-headless-deps`
- **When** 採用方針の整合と GPUI 型非露出を照合
- **Then** #175/#384 の方向一致・旧 ADR 歴史保持・GPUI が
  composition root のみ・10 headless crate に GUI 非含有
  （check-headless-deps Pass）・#384 最終採用は
  #262/#388/#389 後であることが記録される。
- **Oracle**: case-ledger C13・design §14。

### Rule SV22-F-TA-14（SV22-C14 Native Dock Gate 分離）

- **Given** #384 Native Dock 採用 Gate・#388/#389・
  Synvelis #176・#262・`ruviform-poc-native-dock`・
  `sv-gui-03-05` simulated lane
- **When** Floating/Generic/Mock/GPUI-simulated の成功を
  最終採用 PASS にしないことを確認
- **Then** 実 nativeEditor が GPUI メイン Dock 管理下で
  動作することが PASS 定義、matrix は #388、製品 E2E は
  #176、採否は #262 に Deferred 接続される。
- **Oracle**: case-ledger C14・design §15。

## 非対象（この test-spec が保証しないこと）

- ruviform fixture の実走結果（本 slice は pin/参照のみ）
- 実 OS Device/実 Plugin/実 Native Dock/実 GUI の受入
- merge 後対象 SHA の CI 統合検証
- AudioLens 側の互換証拠
