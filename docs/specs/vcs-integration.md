---
id: "SPEC-034X9CW66GeytQXJHrogRB"
type: "spec"
status: "approved"
title: "VCS Integration — Snapshot/Canonical ChangeSet/History/Visual Diff/Safe Apply・Restore-as-Copy 統合契約 (synvelis#7)"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/7"
    - "https://github.com/scarecrow173/synvelis/issues/97"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/vcs-01-immutable-snapshot-resolver.md"
    - "docs/specs/vcs-02-semantic-diff-changeset.md"
    - "docs/specs/vcs-03-git-read-provider.md"
    - "docs/specs/vcs-03-unborn-head-classification.md"
    - "docs/specs/vcs-04-backend-wiring.md"
    - "docs/specs/vcs-04-history-visual-diff-ui.md"
    - "docs/specs/vcs-04-native-mount.md"
    - "docs/specs/vcs-05-apply-restore-headless.md"
    - "docs/specs/vcs-05-monitoring-apply-surface.md"
    - "docs/specs/vcs-06-evidence-completion.md"
    - "docs/specs/vcs-06-fixture-fault-matrix.md"
    - "docs/specs/vcs-06-monitoring-matrix.md"
    - "docs/specs/vcs-06-real-git-fs-fault.md"
    - "docs/specs/vcs-06-ui-headless-matrix.md"
  refines: []
  refined-by: []
  derives-from:
    - "https://github.com/scarecrow173/synvelis/issues/7"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/vcs-integration.md"
  references:
    - "evidence/reports/vcs-06-fixture-matrix-ledger.md"
    - "evidence/reports/vcs-06-compat-evidence.md"
    - "evidence/sv7/vcs-integration-case-ledger.md"
    - "evidence/sv7/vcs-integration-verification.md"
  defers: []
  deferred-by: []
---

# VCS Integration — Snapshot/Canonical ChangeSet/History/Visual Diff/Safe Apply・Restore-as-Copy 統合契約 (synvelis#7)

## What Should Be Built

Issue #7（VCS Integration / Semantic Visual Diff）の**統合契約文書チェーンと受入 Case 台帳**を作る。本スライスは実装ではなく、VCS-01〜VCS-06 の各 slice spec が個別に定めた契約を、Issue #7 が要求する 1 つの製品レベル契約へ統合し、Snapshot・Canonical ChangeSet・History・Visual Diff・Safe Apply・Restore-as-Copy の責務境界と受入 Case（VCS-SNAP/DIFF/GIT/UI/APPLY/RESTORE/COMPAT）の被覆状況を `evidence/sv7/` に台帳化する。

具体的には次の文書を canonical として整備する:

- `docs/specs/vcs-integration.md`（本書）— 統合契約の What/Why/受入条件
- `docs/designs/vcs-integration-design.md` — 層構成・データフロー・既存 slice との責務対応・却下案
- `docs/plans/vcs-integration-plan.md` — 文書チェーンと evidence 台帳の作成計画・検証行列
- `docs/tasks/vcs-integration-task.md` — 本スライスの実行単位
- `docs/test-specs/vcs-integration.md` — 統合契約が保証すべき振る舞いの test-spec
- `docs/impl/ir/vcs-integration.md` / `docs/impl/exp/vcs-integration.md` — 本 docs-only スライスの実装記録
- `evidence/sv7/` — Case 被覆台帳と検証記録

## Why Now

VCS-01（#92 Immutable Snapshot）・VCS-02（#93 Semantic ChangeSet）・VCS-03（#94 Git Read Provider）・VCS-04（#95 History/Visual Diff UI、headless + native mount まで統合済み）・VCS-05（#96 Safe Apply / Restore-as-Copy headless）は実装 slice として進行・統合済みであり、VCS-06（#97）は fixture/証拠台帳を `evidence/reports/vcs-06-*.md` に記録している。しかし Issue #7 本体の統合契約 — 「これらの slice が満たすべき製品レベルの不変条件と受入 Case の全体像」— を束ねる canonical 文書が存在しない。#97 の統合受入は Case ID 単位の verdict を要求するが、親 Issue #7 の受入条件（14 項目）と既存 slice 群・実装状況の対応表がないと、何が Pass で何が未検証かを一意に読めない。

本スライスは docs のみで完結し、コード・CI・生成 index の手編集は行わない。実装 Owner（#92〜96）と統合 Gate（#97）の責務は変更しない。

## Users and Value

- **DAW ユーザー**: Track/Clip/Routing/Plugin/Automation/Audio を横断する revision 差分を、History・Visual Diff・Safe Apply・Restore-as-Copy として安全に閲覧・適用できる契約の土台。
- **VCS slice 実装 Owner（#92〜96）**: 自分の slice が親契約のどの不変条件を担うかを一意に参照できる。
- **統合受入 Owner（#97）**: Case ID → fixture/証拠 → verdict の被覆台帳を `evidence/sv7/` から引ける。
- **将来のレビュアー**: 「headless Pass を製品受入に換算しない」「Unknown を Equal に潰さない」等の境界が文書上で固定される。

