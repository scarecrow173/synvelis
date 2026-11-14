---
id: "DESIGN-034X9E4j59hipUI46lNAQC"
type: "design"
status: "approved"
title: "VCS Integration — 統合契約・層構成・Case 被覆設計 (synvelis#7)"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source: []
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/vcs-integration.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/vcs-integration.md"
    - "docs/specs/vcs-01-immutable-snapshot-resolver.md"
    - "docs/specs/vcs-02-semantic-diff-changeset.md"
    - "docs/specs/vcs-03-git-read-provider.md"
    - "docs/specs/vcs-04-backend-wiring.md"
    - "docs/specs/vcs-04-history-visual-diff-ui.md"
    - "docs/specs/vcs-04-native-mount.md"
    - "docs/specs/vcs-05-apply-restore-headless.md"
    - "docs/specs/vcs-06-fixture-fault-matrix.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/vcs-integration.md"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "evidence/reports/vcs-06-fixture-matrix-ledger.md"
    - "evidence/sv7/vcs-integration-case-ledger.md"
  defers: []
  deferred-by: []
---

# VCS Integration — 統合契約・層構成・Case 被覆設計 (synvelis#7)

## Context

Issue #7 は Synvelis の「Create → Understand → Refine」ワークフローを
project revision の変更理解へ拡張する親設計 Issue である。実装は
VCS-01（#92）〜VCS-05（#96）の slice Issue と統合 Gate VCS-06（#97）に
分割済みであり、本 design はそれらを束ねる**統合契約の設計**であって
新規実装設計ではない。既存 slice spec が各層の正本であり、本書は
「どの slice がどの製品不変条件を担うか」「全 view が同一
ChangeSet をどう共有するか」「Case ID をどう被覆するか」の対応を
固定する。

## Scope

### In Scope

- VCS-01〜VCS-06 slice と Issue #7 統合契約の責務対応表
- 層構成: provider-neutral 契約層（`synvelis-project::vcs`）/
  provider 実装層（`synvelis-library::git`）/ adapter 層
  （`VcsHistoryAdapter`）/ workspace UI 層（`vcs_ui`）/ app op 層
  （`syn.vcs.*`）/ native mount 層の責務境界
- ChangeSet 共有モデル（1 つの `Arc<ChangeSet>` を全 view が消費）
- Safe Apply / Restore-as-Copy の統合契約（ProjectCommand/CAS/Undo
  経路、別新コピー、read-only provider 不変条件）
- 受入 Case 被覆設計（`evidence/sv7/vcs-integration-case-ledger.md` の行設計）
- Issue #7 §6 の 8 view に対する採用/未採用/延期 UI 判定
- Creative Branch と VCS branch の責務分離記録
- sv#95 native mount 実装との整合確認記録

### Out of Scope

- 新規コード設計・API 追加（既存 slice の実装設計は各 slice design
  doc が正本）
- 実 GUI/OS/provider/RT/consumer の受入設計（#114・#97 REAL lanes）
- VCS Writer・3-way merge・Git 書き込み系の設計
- mockup HTML の新規デザイン（採否判定のみ）

## Components and Boundaries

### Slice 責務対応表（Issue #7 契約 → 実装 Owner）

| 契約領域 | 正本 spec | Owner Issue | 実装状態（本書記録時点） |
| --- | --- | --- | --- |
| Immutable Snapshot 解決（Live/Saved/Recovery/Staged/Committed、schema/asset/plugin-state 完全性、race/cancel/budget） | `docs/specs/vcs-01-immutable-snapshot-resolver.md` | #92 | 実装・fixture Pass（vcs-06 ledger） |
| Canonical ChangeSet（pure/non-RT、tri-state equality、deterministic order/fingerprint/cache key、typed stable ID） | `docs/specs/vcs-02-semantic-diff-changeset.md` | #93 | 実装・fixture Pass |
| Git Read Provider（`VcsReadProvider`/`VcsBytesPort`/`GitVcsProvider`、fixed revision、history/working status/staged index、read-only） | `docs/specs/vcs-03-git-read-provider.md`、`vcs-03-unborn-head-classification.md` | #94 | 実装・fixture Pass（unborn HEAD 分類は別 Issue 送り記録あり） |
| History/Visual Diff UI（`VcsHistoryBackend`/`VcsHistoryAdapter`、`vcs_ui` projection、`syn.vcs.*` ops、headless + native mount） | `docs/specs/vcs-04-history-visual-diff-ui.md`、`vcs-04-backend-wiring.md`、`vcs-04-native-mount.md` | #95 | headless/native mount 統合済み（PR #252/#384）。実 OS GUI は #114 |
| Safe Apply Selected Change / Restore As New Copy（typed entity transplant、fingerprint CAS、ApplyJournal 冪等、staged publish+readback） | `docs/specs/vcs-05-apply-restore-headless.md`、`vcs-05-monitoring-apply-surface.md` | #96 | headless 実装済み。engine/runtime 実適用・GUI approval UX は後続 |
| 統合受入・fixture/証拠台帳 | `docs/specs/vcs-06-*.md` 群 | #97 | fixture ledger 記録済み。REAL lanes は NotRun/Unsupported 継続 |
| 本統合契約 + Case 被覆台帳 | 本書 chain + `evidence/sv7/` | #7（本スライス） | docs-only |

