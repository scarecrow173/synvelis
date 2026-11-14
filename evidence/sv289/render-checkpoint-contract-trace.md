# SV289 — Render Checkpoint Contract Trace (synvelis#289 RCP-00)

Issue #289「Render Recipe・依存閉包・有効性・Branch/
Execution/Policy統合仕様正本」の各節・受入条件を、
canonical spec/design/ADR の対応セクションと、参照した
既存契約・コードへ写像した対応表。本 slice は
**docs/evidence のみ**であり、実装・fixture 実走・実機
gate の証拠ではない。

## Environment

| Field | Value |
| --- | --- |
| Synvelis branch | `docs/289-render-recipe-spec`（worktree `sv289-rcp0`） |
| Synvelis base | `scarecrow173/sv289-rcp0` worktree HEAD（docs slice・コード変更なし） |
| OS | Windows (win32) |
| Toolchain | mise 管理 + Node.js（doc ツール）。docs/markdown のみ |
| Issue | https://github.com/scarecrow173/synvelis/issues/289 |
| 合意 | 2026-11-11 合意（Issue 本文記載） |

## Issue 節 ↔ 正本セクション ↔ 既存契約/コード 対応表

| Issue #289 節 | 正本（spec/design/ADR） | 参照した既存契約・コード（read-only） |
| --- | --- | --- |
| §0 権威と非交渉不変条件 | spec §Non-negotiable Invariants（12 項）・ADR §Decision Drivers/Outcome | `docs/specs/project-model.md`（canonical project）・`project-persistence.md`（PROJ-RCP-01）・`media-render.md`（MR-RCP-01）・`execution-plan.md`・`playback-graph.md`・`tim03`/`tim-04`・`vcs-01`/`vcs-integration` |
| §1 永続/派生境界と識別子 | spec §Persistent vs Derived Boundary and Identifiers・design §Persistent vs Derived Objects | `project-model.md` PM-R-08（VariantScope/BranchContentRef）・`vcs-01` SnapshotLocator/Identity・`synvelis-render/src/snapshot.rs` `RenderSnapshot::freeze` |
| §2 Canonical RenderRecipe v1 | spec §Canonical Render Recipe v1（header/target/upstream/processor/temporal/RecipeKey/migration/用語区別）・design §Artifact Manifest | `crates/synvelis-render/src/spec.rs`（`PluginStateRecipe` 別概念の確認）・`synvelis-library/src/digest.rs`（content digest）・`tim-04`（StateCaptureQuality） |
| §3 依存閉包と部分無効化 | spec §Dependency Closure and Partial Invalidation・design §Resolver and Dependency Index | `playback-graph.md`/`sf-03`（typed graph・generation）・`project-model.md`（semantic node/port/edge）・`musical-time.md`（tempo/frame 写像） |
| §4 Eligibility/信頼性 | spec §Eligibility and Trustworthiness（状態機械・Exact 6 条件・再現性クラス・Plugin Missing・oracle）・design §Validity/Eligibility Evaluation | `tim03`（OfflineExact fail-closed）・`tim-04`（state 品質）・`synvelis-render/src/verify.rs`（readback 検証）・`synvelis-library/src/digest.rs`（integrity） |
| §5 Subgraph substitution / Execution Intent | spec §Subgraph Substitution / Execution Intent・design §Plugin-Missing Execution Modes and Last-good Safety | `sf-03`（semantic→runtime lowering・SourceMap・Last-good）・`playback-graph.md`（plan generation receipt）・`tim03`（RT thread 制約）・`synvelis-render/src/{preflight,prepare}.rs` |
| §6 Render Policy / Save / Commit / Share | spec §Render Policy / Save / Commit / Share・design §Policy / Job Orchestration and Receipt | `project-persistence.md` PROJ-RCP-01（save/job receipt 分離・late attach）・`media-render.md` MR-RCP-01・`vcs-integration.md`（commit/immutable locator）・`synvelis-render/src/job.rs`/`lib.rs`（job/receipt） |
| §7 Artifact Publish / Retention / GC / Sharing | spec §Artifact Publish / Retention / GC / Sharing・design §Artifact Manifest and Content-Addressed Store | `synvelis-render/src/verify.rs`（staged verify→publish）・`synvelis-library/src/{cache,registry,local,git/,webdav/}`（CAS/verified blob/provider）・`project-persistence.md`（GC/orphan 保護） |
| §8 Test plan / Failure Matrix | spec §Test Plan / Failure Matrix（case 群 ↔ AC 対応表） | `evidence/sv17/case-ledger.md`・`evidence/sv23/*`（case-ledger/verification の先例形式）・`docs/test-specs/`（子 Issue 側で作成） |
| §9 責務と着手DAG | spec §Owners and Start DAG（owner 分離・子 Issue DAG・edge 型・重複禁止）・design §Child Issue Decomposition | `execution-plan.md`/`execution-plan-design.md`（RCP-00〜07/F01 ノード・start/integration/acceptance edge）・`POLICIES.md` §1/§6（owner 境界）・`AGENTS.md`（Ruviform 節） |