## Scope

### In Scope

- Issue #7 の統合契約を spec/design/plan/task/test-spec/impl-ir/impl-exp の 7 文書として canonical 化する。
- 既存 VCS slice spec（vcs-01〜vcs-06 系）との責務対応表: どの slice が Snapshot/ChangeSet/Git Read/UI/Apply/Restore/証拠を所有するか。
- 受入 Case ID（VCS-SNAP-01〜06、VCS-DIFF-01〜08、VCS-GIT-01〜06、VCS-UI-01〜05、VCS-APPLY-01〜06、VCS-RESTORE-01〜06、VCS-COMPAT-01〜04）と、ledger 上の拡張行（VCS-DIFF-09/10、VCS-APPLY-07/08、VCS-RESTORE-07/08/09、VCS-GIT-REAL-*）の被覆台帳を `evidence/sv7/` に作成する。未実行・非対応は NotRun/Unsupported として理由付きで記録する。
- Issue #7 §6 の Visual Diff 8 view（Arrangement / Signal Flow / Automation / Mixer / Inspector / Clip Detail / Plugin State / Audio）について、採用 UI・未採用 UI・延期 UI の判定を記録する。チャット添付 HTML モックアップ本体はリポジトリ未収録（Issue 記載の通り repo 追加は別作業）であることを明記し、Issue 本文の画面構成と既存 slice spec の view mode 決定（SideBySide / ChangeFocus 採用、Overlay / Evolution Map / Rendered Audio Diff 延期）を根拠に記録する。
- VCS 非接続時に DAW 制作と Live vs Saved Diff が独立動作することの契約確認。
- Creative Branch（#277〜297 系）と VCS branch/checkout の責務分離の確認記録。
- sv#95（VCS History native mount/dock、PR #384）の統合済み実装との整合確認と、発見した差異の記録。

### Out of Scope

- `crates/*/src/`・`tests/` の実装変更（実装は #92〜96・#97 Owner）。
- 実 GUI/IME/2Window/pixel/実 Plugin dock の受入（Issue #114 gate）。
- 実 OS FS durability・実 remote/auth/GC・実 provider（Git 以外）・RT/device・consumer precision の受入（#97 REAL lanes / #294）。
- VCS Writer・3-way merge・checkout/stage/commit 等の Git 書き込み系。
- `docs/*/README.md` 等の管理 index の手編集（`doc-maintenance` による再生成のみ）。
- チャット添付 HTML モックアップのリポジトリ収録（Issue が別作業と明記）。

## Requirements

- VCS は閲覧系が常に read-only。Project/Transport/AudioGraph・Git index/worktree/HEAD・commit state を変更しない。
- 比較元は immutable snapshot（Live / Saved Working / Recovery / Staged-Pending / Committed 固定 revision）に限定し、pin は解決時点 bytes に固定される。
- Versioned migration と Unknown を維持する。旧 schema は read-only projection/migration を通し、表現不能領域は Unknown/PartialProjection であり empty/Equal にしない。
- Diff は canonical ChangeSet を複数 view で共用する。独立再計算・view ごとの別 ChangeSet を作らない。
- Apply/Restore は明示承認・preflight・Project CAS/Undo 経路を必須とし、Restore は失敗時に現在の作業を守る別新コピーのみ。Project/Undo を迂回した直接 Restore、Git checkout/index の暗黙書換は禁止。
- Git checkout/index/branch/worktree/外部 repository 状態を暗黙に変更しない。
- Creative Branch は VCS branch/checkout ではない。VCS provider が Off でも Creative Branch の Fork/Compare/Adopt は動作し、Adopt は ProjectController CAS+1 Undo 操作であり VCS history/index/HEAD/外部 repository を変更しない。branch-local ID を Git commit ID や ProjectRevision と混同しない。作成 lineage が曖昧な場合は `AmbiguousIdentity` で失敗する。
- Routing semantics: `RoutingConnection` が canonical connection、`SendConfig` は `SendId`↔`ConnectionId` 1:1 で tap/gain binding を持つ。connection/endpoint 変更は `Reconnected`、FX order 変更は `Reordered`、send tap/gain binding 変更は `Modified`。1 操作が矛盾する重複 history entry を捏造しない。legacy v5 の RoutingModel 非所持は Unknown/Unsupported/PartialProjection であり empty routing にしない。runtime graph ID と project semantic ID は別空間。visual layout/probe toggle は DSP/project semantic diff ではない。Mute/Solo/Send 値は project semantics であり runtime probe observation と区別する。
- Audio source PCM と rendered output の比較は別物であり、比較条件・失敗・未取得を明示する。
- VCS 処理は RT スレッドに流さず非同期に実行できる。
- 不透明 Plugin 状態・Asset 欠落・Unsupported/Missing/Ambiguous を「変更なし」と誤表示しない。
- tri-state equality（Equal/Different/Unknown）を維持し、known change は unresolved issue 共存でも Different、0 changes + unresolved issue は Unknown。

