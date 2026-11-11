---
id: "SPEC-eEtM5VJlLpCbVXLnCEar4M"
type: "spec"
status: "proposed"
title: "RCD-03 VCS semantic diff monitor 宣言分類 slice — synvelis#117"
created: "2026-11-11"
updated: "2026-11-11"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/117"
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
    - "docs/specs/rcd-03-monitor-persistence.md"
    - "docs/specs/vcs-02-semantic-diff-changeset.md"
    - "docs/specs/rcd-03-app-arm-monitoring-ux.md"
    - "docs/specs/rcd-03-monitoring-punch-latency.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# RCD-03 VCS semantic diff monitor 宣言分類 slice — synvelis#117

## What Should Be Built

`docs/specs/rcd-03-monitor-persistence.md` の Deferred Design
Concerns で follow-up とされた残件を回収する。schema v12 で
`Project::monitoring` に永続化される authored monitor 宣言
（track ごとの `TrackMonitorDecl` と project 全体の `PunchMarks`）
を VCS-02 semantic diff（`crates/synvelis-project/src/diff/`）の
分類対象に追加し、2 つの `ImmutableSnapshot` 間で monitor 宣言の
追加・削除・変更が既存分類規約どおりの `SemanticChange` として
現れるようにする。

- `ObjectKind` に `TrackMonitorDecl`・`PunchMarks` を追加し、
  比較ドメインは `ComparisonDomain::Recording` とする
  （Arm/Monitor/Punch は recording intent の authored 宣言であり、
  recording registry と同じ domain で比較されるべき状態）。
- `monitoring.tracks`（`BTreeMap<TrackId, TrackMonitorDecl>`）は
  `SmfDocument`/`AppliedCommit` と同じ「他 entity の ID 空間を
  借用する keyed-synthetic collection」として `diff_map` で
  照合する。`raw_kind_map` からは除外する（track raw と衝突を
  捏造しない）。
- `monitoring.punch` は project 全体 singleton として、
  declaration-presence 意味論で分類する
  （`punch_in`/`punch_out` ともに `None` = 宣言なし）。
- 分類追加による意味論の変更を `DIFF_POLICY_VERSION` で version
  付けし、旧 policy で計算された `ChangeSet` と key 衝突しない。

## Why Now

monitor 宣言は Issue #117 の永続化 slice（PR 済み）で
`Project` の authored state になったが、semantic diff は
curated projection（既知 entity のみ列挙）のため monitor 宣言の
変更が `ChangeSet` に現れず、`Equal` 判定も monitor 差を見落とす。
宣言が VCS/History の既存契約に乗るための最小の整合的変更として、
`ObjectKind`/`Relation` 分類の既存先例に沿って分類を追加する。

## Users and Value

- VCS history/diff の消費者（VCS-03 provider・VCS-04 UI・
  VCS-05 apply の選別基盤）が、monitor 宣言の追加・削除・変更を
  他の authored 変更と同じ精度で観測できる。
- monitor 宣言のみが異なる snapshot 対が誤って `Equal` と
  報告されなくなる。

## Scope

### In Scope

- `crates/synvelis-project/src/diff/`（`types.rs`・
  `engine.rs`・`adapters.rs`）: 新規 `ObjectKind` 2 variant、
  `domain()` 写像、`raw_kind_map` の keyed-synthetic 除外
  コメント、Recording adapter の monitor 宣言 diff（map 照合 +
  punch singleton）、`DIFF_POLICY_VERSION` bump。
- `crates/synvelis-project/tests/`: monitor 宣言分類 fixture
  （`vcs02_diff.rs` 内の新規セクション）。
- 本 slice の spec/design/plan/task/test-spec/impl-ir 文書 +
  管理 index 再生成。

### Out of Scope

- monitor 宣言 change の VCS-05 apply/restore 対応 — 新規 kind は
  apply surface 外として fail-closed（`ApplyError::Unsupported`）
  のまま。apply 側の transplant 設計は別 slice。
- `MonitorAdapter` の engine 搭載・Loop/PunchInLoop gate・
  count-in UX・実 device monitor/latency・実 GUI — Issue #117 の
  後続残件で本 slice 対象外。