## 受入条件 RCP-SPEC-01〜08 ↔ 正本 ↔ 状態

| AC | 要求（Issue 転記） | 正本の対応 | 本 slice の状態 |
| --- | --- | --- | --- |
| RCP-SPEC-01 | Canonical Recipe フィールド・Hash/Context 版・Exclude List・Unknown 対照表と相互互換 Migration を設計 fixture で固定 | spec §Canonical Render Recipe・§Dependency Closure | **設計契約確定**。fixture 実装は #290（NotRun） |
| RCP-SPEC-02 | Sidechain/Send/Return/MIDI/Automation/Tempo/Plugin State/External Asset/Time-range 依存 closure と無効化の正負例、false-valid=0 | spec §Dependency Closure | 同上（#290） |
| RCP-SPEC-03 | Exact/Preview/Unknown/Corrupt 状態機械・再現性クラス・Audio readback・Hash collision/Version 変化 oracle | spec §Eligibility | 同上（#290/#291） |
| RCP-SPEC-04 | Branch→Recipe→Artifact・Adopt 1 CAS/Undo・late attach・Snapshot/VCS 保存・GC Roots の状態遷移と安全性反例 | spec §Branch/Adopt・§Artifact Publish・§Render Policy | 同上（#294/#291） |
| RCP-SPEC-05 | Plugin Missing/Edit/Adopt・Execution Intent 別 Subgraph cut・Plan activation/Last-good/Clock/Capability 故障行列を型と fixture へ | spec §Subgraph Substitution | 同上（#293） |
| RCP-SPEC-06 | Render Policy trigger/Scope/Completeness/Job Receipt と Save/Commit/Share 非原子性を検証条件へ | spec §Render Policy | 同上（#292） |
| RCP-SPEC-07 | Owner/DAG/PR 触るファイル/AudioLens 互換/実 OS・実 Plugin 未実施 Gate を子 Issue ごとに分離、重複実装禁止 | spec §Owners and Start DAG・design §Child Issue Decomposition | **本書で固定**（docs-level） |
| RCP-SPEC-08 | #290〜#297 の Owner/型・Start/Integration/Acceptance DAG を照合、Ruviform DAW 混入・重複 DSP Compiler/Plugin Host/Media Writer なし | spec §Owners and Start DAG・§Non-negotiable Invariants・ADR §Considered Options | **本書で固定**（docs-level） |

**注**: 「設計契約確定」「本書で固定」は docs-level の整合を
意味し、実装・fixture 実走・実機 gate の Pass ではない。
各 AC の実装検証は対応する子 Issue（#290〜#296）の
test-spec/fixture/evidence が担う。

## 用語区別の確認（`PluginStateRecipe` 衝突）

| 概念 | 定義場所 | 性質 |
| --- | --- | --- |
| `RenderRecipe`/`RecipeKey`（本仕様） | `docs/specs/render-checkpoint.md` §Canonical Render Recipe | 処理要求の canonical 表現と digest。**新規概念** |
| `PluginStateRecipe`（既存） | `crates/synvelis-render/src/spec.rs:32` | plugin state の品質 evidence + gate。**別物** |

spec §Canonical Render Recipe の「用語の区別」で両者の
混同・流用・改名・包含を禁止することを明示した。

## 子 Issue DAG 照合（#290〜#297）

`docs/specs/execution-plan.md`・`docs/designs/
execution-plan-design.md` の RCP ノード定義と本 spec
§Owners and Start DAG を照合:

- `SV-RCP-00(#289)` → `start` → `SV-RCP-01(#290)`〜
  `SV-RCP-06(#295)`（並行着手可能）— 一致
- `RF#562`/`RF#196`/`RF#198`/`SV-SF-03(#106)`/
  `SV-TIM-04(#128)`/`SV-CB-00(#277)`/`SV-MEDIA(#285)`
  → `integration` → `SV-RCP-01..06` — 一致
- `SV-RCP-01..06` → `acceptance` → `SV-RCP-07(#296)` —
  一致
- `SV-RCP-F01(#297)` は将来 PoC・初期 Gate 非 blocking —
  一致
- edge は forward-only・acyclic・定義済み node のみ、
  `acceptance` edge の to は Gate node のみ — `execution-
  plan-design.md` の edge 型規則と同一であることを確認

## Status vocabulary（本 evidence で使用）

`POLICIES.md` §3 準拠:

- 要求状態: `Required`/`Deferred`/`NotApplicable`
- 実行結果: `Pass`/`Fail`/`NotRun`/`Unsupported`
- 本 slice（docs-only）: docs 検証のみ `Pass`、実装・
  fixture・実機系は `NotRun`/`Unsupported`/`Deferred`。
  設計上の合意を実装 Pass に読み替えない。
