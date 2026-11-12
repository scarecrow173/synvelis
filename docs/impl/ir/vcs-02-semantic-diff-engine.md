---
id: "IMPL-Vcs02SemanticDiffEngine"
type: "impl"
status: "completed"
title: "VCS-02 Semantic Diff Engine — canonical ChangeSet・typed identity/lineage・Unknown tri-state"
created: "2026-11-10"
updated: "2026-11-10"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/93"
    - "https://github.com/scarecrow173/synvelis/issues/7"
    - "https://github.com/scarecrow173/synvelis/issues/92"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-project/src/diff/types.rs"
      - type: "file"
        path: "crates/synvelis-project/src/diff/engine.rs"
      - type: "file"
        path: "crates/synvelis-project/src/diff/adapters.rs"
      - type: "file"
        path: "crates/synvelis-project/src/diff/mod.rs"
      - type: "file"
        path: "crates/synvelis-project/tests/vcs02_diff.rs"
      - type: "file"
        path: "docs/specs/vcs-02-semantic-diff-changeset.md"
      - type: "file"
        path: "docs/designs/vcs-02-semantic-diff-engine-design.md"
      - type: "file"
        path: "docs/plans/vcs-02-semantic-diff-engine.md"
      - type: "file"
        path: "docs/test-specs/vcs-02-semantic-diff-fixtures.md"
      - type: "file"
        path: "docs/tasks/vcs-02-task1-semantic-diff-engine-headless-slice.md"
      - type: "file"
        path: "docs/impl/ir/vcs-02-semantic-diff-engine.md"
    modified:
      - type: "file"
        path: "crates/synvelis-project/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-project/src/snapshot/digest.rs"
      - type: "file"
        path: "docs/specs/README.md"
      - type: "file"
        path: "docs/designs/README.md"
      - type: "file"
        path: "docs/plans/README.md"
      - type: "file"
        path: "docs/test-specs/README.md"
      - type: "file"
        path: "docs/tasks/README.md"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/vcs-02-semantic-diff-engine.md"
    - "docs/designs/vcs-02-semantic-diff-engine-design.md"
    - "docs/specs/vcs-02-semantic-diff-changeset.md"
    - "docs/tasks/vcs-02-task1-semantic-diff-engine-headless-slice.md"
  implemented-by: []
  depends-on:
    - "docs/designs/vcs-02-semantic-diff-engine-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/ir/vcs-01-immutable-snapshot-resolver.md"
    - "docs/specs/vcs-01-immutable-snapshot-resolver.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/plans/vcs-02-semantic-diff-engine.md"
  derived-by: []
  verifies:
    - "docs/test-specs/vcs-02-semantic-diff-fixtures.md"
  verified-by: []
  references: []
  defers:
    - "VCS-03 Git Read Provider（Issue #94）"
    - "VCS-04 History/Visual Diff UI（Issue #95）"
    - "VCS-05 Apply/Restore"
    - "実 OS filesystem atomicity / 実 Git remote/provider / GUI / 実 plugin host probe（Issue #97）"
    - "lineage schema（Issue #7 で将来の Project schema extension へ deferred）"
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# VCS-02 Semantic Diff Engine — Implementation Record

## What was built

`synvelis-project` に VCS 非依存の Semantic Diff Engine を additive
module `src/diff/` として実装した。`compare_snapshots` は 2 つの
VCS-01 `ImmutableSnapshot` を入力に、`projection.project()` のみを
読み（SnapshotResolver / ProjectController / ProjectStore / VCS port /
live transport / runtime engine は不変・非接触）、canonical
`ChangeSet` を 1 つ返す pure・non-RT エンジン。

- `types.rs`: `ComparisonSpec`（before/after `SnapshotIdentity`・
  policy version・`ComparisonDomains`・`ComparisonBudget`）/
  `ComparisonKey`（両 digest・schema/projection version・policy・
  domains から導出する deterministic key）/`ChangeSet`（spec・key・
  ordered changes・unresolved issues・overall・completeness・
  summary）/`SemanticChange`（`ChangeId`・`SemanticObjectRef`・
  `ChangeKind`・before/after `SemanticValueRef`・affected range・
  field evidence・dependencies）/`SemanticObjectRef`（project ID・
  `ObjectKind`・typed stable ID・`lineage`）/`ChangeEvidence` +
  `SemanticFieldDiff`（canonical JSON field-level before/after）/
  `ComparisonEquality` tri-state（`Equal`/`Different`/
  `Unknown(ComparisonIssue)`）/`ComparisonIssue` 7 variant /
  `DiffError`（`Cancelled`/`BudgetExceeded`/`Internal`）/
  `ChangeSetCache`（key digest・capacity 既定 64 の LRU、
  `Arc<ChangeSet>` 共有）。
