---
id: "TSPEC-034QdZPqGguxUOzVihyxjE"
type: "test-spec"
status: "approved"
title: "VS-A F0 Gate 検証仕様 — Stable ID・時間・Command・Save/Reload・Recording"
created: "2026-11-01"
updated: "2026-11-01"
owners: []
relations:
  source: []
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
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/vs-a-project-recording-foundation-stable-id-command-poc.md"
  derived-by: []
  verifies:
    - "docs/specs/vs-a-project-recording-foundation-stable-id-command-save-reload.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# VS-A F0 Gate 検証仕様 — Stable ID・時間・Command・Save/Reload・Recording

## Purpose

spec（VS-A）の全 Acceptance Criteria を実テストへ 1:1 に落とす正本。
各テストは headless・決定的・実デバイス非依存で、F0 Gate 完了判定の
検証根拠となる。新しい AC の追加時はこの対応表も更新する。

## Feature

`ProjectController<O>` による原子的 Project Mutation・Undo/Redo・
Save/Reload と、Stable ID・時間型・Recording 参照の型安全な基盤。

## Rules

- Stable ID は型で区別され、`"<kind>-<32hex>"` で round-trip する。
- SessionFrame/SourceFrame は非負・checked のみ、ProjectTime の
  Musical/Media は負位置を許容する。
- `[start,end)` は `end < start` 構築を拒否し、空区間は型上許容、
  CaptureSegment では非空を要求する。
- 全 Command は ExpectedRevision 照合→全件検証→単一 Mutation で、
  失敗時は部分適用しない。
- Undo/Redo は内容のみ復元し新規 Live Revision を発行、Saved 不変。
- Session 状態機械は Created→Open→Closed→Finalized のみ合法で、
  登録系 Command は Open のみ受理。
- Save は temp→publish→HEAD rename の順で、Load は
  deny_unknown_fields + SchemaVersion + 参照検証で Reject する。
- Verified Source（Source.state）は Imported≠Verified≠Committed≠Saved
  を区別し、Media bytes を含まない。

## Examples（AC → test 対応表）

| Spec AC | テスト（crate/tests file::test name） |
| --- | --- |
| AC1 workspace/lockfile | `mise run check`/`test`/`build` 実行 + `cargo test --workspace --locked`（mise test 内） |
| AC2 ID 型安全/round-trip | `id.rs` `compile_fail` doctest + `tests/id_roundtrip.rs`（全 ID 種 serialize/deserialize・kind 不一致 Reject・ClockEpochId 含む） |
| AC3 時間分離/checked | `tests/time_checked.rs`（SessionFrame/SourceFrame 負構築・overflow・負転 Err、ProjectTime 負値 OK） |
| AC4 `[start,end)` | `tests/range.rs`（end<start 構築 Err・空区間・隣接 `[a,b)`/`[b,c)` 無重複・contains/intersection） |
| AC5 Session 状態機械 | `tests/session_state.rs`（Created→Open→Closed→Finalized・逆/飛び級 IllegalTransition・非 Open 登録拒否） |
| AC6 参照整合 | `tests/recording_refs.rs`（全リンク存在性・別 Session 所属・segment 非空・take.segment_ids ⊆ session・stream→lane/clock_epoch） |
| AC7 原子的 | `tests/atomic.rs`（StaleRevision・不存在 target・検証失敗で Live 不変/revision 不増） |
| AC8 Undo/Redo | `tests/undo_redo.rs`（内容復元・新規 revision 発行・Saved 不変・redo クリア・空 stack no-op） |
| AC9 SchemaVersion Reject | `tests/schema.rs`（未知 version・未知フィールド・壊れた JSON で Load 失敗・Live 不変） |
| AC10 PPQN/TempoMap | `tests/tempo.rs`（既定 960・TempoMapRef revision・tick↔MediaTime・120BPM/4/4/48kHz/4小節=384000frame↔15360tick） |
| AC11 poc 決定出力 | `tests/poc_determinism.rs`（bin 2 回実行→同一 stdout・`mise run test` 内） |
| AC12 Verified Source 参照 | `tests/source_state.rs`（SourceState 直和・前進のみ・bytes 非含有を serialize で確認・Project Save≠Finalize） |
| AC13 記録 | impl record/ADR/本 test-spec の存在確認 |

## Guarantees

- 全テストは実デバイス・乱数・時刻・ネットワーク非依存（git 依存の
  初回 fetch を除く）。
- InMemoryStore は canonical JSON bytes を保持し round-trip が実 encoding
  を経る。recording-poc は DirectoryBundleStore で実 protocol を検証。
- Load 失敗時は従前の Live/Saved 状態を壊さない。

## Non-goals

- RT Capture/Writer/Gate/Comp の検証（VS-B 以降）。
- Journal/Recovery/VCS/GC の動作（参照保持の記録のみ）。
- 実 Device/GUI/Plugin の動作。

## Risk

- これらの保証を落とすと、後続 VS-B〜D が二重 Store や
  部分適用のある Controller 上に構築され、E2E-L/R/X の前提が壊れる。
