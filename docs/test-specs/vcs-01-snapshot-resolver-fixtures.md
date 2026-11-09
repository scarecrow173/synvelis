---
id: "TSPEC-034UAzXp7iDmaV6yx7QTZQ"
type: "test-spec"
status: "approved"
title: "VCS-01 snapshot resolver fixtures"
created: "2026-11-09"
updated: "2026-11-09"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/92"
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
  depends-on:
    - "docs/specs/vcs-01-immutable-snapshot-resolver.md"
    - "docs/designs/vcs-01-immutable-snapshot-resolver-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/vcs-01-immutable-snapshot-resolver.md"
  derived-by: []
  verifies:
    - "docs/specs/vcs-01-immutable-snapshot-resolver.md"
  verified-by: []
  references: []
  defers:
    - "実 Git/実 OS/実 plugin host での検証（VCS-06 #97）"
  deferred-by: []
---

# VCS-01 snapshot resolver fixtures

## Purpose

Issue #92（VCS-01）の headless fixture 受入を `tests/vcs01_snapshot.rs`
で固定する。VCS-SNAP-01〜06 のうち headless で検証可能な行を
カバーし、実 OS/実 Git/GUI は NotRun/Unsupported として
impl record に記録する。

## Feature

`synvelis-project` の `SnapshotResolver` が Live/Saved/
Recovery/VcsStaged/VcsCommitted locator を Immutable
Snapshot へ read-only に解決し、race/破損/未知/部分状態を
明示 Failure または Degraded 記録で扱う。

## Rules

- Rule SNAP-FIX-01: `capture_live` は expected revision 一致時に
  その時点の Live を clone し、以後の編集・Undo・save が
  snapshot を変えない。revision 不一致は `StaleLiveRevision`。
- Rule SNAP-FIX-02: 同一 revision/内容の Live capture と Saved
  resolve は同一 `ContentDigest`（canonical envelope bytes の
  SHA-256）を持ち、`identity.source`/`locator`/`capture_generation`
  のみ異なる。
- Rule SNAP-FIX-03: `resolve_saved` は HEAD→bytes→HEAD→bytes の
  4 read で固定し、HEAD 変化は `HeadConflict`、同名 bytes 変化は
  `SourceChanged`。`head_ref: Some` の期待値不一致も
  `HeadConflict`。
- Rule SNAP-FIX-04: `resolve_recovery` は recovery-rooted store の
  checkpoint 名を読み、saved store の HEAD/snapshot を参照も
  変更もしない。checkpoint bytes の capture 中変化は
  `SourceChanged`。
- Rule SNAP-FIX-05: v5 envelope（`routing`/fader target 欠落）は
  migration ladder で現行 schema へ read-only projection され、
  `identity.schema_version=5`・`projection_version=9`・source
  bytes は不変。未知 schema（ladder 未到達・未来版）は
  `UnsupportedSchema`、破損 JSON/不正参照/Duplicate ID 相当は
  `InvalidSnapshot`。
- Rule SNAP-FIX-06: Missing Asset（probe `Absent`）・Unknown
  （probe 未供給/`Unknown`）・`ProcessorBinding::Unresolved` は
  `Missing`/`Unknown`/`Missing` として記録され
  `completeness: Partial`・`validation: Degraded` になり、
  snapshot 自体は構築される（Equal 判定は行わない）。
- Rule SNAP-FIX-07: VCS staged で generation が window 前後で
  不一致 → `IndexChanged`。参照 blob が index に absent →
  `PartialStage`（明示 Failure）。committed で absent blob →
  `Missing` 記録（degraded、Failure ではない）。
- Rule SNAP-FIX-08: cancelled token は phase 境界で
  `Cancelled`、snapshot bytes > `max_snapshot_bytes` は
  `BudgetExceeded`。
- Rule SNAP-FIX-09: 全 resolve の前後で `ProjectController`
  （revision/live/undo/redo/saved）・`ProjectStore`（HEAD・
  snapshot bytes）・VCS port 呼出しが不変 — read-only oracle。
- Rule SNAP-FIX-10: 同名 revision（`r1.json` 等）でも
  ProjectId・VCS fixed_revision・schema_version・digest の
  複合で識別され、digest 単独・revision 単独での同一性断定を
  しない。

## Examples

- Example: Live capture → 後続 AddTrack/undo/save → snapshot の
  project・digest が capture 時点のまま（SNAP-FIX-01/02/09）。
- Example: InMemoryStore save → resolve_saved → Live capture と
  digest 一致、identity.source が `saved` vs `live`（SNAP-FIX-02）。
- Example: fake store が head() 2 回目に別名を返す →
  `HeadConflict`（SNAP-FIX-03）。
- Example: recovery store に checkpoint snapshot、saved store
  には別 HEAD → 両者が異なる identity で解決（SNAP-FIX-04/10）。
- Example: `as_v5_envelope` 相当の v5 bytes を store/vcs port に
  配置 → schema_version=5/projection_version=9 の snapshot、
  routing が legacy projection で存在（SNAP-FIX-05）。
- Example: `BlobProbe` が `Absent`/`Unknown` を返す project →
  asset_resolution に status、completeness=Partial
  （SNAP-FIX-06）。
- Example: fake `VcsBytesPort` が generation を window 中に
  変更 → `IndexChanged`。blob absent → `PartialStage`
  （SNAP-FIX-07）。
- Example: cancel token cancel 済み・巨大 bytes →
  `Cancelled`/`BudgetExceeded`（SNAP-FIX-08）。

## Guarantees

- resolve は `ProjectController`/`ProjectStore`/`VcsBytesPort`
  の状態を変更しない（全て `&` 参照、write method 非存在）。
- digest は capture した固定 bytes のみに依存し、resolve 中の
  外部変化を反映しない。
- 破損・未知・Missing を成功・Equal と偽らない。

## Non-goals

- Semantic Diff/ChangeSet/comparison（VCS-02）。
- 実 Git provider・実 FS/OS・GUI・plugin host（VCS-03/04/05/06）。
- SHA-256 の暗号学的検証用途（content addressing のみ）。

## Risk

- fixture が実 Git/実 FS の atomicity を代替しない — impl
  record で NotRun/Unsupported を明記する。
- `Resolved` plugin binding の opaque state は `Unknown` 固定
  — 将来 probe で `Known`/`Missing` へ精緻化する余地を残す。