### 層構成とデータフロー

```
Git repo / worktree / index          (read-only 観測のみ)
        │  VcsReadProvider（provider-neutral, synvelis-project::vcs）
        │  VcsBytesPort（index_generation/read_file/read_blob）
        ▼
SnapshotResolver（VCS-01）            → ImmutableSnapshot（5 source 系統）
        │  compare_snapshots / ChangeSetCache（VCS-02）
        ▼
canonical ChangeSet（Arc<ChangeSet>、tri-state、deterministic）
        │  VcsHistoryAdapter / VcsHistoryBackend（VCS-04）
        ▼
VcsHistoryViews（workspace::vcs_ui）  → VcsHistoryView read-model
        │  syn.vcs.view / syn.vcs.visual ops（synvelis-app）
        ▼
ViewType::VcsHistory → ViewHost/ViewMount → create_view_window（native mount、sv#95）
```

- **Apply 経路（唯一の write 経路）**: ChangeSet 選択 subset →
  typed entity transplant plan → fingerprint CAS → `ProjectController`
  単一 writer → 1 undo step。`ApplyJournal`（Pending/Applied/Failed）で
  冪等 receipt。`Removed` は cascade 設計 deferred のため全 kind
  fail-closed。
- **Restore 経路**: 解決済み snapshot → content-addressed bundle
  （`DirectoryBundleStore` 互換 layout）→ staging → validate/readback
  → same-dir rename publish。`Absent` のみ受理、`project_id` 再採番・
  stable ID 保持・plugin-state/asset 検証。source/Saved HEAD/Git
  index/worktree は不変。
- **非接続 degraded mode**: `vcs_backend = None` で `syn.vcs.*` は
  `NoBackend` を正直に返す。project new/open/save・Live vs Saved
  diff・DAW 制作は VCS 非依存で動作する（fake backend を bind しない）。

### ChangeSet 共有不変条件

- `VcsHistoryViews` が 1 つの `ChangeSetCache` を所有し、全 view の
  compare が同一 cache を通る。同一 `(before, after, spec)` は
  `Arc<ChangeSet>` を共有し独立再計算しない。
- 全 Visual Diff view（Arrangement/Signal Flow/Automation/Mixer/
  Inspector 等の read-only projection）は同一 ChangeSet の異なる
  投影であり、view ごとに別の diff アルゴリズムを持たない。
- `Unknown`/`Partial`/`Stale`/`Unsupported`/`Missing`/`Ambiguous`/
  `Unobserved` は明示状態として各 view に運ばれ、empty/Equal に潰れない。

## Data and Control Flow

1. **Snapshot 解決**: endpoint pin（Live revision / Saved HEAD /
   Staged `index_generation` / Committed `FixedVcsRevision` /
   Recovery）→ provider/bytes port 経由で bytes 取得 → read-only
   migration/projection → `ImmutableSnapshot`（completeness・degraded
   付き）。Live 前進・index 変動で pin 内容は書き換わらない。
2. **比較**: 2 snapshot → `compare_snapshots` → deterministic
   `ChangeSet`（変更 kind 列・unresolved issues・tri-state equality・
   summary）。cache key = snapshot identity + comparison spec。
3. **History 閲覧**: `list_history`（bounded page・opaque cursor・
   merge-aware・全 parent 保持）→ `HistorySelection`（chosen_parent
   metadata 保持）→ A/B pin → stale 判定（endpoint 種別ごとの観測値
   比較）。閲覧は read のみで Project/Transport/AudioGraph/engine
   graph build/Git 状態を変更しない。