- `synvelis-app`/`player`/`recording`/`library`/`workspace`/
  `gateway`/`ui/` の変更。
- session 観測値（`input_output_ready`/`hardware_direct`/
  latency/稼働 session）の分類 — 非永続の外部観測であり
  authored 宣言ではない（永続化 spec と同じ境界）。

## Requirements

- monitor 宣言は authored intent のみを分類対象とし、session
  観測値を diff に混入しない（永続化 spec の境界を維持）。
- `TrackMonitorDecl` は map 存在差で `Added`/`Removed`、同一
  track key の内容差で `Modified` とする。`lane`/`channel_map`/
  `arm`/`monitor` の各 field は `Modified` として field-level
  evidence に現れる（`lane` は宣言の owner 参照ではなく binding
  target であり、identity 規則の例外とはしない）。
- `monitoring.tracks` の key は `TrackId` — `raw_kind_map` に
  登録しない（Track との cross-kind 衝突を捏造しない）。key の
  raw が snapshot 間で異 kind に衝突する場合は既存の
  `AmbiguousIdentity(CrossKindId)` がそのまま効く。
- `PunchMarks` は singleton object（stable_id = project tag、
  `RoutingModel`/`Project` と同じ規約）として、declaration
  presence（いずれかの mark が `Some`）に基づく
  `Added`/`Removed`/`Modified` を出力する。両 mark が有効な
  `[in, out)` を構成するとき `affected_range` に
  `SemanticTimeRange::Musical` を付す。
- 新規 kind は既存 kind の `ObjectKind as u8` discriminant を
  変えない位置（enum 末尾）に追加し、既存 change-id 規則・
  sort 規則・既存 kind の振る舞いを不変に保つ。
- `DIFF_POLICY_VERSION` を 2 に進め、比較意味論の拡張を key に
  束縛する（旧 policy の `Equal` 結果が新 policy で流用されない）。
- `validate_references`・apply engine・VCS-05 apply surface・
  snapshot manifest の既存契約を変更しない（新 kind は apply
  surface 外 = 既存の fail-closed `_` arm で `Unsupported`）。

## Acceptance Criteria

- [ ] authored monitor 宣言（`TrackMonitorDecl`/`PunchMarks` —
  schema v12 で永続化される monitor 状態）が VCS semantic diff の
  `ObjectKind`/`Relation` 分類に現れ、変更検出・表示が既存分類
  規約と一致する（`Added`/`Removed`/`Modified`、決定的 sort、
  `Recording` domain での domain filter 参加）。
- [ ] 分類の追加が既存の diff 対象（他 `ObjectKind`/Relation）
  の振る舞いを変えない — `vcs02_diff.rs`・`vcs06_diff_matrix.rs`
  既存 fixture が変更なしで pass する（回帰）。
- [ ] monitor 宣言の add/remove/modify が diff で区別されることを
  fixture で検証する（宣言 upsert/clear、punch marks の
  set/clear/変更の各ケース）。

## Deferred Design Concerns

- monitor 宣言 change の VCS-05 apply/restore — apply surface
  への `TrackMonitorDecl`/`PunchMarks` 追加（transplant payload・
  cascade・preflight 設計が必要）。
- monitor 宣言に対する `Moved`/`Reordered` 相当の分類は現行
  意味論に該当しない（lane binding 変更は `Modified`、decl
  は順序を持たない map）。
- `MonitoringData` 以外の recording intent（session 観測値）は
  永続化されないため diff 対象外のまま。

## Sources and Relations

- `docs/specs/rcd-03-monitor-persistence.md`（永続化 spec —
  Deferred Design Concerns で本 slice を follow-up として記録）。
- `docs/specs/vcs-02-semantic-diff-changeset.md`（分類規約・
  tri-state・同一性ルールの正本）。
- `docs/designs/rcd-03-monitor-persistence-design.md`（monitor
  POD モデルの正本 — §semantic diff follow-up）。
- GitHub issue `scarecrow173/synvelis#117`（Refs。本 spec は
  Issue を Close しない）。