## Acceptance Criteria

Issue #7 から転記（各項目は対応する Case ID・証拠台帳行・検証層で照合する）:

- [ ] VCS-SNAP-01〜06、VCS-DIFF-01〜08、VCS-GIT-01〜06、VCS-UI-01〜05、VCS-APPLY-01〜06、VCS-RESTORE-01〜06、VCS-COMPAT-01〜04 を、それぞれの責務 Owner と #97 に記載の fixture/証拠台帳に照合し、未実行または非対応の場合も状態を記録する（→ `evidence/sv7/` case ledger）。
- [ ] 下表の各 VCS Slice が規定する Snapshot、ChangeSet、Git Read、UI、Safe Apply/Restore Copy の責務・先行関係と矛盾しない（→ design doc の責務対応表）。
- [ ] VCS 非接続でも DAW 制作と Live vs Saved の Diff が独立して動作する（→ VCS-COMPAT / degraded-mode 記録）。
- [ ] Immutable Project Snapshot、Schema/Asset 欠落の状態報告、安定 ID の設計が成立する（→ VCS-SNAP/DIFF 行の被覆確認）。
- [ ] Track Rename、Clip Move、FX Reorder、Edge Rewire、Parameter 変更の差分が意味的に正しい（→ VCS-DIFF 行）。
- [ ] 不透明 Plugin 状態/Asset 欠落を「変更なし」と誤表示しない（→ VCS-DIFF Unknown 行・VCS-SNAP degraded 行）。
- [ ] Merge 親を含む History、任意 A/B 選択、対象別履歴検索を検証する（→ VCS-GIT/VCS-UI 行）。
- [ ] Arrangement/Signal Flow/Automation/Mixer/Inspector の Visual Diff が同じ ChangeSet に整合する（→ VCS-UI 行 + view 共有 ChangeSet 契約）。
- [ ] Source PCM と Rendered Output の違い、比較条件と失敗/未取得を表示する（→ Audio diff 契約の記録、未実装は NotRun）。
- [ ] History 閲覧中も Project/Transport/AudioGraph が変わらない（→ read-only oracle 行）。
- [ ] Commit 対象の Staged/Saved/Live 差を見せ、欠落 Asset/未登録を警告する（→ VCS-UI/GIT 行）。
- [ ] Apply Change は検証付き Undo 可能な Project Command であり、Restore は失敗時に現在の作業を守る（→ VCS-APPLY/RESTORE 行）。
- [ ] VCS 処理は RT スレッドに流れず、非同期に実行できる（→ 設計契約 + 該当 Case 行）。
- [ ] チャット添付 HTML モックアップの全 8 画面・主要インタラクションをレビューし、採用する UI/未採用 UI を記録する（→ design doc の UI 判定表。HTML 本体未収録の旨と根拠を明記）。

## Deferred Design Concerns

- 実 OS GUI/IME/2Window/pixel/実 Plugin dock 受入 — Issue #114 gate。headless/gpui-test Pass を昇格しない。
- 実 remote/auth/GC・実 OS permission・他 provider（SVN/Perforce 系）— #97 REAL lanes、`VcsCapabilities` の `Unsupported` 宣言で誠実に扱う。
- VCS Writer・3-way merge・commit/push 等の書き込み系 — read-only provider 契約の範囲外、別 Issue。
- Rendered Audio Diff / waveform delivery・Overlay / Evolution Map view mode — vcs-04 spec の非目標として後続 slice。
- チャット添付 HTML モックアップのリポジトリ収録 — Issue #7 が別作業と明記。
- `plugin_instances` の VCS diff 可視化 — #324 系（restore 側 verbatim は VCS-RESTORE-08 で pin 済み）。
- unborn HEAD 分類バグ（`GitVcsProvider::head_oid` の TransientIo 誤分類）— vcs-04-backend-wiring spec が別 Issue 送りと記録。

## Sources and Relations

- Issue #7（本書の統合契約の出典）: <https://github.com/scarecrow173/synvelis/issues/7>
- Issue #97（統合受入 Gate・fixture/証拠台帳 Owner）: <https://github.com/scarecrow173/synvelis/issues/97>
- 実装 Owner Issue: #92（VCS-01）・#93（VCS-02）・#94（VCS-03）・#95（VCS-04）・#96（VCS-05）
- 関連 slice spec: `docs/specs/vcs-01-*.md` 〜 `docs/specs/vcs-06-*.md`（front matter `relations.related` 参照）
- 既存証拠台帳: `evidence/reports/vcs-06-fixture-matrix-ledger.md`、`evidence/reports/vcs-06-compat-evidence.md`
- 本スライスの被覆台帳: `evidence/sv7/`（case-ledger + verification）
