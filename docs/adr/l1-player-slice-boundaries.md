---
id: "ADR-Ym8S8syVSNCeQiMNBHHfHt"
type: "adr"
status: "accepted"
title: "L1 — Import・Blob・Lowering・Driver の配置と境界"
created: "2026-11-03"
updated: "2026-11-03"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/24"
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
    - "docs/adr/vs-d-commit-identity-manifest-single-mutation.md"
    - "docs/adr/vs-c-journal-checkpoint-sync-fault-finalized.md"
    - "docs/specs/l1-local-wav-verified-media-graph-playback.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/l1-local-wav-verified-media-graph-playback.md"
  derived-by: []
---

# ADR: L1 Player Slice の配置と境界

## Status

Accepted (review-protocol 0 blocker 相当)

## Context

Issue #24 は Safe Import・Verified Media・Lowering・Headless 再生を要求する。
配置先・冪等性・blob 管理・processor 方式・mute 意味の決定が必要。

## Considered Options

- **Option 1: synvelis-player 新設** — import/resolver/lowering/processor/driver を
  `crates/synvelis-player` に集約。`synvelis-recording` の staging/sync/digest
  helper を再利用。依存方向 `tests/poc → synvelis-player →
  {synvelis-project, synvelis-recording}`。
- **Option 2: synvelis-recording へ追加** — recording session 型に引きずられ、
  「import は録音ではない」非対称を埋め込む。却下。
- **Option 3: synvelis-project へ全置き** — 永続化ポートの crate に I/O・codec・
  graph 依存が流入し、層違反となる。却下。

## Decision

Option 1 を採用し、以下の設計決定を確定する。

- **冪等性**: `ImportId` (define_id "import") と
  `Project::applied_imports: BTreeMap<ImportId, AppliedImport>` を追加し、
  VS-D の `applied_commits` パターンを踏襲。`import-manifest.json` が
  staging dir に予約 ID・content hash を記録し、retry/resume を一意にする。
  同一 import_id の再実行は適用済み結果を返し、hash 不一致は integrity
  conflict。
- **単一 atomic mutation**: `ImportOp::CommitImport` は 1 op で
  Asset+Source(state=Committed)+applied_imports entry を登録。`apply` は
  scratch clone 上で失敗時全破棄 (controller 保証)。Blob 公開は op の
  *前* に完了するため、commit 失敗は orphan blob を明示して返す。
- **Blob store**: `<media_root>/blobs/<digest16>` content-addressed。
  `publish_atomic` (tmp→sync→rename) + `sync_parent_dir`。同一 digest の
  既存 blob は実体を再 hash して一致すれば reuse、不一致なら Corrupt。
- **Mute**: `Clip.muted: bool` (`#[serde(default)]`) + `SetClipMuted` op。
  lowering 時に muted clip は node を生成せず `Skipped::Muted` として記録。
- **Processor 方式**: clip は `NodeKind::Processor` + `ClipPlayer`
  (`PrefetchConsumer` を内包) として install。master は `NodeKind::Sink`
  (boundary) で driver が `output_reader` で排出。レイアウト差は
  `ChannelMap` 明示、rate 差は型付き skip (暗黙 resample なし)。
- **Transport clock**: `Arc<AtomicU64>` engine 位置を driver が block 毎に
  更新し `ClipPlayer` が読む。seek は `PrefetchHandle::seek` + clock 更新で
  generation boundary 経由の再同期。
- **Revision 更新**: Ruviform 依存を `c2a2f07` (PR #225/#227/#231 統合済)
  へ lock 再生成。AGENTS.md の「既定ブランチ追従 + lock pin」に適合。

## Consequences

- Import 失敗は全段で型付き (`ImportError`) — Project 部分 mutation なし。
- Orphan blob は outcome/error に明示され `abandon` 系 API で整理可能。
- Player は Graph public API に Project 型を一切渡さない (AudioFormat と
  stable handle のみ)。
