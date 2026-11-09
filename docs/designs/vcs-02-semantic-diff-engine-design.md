---
id: "DESIGN-034UEtYMoQUWsPmOBrl819"
type: "design"
status: "approved"
title: "VCS-02 Semantic Diff Engine design"
created: "2026-11-09"
updated: "2026-11-09"
owners: []
relations:
  source:
    - "SPEC-034UEsXPitEuEsjBM4qjo6"
  verified_by: []
  implemented_by: []
  supersedes: []
  superseded_by: []
---

# VCS-02 Semantic Diff Engine design

Spec: `docs/specs/vcs-02-semantic-diff-changeset.md`。本書は module 構成・アルゴリズム・identity 判定の詳細設計。

## Module 構成

`crates/synvelis-project/src/diff/`（`synvelis-project` 内に閉じ、Ruviform 非依存・pure・non-RT）:

- `types.rs` — `ComparisonSpec`/`ComparisonKey`/`ChangeSet`/`SemanticChange`/`SemanticObjectRef`/`ChangeKind`/`ComparisonEquality`/`ComparisonIssue`/`ComparisonDomain`/`ComparisonBudget`/`DiffError`/`SemanticTimeRange`/`SemanticValueRef`/`ChangeEvidence`/`SemanticFieldDiff`/`ChangeSetSummary`。
- `engine.rs` — `compare_snapshots(before, after, spec, cancel) -> Result<ChangeSet, DiffError>`、`ChangeSetCache`、entity matching helper、canonical JSON field diff、policy 適用、sort/summary/equality 確定。
- `adapters.rs` — `DomainAdapter` trait と per-domain adapter（ProjectMeta / Arrangement / Media / Control / Routing / Recording / Comping）。新 Project domain は adapter 追加で拡張。
- `mod.rs` — 公開 re-export と module doc。

`lib.rs` に `pub mod diff;` と主要型 re-export を追加。既存 public API・`Cargo.toml`/`Cargo.lock`/`mise.toml` は変更しない（dev-dep 追加なし）。

## アルゴリズム

1. `ComparisonKey` 構成（両 `SnapshotIdentity` の digest/schema/projection + policy + domains）。
2. `project_id` 不一致 → entity diff せず `AmbiguousIdentity{CrossProject}` 1 件で `Unknown`。
3. 両 digest 一致 → adapter を skip し snapshot-level issue のみ収集（AA fast path。partial は `Equal` にしない）。
4. raw→kind map を両 snapshot から構築（entity-issued ID のみ。`SmfDocument`/`AppliedCommit`/`AppliedImport`/`RoutingModel`/`Project` 等の keyed 合成 identity は除外）。同一 raw が異 kind → 該当 entity を `AmbiguousIdentity`。
5. 各 adapter が `included_domains` に応じて domain を diff:
   - entity map を stable ID で照合 → Added/Removed/Matched。
   - Matched pair は `PartialEq` で quick check → 差異時のみ canonical JSON 再帰 diff → `(field path, before, after)` 列。
   - field policy が `Identity` class を返したら object 全体を `AmbiguousIdentity`（他 field の change も出さない）。`Skip`/`Suppressed`/`Modified`/`Moved`/`Reordered`/`Reconnected`/`Replaced` に分類。
   - `(object, kind)` ごとに 1 `SemanticChange` へ集約。`before/after` は entity canonical JSON の `ContentDigest`。
6. Routing adapter は chain membership を post-pass: processor→chain 割当差 → `Moved(Processor)`、同一 chain 内順序差 → `Reordered(Chain)`。
7. snapshot-level issue 収集: assets manifest（Media domain）、opaque_state（Routing domain）、schema suppression（PartialProjection）。
8. sort → summary → `overall`/`completeness` 確定 → `ChangeSet`。

## Schema suppression

migration ladder（`migration.rs`）の投影由来 field/entity を authored diff から除外し、差異があれば `PartialProjection`:

- `schema_version < 6`: routing domain 全体 suppress（投影 ID は watermark 由来、send/FX/return 表現不能）。
- `< 7`: `ParameterTarget.role`/`time_scope` を suppressed field。
- `< 8`: `MASTER_FADER_RAW` target を suppressed entity。

## Identity / lineage

`SemanticObjectRef.lineage: Option<LineageRef>` は常に `None`（lineage schema は #7 で将来拡張、偽造しない）。Ambiguous 判定は (a) project_id 不一致、(b) raw→kind 衝突、(c) owner/lineage 参照 field の同一 ID 間差異、の 3 経路。同一 kind・同一 raw の内容差は検出不能な再発行を含み得るが、creation lineage 不在のため `Modified` 等の確定分類に留める（制限として文書化）。

## Resource / cache

- `ComparisonBudget { max_entities, max_changes, max_unresolved }`（既定 1_000_000 / 10_000 / 10_000）。超過・`SnapshotCancel` → `DiffError::{BudgetExceeded, Cancelled}`。
- entity は `BTreeMap` 照合で bounded、field diff は差異 entity のみ JSON 化。
- `ChangeSetCache`: `comparison_key.digest()` → `Arc<ChangeSet>`、容量上限（既定 64）FIFO 退避。derived index で正本ではない。

## 依存方向

`synvelis-project` 内で完結。Ruviform Graph/Media には Synvelis 型を持ち込まない。新 dev-dep・外部 crate 追加なし（`serde`/`serde_json`/`sha2` 相当の既存 `snapshot::digest::ContentDigest` を再利用）。