4. **Apply（明示操作時のみ）**: selection → preflight（target
   revision・stable ID・schema・asset・plugin-state・conflict・
   現在 Project revision）→ fingerprint CAS → ProjectController
   commit → journal receipt。失敗時 live/journal 不変。
5. **Restore（明示操作時のみ）**: destination `Absent` 検査 →
   staging publish → readback 検証 → atomic rename → receipt。
   途中失敗は staging を残し destination を汚さない。

## Non-Functional Constraints

- **Read-only 不変条件**: VCS 閲覧系の全経路は Project Commit/Save/
  Apply/Restore・Git index/worktree/branch/HEAD 変更・transport/
  recording/undo/redo・engine graph build への作用を持たない。
  検証は oracle（revision/undo 不変・provider 呼出が read 系のみ）。
- **RT 分離**: VCS 処理・snapshot 解決・diff 計算・provider read は
  すべて non-RT。RT スレッドへ流さない。
- **正直な状態**: missing/unknown/degraded/unsupported/ambiguous は
  明示エラー・明示 UI 状態であり silent Equal 禁止。
- **決定性**: ChangeSet の ordering・fingerprint・cache key は
  deterministic。同一入力は同一 `Arc<ChangeSet>`。
- **namespace 分離**: `commit_oid`（provider object namespace）と
  asset `ContentDigest`（常に SHA-256）は別名前空間で比較・変換・
  同一視しない。runtime graph ID（`NodeId`/`PortId`/generation）と
  project semantic stable ID も別空間。
- **非接続独立性**: VCS provider Off/非 repository でも project
  制作・Live vs Saved diff・Creative Branch が独立動作する。

## Risks and Trade-offs

| # | Risk | Likelihood | Impact | Mitigation |
| --- | --- | --- | --- | --- |
| 1 | headless/fixture Pass を実 GUI/OS/provider/RT 受入へ誤昇格 | med | high | verdict 語彙 `Pass`/`Pass*`/`NotRun`/`Unsupported` を ledger で固定。`evidence/sv7/` でも同規則を継承 |
| 2 | 既存 slice spec 間の軽微な前提差異（例: unborn HEAD 分類、monitoring apply surface の slice 分割）が統合契約と矛盾 | low | med | 本書は slice spec を正本として参照し、差異は Deferred/既知制限として記録。修正は各 Owner Issue へ |
| 3 | Case ID の ledger 行と Issue #7 受入条件の対応漏れ | med | med | `evidence/sv7/vcs-integration-case-ledger.md` で全 Case ID + 拡張行を 1:1 に列挙し、Issue 受入条件 → Case ID の対応表を付ける |
| 4 | mockup HTML 未収録により 8 view の画面レビューが不完全 | high | low | Issue 本文の画面構成 + 既存 view mode 決定を根拠に採否を記録し、HTML 本体レビューは別作業として明記 |
| 5 | doc graph の既存 broken relations が新文書の検証を阻害 | high | low | 既存の pre-existing blocker として記録し、本スライス起因の新規 broken relation のみを修正対象とする |

## Deferred Design Concerns

- 実 OS GUI/IME/2Window/pixel/実 Plugin dock — #114 gate。
- 実 remote/auth/GC・他 VCS provider・実 OS permission matrix —
  #97 REAL lanes。`VcsCapabilities`/`VcsError::UnsupportedCapability`
  で誠実に扱う。
- VCS Writer・3-way merge・commit/push — read-only provider 契約の
  範囲外。
- Rendered Audio Diff / waveform delivery、Overlay / Evolution Map —
  vcs-04 spec 非目標の後続 slice。
- `plugin_instances` の diff 可視化 — #324 系。
- Creative Branch の VCS 連携深化 — #277〜297 系の別契約。

## Issue #7 §6 Visual Diff 8 view — 採否判定

チャット添付 HTML モックアップ本体はリポジトリ未収録（Issue #7 が
「GitHub リポジトリへの HTML 本体の追加は別作業」と明記）。以下は
Issue 本文の画面構成と既存 slice spec の確定済み view mode 決定を
根拠とした契約レベルの採否記録であり、pixel/実機レビューの代替では
ない。

