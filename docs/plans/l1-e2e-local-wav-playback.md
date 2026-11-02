---
id: "PLAN-wBxgPj2RVAMRBuOiry4yv2"
type: "plan"
status: "approved"
title: "L1 実装計画 — Local WAV Safe Import・Verified Media・Headless Graph Playback"
created: "2026-11-03"
updated: "2026-11-03"
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
    - "docs/specs/l1-local-wav-verified-media-graph-playback.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/l1-local-wav-verified-playback-design.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/l1-e2e-local-wav-verified-playback.md"
---

# L1 Plan

## Task decomposition

1. **l1-task1** — `synvelis-project`: `ImportId`、`AppliedImport`+
   `applied_imports`、`Clip.muted`+`SetClipMuted` op + unit tests。
2. **l1-task2** — `synvelis-player` 基盤: crate skeleton、workspace members、
   Ruviform deps + lock 更新 (`c2a2f07`)、`fixture` (決定的 WAV 生成)。
3. **l1-task3** — `import` + `resolver`: staging→verify→blob→CommitImport→
   receipts、orphan 明示、型付き ResolveError。
4. **l1-task4** — `lowering` + `processor` + `driver`: GraphSpec 生成、
   ClipPlayer、HeadlessDriver (pump 制御/seek/stop/last-good)。
5. **l1-task5** — `tests/l1.rs` 受入 L1-01〜08 + 検証 lane + impl record。

## File map

- modify: `crates/synvelis-project/src/{id,project,ops,lib}.rs`、
  `Cargo.toml` (root)、`Cargo.lock`
- add: `crates/synvelis-player/**`

## Risk register

- ruviform API drift (pin `8ad88f5`→`c2a2f07`): 全依存を同一 rev で解決、
  lock 再生成で検証。
- `PrefetchConsumer::fill` の sub-slice 分割: block を window で区切る
  ロジックを ClipPlayer に閉じ込め、block 境界跨ぎテストで実測。
- Musical→frame: TempoMap 変換コスト → L1 では Media anchor 主検証、
  Musical は秒変換→floor の P0 実装。

## Rollback

新規 crate は削除で完全 rollback。`synvelis-project` 追加分は
`#[serde(default)]` で旧 snapshot 互換。lock は `git checkout -- Cargo.lock`
で復帰可能。

## Verification matrix

L1-01〜08 ↔ `tests/l1.rs` 内の `l1_*` テスト (test-spec 参照)。
`mise run fmt-check && mise run check && mise run lint && mise run test`。
