---
id: "IMPL-034X9Ln9ARL8qEXoWVpZcz"
type: "impl"
status: "completed"
title: "Issue #4 Signal Flow 設計正本 — 文書チェーンと証拠の作成"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/4"
  changes:
    added:
      - type: "file"
        path: "docs/specs/signal-flow-design.md"
      - type: "file"
        path: "docs/designs/signal-flow-design-design.md"
      - type: "file"
        path: "docs/plans/signal-flow-design-plan.md"
      - type: "file"
        path: "docs/tasks/signal-flow-design-task.md"
      - type: "file"
        path: "docs/test-specs/signal-flow-design.md"
      - type: "file"
        path: "docs/impl/ir/signal-flow-design.md"
      - type: "file"
        path: "docs/impl/exp/signal-flow-design.md"
      - type: "file"
        path: "evidence/sv4/design-rationale.md"
      - type: "file"
        path: "evidence/sv4/consistency-check.md"
    modified:
      - type: "file"
        path: "docs/specs/README.md"
      - type: "file"
        path: "docs/designs/README.md"
      - type: "file"
        path: "docs/plans/README.md"
      - type: "file"
        path: "docs/tasks/README.md"
      - type: "file"
        path: "docs/test-specs/README.md"
      - type: "file"
        path: "docs/impl/ir/README.md"
    deleted: []
    renamed: []
    moved: []
    generated:
      - type: "file"
        path: "docs/specs/README.md"
      - type: "file"
        path: "docs/designs/README.md"
      - type: "file"
        path: "docs/plans/README.md"
      - type: "file"
        path: "docs/tasks/README.md"
      - type: "file"
        path: "docs/test-specs/README.md"
      - type: "file"
        path: "docs/impl/ir/README.md"
  implements:
    - "docs/tasks/signal-flow-design-task.md"
    - "docs/plans/signal-flow-design-plan.md"
  implemented-by: []
  depends-on:
    - "docs/specs/signal-flow-design.md"
    - "docs/designs/signal-flow-design-design.md"
    - "docs/test-specs/signal-flow-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/signal-flow-design.md"
    - "docs/designs/signal-flow-design-design.md"
    - "docs/test-specs/signal-flow-design.md"
    - "docs/impl/exp/signal-flow-design.md"
    - "evidence/sv4/design-rationale.md"
    - "evidence/sv4/consistency-check.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/tasks/signal-flow-design-task.md"
  derived-by: []
  verifies:
    - "docs/test-specs/signal-flow-design.md"
  verified-by: []
  references:
    - "https://github.com/scarecrow173/synvelis/issues/4"
    - "https://github.com/scarecrow173/synvelis/issues/109"
    - "https://github.com/scarecrow173/synvelis/issues/114"
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted:
      - "docs/impl/exp/signal-flow-design.md"
    rejected: []
---

# Issue #4 Signal Flow 設計正本 — 文書チェーンと証拠の作成

## Summary

Issue #4 の設計正本として、Signal Flow View の階層・Typed
Port/Connection・Semantic Routing 編集・Probe 表示・世代証拠契約を
define する文書チェーン 7 ファイル（spec/design/plan/task/
test-spec/ir/exp）と `evidence/sv4/` 証拠 2 ファイルを作成した。
コード実装は対象外 — 実装 Owner は #104〜109。

## Context

`docs/tasks/signal-flow-design-task.md`（plan:
`docs/plans/signal-flow-design-plan.md`、spec:
`docs/specs/signal-flow-design.md`、design:
`docs/designs/signal-flow-design-design.md`、test-spec:
`docs/test-specs/signal-flow-design.md`）の実施記録。dispatch
された編集範囲は `docs/` の Signal Flow 設計文書チェーンと
`evidence/sv4/` の新規ファイルに限定され、`crates/`・manifest・
CI 設定は変更していない。

## Implementation

### Changes

- `docs/specs/signal-flow-design.md`: 要件 SFD-R1〜R8（階層・
  Port/Connection・編集・Probe・SourceMap/世代証拠・受入追跡・
  Owner 境界・未決分離）、Issue #4 受入 6 項目の転記、Deferred
  Design Concerns、SF-01〜06 への depends-on と #104〜109/
  Ruviform への references。
- `docs/designs/signal-flow-design-design.md`（status: approved）:
  Canonical Design §A（階層モデル）・§B（Typed Port/Connection）・
  §C（Semantic Routing 表示/編集）・§D（Probe 表示/編集）・
  §E（SourceMap/Configured/Observed 受入仕様・scenario ID 対応表
  SFD-HIER-01/02・SFD-ROUTE-01/02・SFD-VALID-01・SFD-EDIT-01・
  SFD-PROBE-01）、Decision Log D1〜D15、Deferred Design Concerns。
  Probe 状態語彙は実装 enum（`ProbeResolveState`）に整合させ、
  Issue 本文の概念名との対応を注記。