| # | View | 判定 | 根拠 |
| --- | --- | --- | --- |
| 1 | Arrangement | **採用** | read-only projection として VCS-04 の対象 view。SideBySide/ChangeFocus で clip/track 差分を表示 |
| 2 | Signal Flow | **採用** | `signal_flow_ui` 先例の read-only projection。routing semantic（Reconnected/Reordered/Modified）を表示 |
| 3 | Automation | **採用** | read-only projection。parameter/automation lane 差分を ChangeSet から投影 |
| 4 | Mixer | **採用** | read-only projection。send/mute/solo 等の project semantic 値を表示（runtime probe observation と区別） |
| 5 | Inspector | **採用** | read-only projection。選択 entity の field 差分を表示 |
| 6 | Clip Detail | **条件付き採用** | clip 単位の詳細差分は ChangeSet の clip entity 投影で実現可能だが、独立 view type としての mount は後続 slice |
| 7 | Plugin State | **未採用（現時点）** | `plugin_instances` の diff 可視化は #324 系で設計中。opaque plugin state は Unknown/MissingOpaqueState として正直に表示する契約のみ確定 |
| 8 | Audio（Rendered Audio Diff / waveform） | **延期** | vcs-04 spec が Rendered Audio Diff・waveform delivery を明示的非目標。source PCM vs rendered output の区別と失敗/未取得表示の契約のみ本書で固定 |

view mode 自体は **SideBySide** と **ChangeFocus** の 2 種を採用
（vcs-04-history-visual-diff-ui spec 確定済み）。**Overlay /
Evolution Map** は延期。HTML モックアップの pixel レビューと
Clip Detail 独立 mount・Plugin State view は後続 slice/#114 で再審。

## Creative Branch との責務分離記録

- Creative Branch（#277〜297 系）は VCS branch/checkout を代替しない。
  VCS provider が Off/非 repository でも Fork/Compare/Adopt は
  独立動作する。
- Creative Branch Adopt は `ProjectController` CAS+1 の Undo 可能
  操作であり、VCS history/index/HEAD/外部 repository を変更しない。
- branch-local ID・Git commit ID・ProjectRevision は別 namespace
  であり相互変換・同一視しない。
- 作成 lineage が曖昧な場合は `AmbiguousIdentity` で fail-closed。
- 本統合契約は Creative Branch の内部設計を規定しない。分離の
  不変条件のみを固定する。

## sv#95 native mount との整合確認

sv#95（PR #384、`docs/specs/vcs-04-native-mount.md` /
`docs/impl/ir/vcs-04-native-mount.md`）で統合済みの実装を確認した:

- `syn.vcs.visual{open}` → `view-{n}` mint → `syn.view.attach`
  （dock-domain、registry descriptor fallback、未登録は fail closed）
  → `mount_vcs_history_view`（session-owned epoch fresh mint）→
  `create_view_window` → run loop `update_view` 追従、の経路は本書の
  層構成図と一致する。
- `vcs_screen_cell` の publish は post-op tail に集約され、VCS op と
  project 変更の双方で mounted spec が追従する — 「History 閲覧中も
  Project が変わらない（read-only）」と「Live endpoint の stale 表示」
  の両立として整合。
- gpui-test scenario は read-only（select/invoke 拒否・intent 0）と
  外部 gate の `Unsupported` 記録を検証済み。実 OS GUI/IME/pixel/
  plugin dock は #114 に残る — 本統合契約の Deferred と一致。
- **差異・注意点**: `vcs-04-backend-wiring` spec が記録する unborn
  HEAD の provider 分類バグ（`service_error`）は library 側既知制限
  であり本契約の阻害ではないが、VCS-GIT 系 Case の unborn HEAD 行の
  verdict 解釈時に考慮が必要（ledger に既記録）。

## Verification Notes

- [x] 本 design が `docs/specs/vcs-integration.md`（approved）を
  derives-from し、全 slice spec との矛盾がないことをレビューで確認
- [x] `evidence/sv7/vcs-integration-case-ledger.md` が全 Case ID（基底 + ledger 拡張行）
  を 1:1 で列挙し、各行に owner/依存/fixture/verification layer/
  requirement state/result state/evidence path を持つこと
- [x] verdict 語彙（Pass/Pass*/Fail/NotRun/Unsupported/Blocked）と
  「headless Pass を実機受入に換算しない」規則が ledger と spec の
  双方で一致すること
- [x] `mise run fmt-check` が docs-only 変更で exit 0
- [x] `doc_maintenance.js index apply` で触れた dir の index が
  再生成され drift がないこと
- [x] `sync_impl_index.js --kind ir|exp` で impl index が sync すること