- `engine.rs`: entity map の typed stable ID matching、raw u128 の
  cross-kind collision 検出（`AmbiguousIdentity`）、canonical JSON
  再帰 field diff、`ChangeId` = comparison-key digest + domain +
  object + kind + field paths の deterministic 導出、
  snapshot-level issue 収集（asset manifest / opaque state）、
  schema-version suppression（pre-v6 routing / pre-v7 role・
  time_scope / pre-v8 MASTER_FADER_RAW）、deterministic sort
  （domain→kind→stable ID→change kind→field→change ID）、
  equality/completeness 確定、budget・cooperative cancel。
- `adapters.rs`: `DomainAdapter` trait + Project/Arrangement/Media/
  Control/Routing/Recording/Comping の 7 adapter。field 分類 policy
  が semantic kind を決める: clip anchor のみ → `Moved`、extent/
  playback → `Modified`、source 差し替え → `Replaced`、processor
  chain 内順序 → `Reordered`（chain）・chain 間移動 → `Moved`
  （processor）、connection endpoint → `Reconnected`、edge 増減 →
  `Added`/`Removed`、digest 差し替え → `Replaced`、automation
  region 編集 → `Modified`。clip-relative note は clip 移動に
  追従し note 差分を出さない。MIDI source の notes は子
  `MidiNote` entity として global diff、owner source 相違は
  `AmbiguousIdentity`。SMF documents は keyed-synthetic として
  実体 diff（pre-v9 absent = faithfully empty）。
- `lib.rs`: `pub mod diff` + re-export。`snapshot/digest.rs`:
  `ContentDigest` に `PartialOrd`/`Ord` derive 追加（cache key
  用、挙動変更なし）。

### Identity / Unknown semantics

- identity = project ID + typed `ObjectKind` + typed stable ID。
  raw u128 が異 kind に跨る場合は `CrossKindId`、owner/lineage
  参照 field の相違は `OwnerConflict`、project ID 不一致は
  `CrossProject` — いずれも entity diff を出さず
  `AmbiguousIdentity` issue + `Unknown`。
- `Equal` は changes 0・issues 0・両 snapshot `Complete` のとき
  のみ。confirmed change があれば `Different`（issues 共存時は
  `completeness=Partial`）。changes 0 + issues ありは
  `Unknown(representative)`。digest-equal fast path でも
  snapshot-level issue は必ず収集し partial snapshot を `Equal`
  に昇格させない。
- `lineage` は現行 schema に永続化経路がないため常に `None`
  （捏造しない。Issue #7 の将来 schema extension へ deferred）。

## Verification（実施済み）

| Task | Result |
| --- | --- |
| `mise run fmt` / `fmt-check` | Pass |
| `mise run check` | Pass（workspace --all-targets） |
| `mise run lint` | Pass（-D warnings） |
| `mise run test` | Pass（workspace --locked。vcs02_diff 21 tests 含む） |
| `mise run test-rt` | Pass（rt_ contract tests） |
| `mise run test-msrv` | Pass（rust 1.89.0 toolchain で workspace test） |
| `mise run check-app-native` | Pass |
| `mise run lint-app-native` | Pass |
| `mise run build-app-native` | Pass |

Headless fixture coverage: test-spec VCS-DIFF-01〜08 を
`tests/vcs02_diff.rs` 21 test で検証。semantic 分類
（rename/move/trim/duplicate/source swap/processor reorder・move/
connection rewire・add・remove/automation/asset digest）、
AA/AB/BA・determinism、missing/unknown asset・opaque state、
pre-v6 partial projection、cross-kind・owner・cross-project
ambiguity、cache identity、budget/cancel を網羅。fixture は
VCS-01 経路（`ProjectController` + `SnapshotResolver::
capture_live`）を主経路とし、ops で生成不能な crafted state
（identity collision・owner conflict・degraded manifest・旧
schema version）のみ直接 `ImmutableSnapshot` 構築を使用。

### NotRun / Unsupported（明示）

- 実 OS filesystem atomicity: **NotRun** — headless fixture のみ。
- 実 Git remote/provider・実 index/worktree: **Unsupported** —
  VCS-03（Issue #94）の `VcsBytesPort` 境界待ち。
- GUI / native UI / 実 plugin host probe: **Unsupported** —
  VCS-04（Issue #95）/ Issue #97。
- Source Project 変更・Git checkout: **実施せず**（読み取り専用
  比較のみ）。
- `synvelis-app`/`workspace`/`library`/`gateway`/`player`/
  `recording`/`render`/`theme`/`poc`: **無変更**（diff module は
  `synvelis-project` 内に閉じる）。

## Notes / follow-ups

- `Cargo.toml`/`Cargo.lock`/`mise.toml` 無変更。依存追加なし。
- `ChangeSetCache` は derived state のみ保持し project/media
  truth にはならない（comparison-key digest でのみ index）。
- SMF document は v9 導入のため pre-v9 snapshot では absent =
  faithfully empty として実在 diff を出す（suppression 対象外）。
- 未対応の将来拡張: lineage schema、VCS-03 以降の provider 接続、
  history/visual diff UI（ChangeSet consumer）。