- `docs/plans/signal-flow-design-plan.md`（status: approved）:
  単一ストリーム・file map・実装順序・検証行列・review handoff。
- `docs/tasks/signal-flow-design-task.md`（status: in-progress→
  completed）: 実行 task と Done When/Verification。
- `docs/test-specs/signal-flow-design.md`: 文書受入ルール
  SFD-T1〜T7（チェーン完全性・要件カバレッジ・受入追跡・語彙整合・
  未決分離・証拠誠実性・Owner 境界）。
- `docs/impl/exp/signal-flow-design.md`: 語彙照合・契約確認・
  文書規約の探索記録（叙述形式の exp、`.md`）。
- `evidence/sv4/design-rationale.md`: Decision Log 根拠・代替比較・
  保留/非採用の記録。
- `evidence/sv4/consistency-check.md`: 既存 SF 系 spec/design・
  `crates/` 実装語彙との整合確認結果。
- `docs/{specs,designs,plans,tasks,test-specs,impl/ir}/README.md`:
  `doc_maintenance.js index apply` で再生成（手編集なし）。
  `docs/impl/exp/README.md` は変更不要（in-sync）。

### Decisions During Implementation

- design の `status` を `approved`、plan を `approved` とした。
  worker contract によりユーザー承認の代わりに review-protocol
  相当のレビュー（客観チェック＋独立 pass＋証拠検証）を実施し、
  CONFIRMED P0/P1 blocker がないことを確認した上で承認扱いとした。
  実装語彙照合で発見した Probe 状態の表記揺れ（Issue 概念名 vs
  実装 enum）は design §D.3 で実装語彙を正本として注記により解消。
- `docs/impl/ir/` の生成ファイル名は generator が `issue-4-signal-
  flow.md` を出力したため、dispatch 指定の `signal-flow-design.md`
  に rename した（内容・id は generator 出力を保持）。
- graph router（focus `signal-flow-design`）は repository 全体の
  既存 broken relation で `blocked` を返したため、文書作成は
  skill 手順に従い直接実施。外部要因として PR 本文に記載。

## Related Experiments

### Adopted

- `docs/impl/exp/signal-flow-design.md` — Issue 本文契約の構造確認、
  `ProbeResolveState`/`RoutingEditError`/`SignalFlow*` 型の実装
  語彙照合、文書チェーン規約と generator gate の確認、graph
  router blocked の外部要因記録。design の語彙整合と scenario ID
  設計の根拠として採用。

### Rejected

なし。

## Validation

- `node .agents/skills/doc-maintenance/scripts/doc_maintenance.js
  index apply --dir docs/{specs,designs,plans,tasks,test-specs,
  impl/ir,impl/exp}` → 各 dir で written（exp は 0 written・既に
  in-sync）、blocked=0。
- `node .agents/skills/doc-maintenance/scripts/doc_maintenance.js
  index plan`（引数なし）→ 10 target 全て `in-sync`（changes=0）。
- `mise run fmt-check`（`cargo fmt --all -- --check`）→ exit 0。
- relation 解決チェック: design/spec の `docs/` relations 全件が
  既存ファイルに解決（test-spec 作成前の一時的 missing を除く）。
- OS: Windows（worktree `C:\Users\akiyoshi\orca\workspaces\
  synvelis\sv4-signal-flow`）、toolchain: Node.js v26.12.1、
  doc-maintenance/impl-doc の bundled script を直接実行、
  `mise run fmt-check` は mise 経由。
- 本 IR の audit: `node .agents/skills/impl-doc/scripts/
  audit_impl_record.js --json` → 本 IR の finding 0 件
  （repository 全体の error 11・warning 66 は既存文書由来で
  本タスク範囲外）。

## Risks

- 文書受入のみで実装との乖離は未検証 — 後続 slice（#104〜109）
  が本書を authoritative 参照として実装し、#109 oracle が実装
  契約を検証する。
- scenario ID（SFD-*）が #109 Case registry と混同される危険 —
  「設計追跡子」と明記済み。
- doc-driven-dev-graph router の blocked は repository 全体の既存
  broken relation が原因で本タスク範囲外 — 修復は別タスク。

## Follow-ups

- 実機 GUI/Plugin/Device 受入は #114/#109 の外部 gate（本タスク
  では NotRun）。
- Visual Group 保存粒度・Channel Map UI 既定・特殊 Feedback・
  GUI 数値閾値は Decision Log「保留」— 該当 slice で決定。
- repository 全体の broken relation 修復は別タスク（sf-03 spec
  の `defers` 等）。
