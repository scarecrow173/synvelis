# Acceptance Ledger — synvelis#293 RCP-04 semantic→selection integration slice

Date: 2026-11-16. Slice: `synvelis-workspace` の semantic
project/routing model → RCP-04 pin 済み入力
（`ResolvedGraphSnapshot`/`PluginCapabilitySnapshot`/
`RenderCandidateIndex`）→ `ExecutionSelector` →
`PlanReconciler` → `RoutingEngine` prepare/publish の headless
配線（`synvelis-workspace::rcp_selection`）。実 Plugin/OS・
PCM・RT・app adapter（`SessionRoutingEngine`）・
BranchAudition/provenance は対象外。

対象 Issue: https://github.com/scarecrow173/synvelis/issues/293
（OPEN・`Refs` のみ・Close しない — RCP-EX-06 実機分は
synvelis#296 RCP-GATE の残件）。

## 計測環境

| 項目 | 値 |
| --- | --- |
| Synvelis SHA | `feat/293-semantic-selection-integration` HEAD（ledger は同 commit 内） |
| base | `origin/main` `0b57299` |
| OS / CPU | Windows 11 |
| Toolchain | mise 管理 Rust（`mise run`/`mise exec` 経由） |
| Project schema | 既存 schema を消費（変更なし） |
| Recipe/artifact/policy schema | RCP-01..RCP-03・lookup 契約を消費（変更なし） |

## 判定語彙

- **Pass (headless fixture)**: `crates/synvelis-workspace/tests/
  rcp_selection_integration.rs` 10 case が `mise run test
  synvelis-workspace` で green。
- **NotRun**: 実 Plugin/実 OS/PCM/RT・RCP-EX-06 実機 Gate・
  app adapter 経路・BranchAudition/provenance — 本 slice では
  実行していない。
- **Deferred**: `SessionRoutingEngine`（`synvelis-app`）への
  配線（lease 外）、BranchAudition/provenance（#294）、
  docs index 再生成（Coordinator 統合時）。
- **Unsupported**: なし（typed error 系は fixture で検証済み）。

## 受入条件 → 証拠対応

| AC | 判定 | 証拠 |
| --- | --- | --- |
| semantic→selection producer | Pass (headless) | `rcp_selection.rs` + `rcp_ex01_main_playback_all_live_publishes`（structure_digest = `routing_fingerprint` で receipt build key が engine checked_key と一致）・`rcp_producer_fails_closed_on_bad_evidence`（MissingPluginEvidence/DescriptorMismatch/UnknownCoverage） |
| PlanReconciler → RoutingEngine 配線 | Pass (headless) | `drive_selection_plan` + 全 drive test（Published/Equivalent/Failed の typed outcome・stale gate・last-good 保持） |
| Intent 別 Plan/Blocked/Preview（RCP-EX-01/03 統合経路） | Pass (headless) | `rcp_ex01_*` 4 case・`rcp_ex03_export_rejects_preview_only_artifact`（Export/MainPlayback 拒否・EditingPreview opt-in のみ NonEquivalent） |
| cutset/topology（RCP-EX-02 統合経路の構造部） | Pass (headless・構造部) | `rcp_ex02_topology_edge_kinds_preserved`（Send/Sidechain/Return/Main edge kind + chain 順）。cutset 全 Port 充足・PDC・二重加算 oracle は player slice `rcp_04.rs` で検証済み・本 slice は接合部 pin |
| edit 分類（RCP-EX-04 統合経路） | Pass (headless) | `rcp_ex04_rename_norebuild_controlonly`（NoRebuild/ControlOnly + engine Equivalent）・`rcp_ex04_structural_edit_rebuilds_and_republishes`（Rebuild + 新 publish） |
| stale/last-good（RCP-EX-05 統合経路） | Pass (headless・headless 範囲） | `rcp_ex05_stale_selection_never_publishes`（stale gate で engine 未呼出・Blocked・fresh 復帰）。QueueFull/Retire/Clock Epoch 変更の実機系は RCP-EX-06 残件 |
| eligibility 整合（RCP-EX-03 統合経路） | Pass (headless) | preview-class artifact が MainPlayback/Export で偽 Exact を生まないことを oracle で pin。`ArtifactEligibility`/`LookupOutcome` → `RenderCandidate` は report をそのまま写し推測しない |
| RCP-EX-06 実機 | NotRun | PCM・実 Plugin・実 OS・実 RT の async nonRT prepare/no-alloc/seek/loop/tail/PDC/frame accuracy — synvelis#296 RCP-GATE。headless green を実機 Pass に換算しない |

## NotRun / Deferred 一覧

- `SessionRoutingEngine`（`synvelis-app/src/engine.rs`）adapter
  配線: lease 外。`RoutingEngine` レベルの headless oracle で
  代替。app 側変更が必要と判明した場合は別 lease。
- BranchAudition/provenance 経路: #294 影響圏・本 slice 対象外。
- 実 Plugin ABI（`scarecrow173/ruviform#630`）・実 signal/PCM
  clock（`scarecrow173/ruviform#628`）・仮想 OS endpoint
  （`scarecrow173/ruviform#629`）・実 Window/GPUI runner
  （`scarecrow173/ruviform#631`）・実 OS GUI（#114・#176）・
  実行/製品 gate（#23）: NotRun（別 gate）。
- `*-full`/`*-msrv`/3OS 横断/GUI lane: NotRun（Daily/手動
  Full CI へ委譲）。
- docs index（`docs/**/README.md`）: Deferred — Coordinator が
  統合時に `doc-maintenance` で再生成。本 slice では未同期の
  まま（PR 本文「未対応の作業」に明記）。
