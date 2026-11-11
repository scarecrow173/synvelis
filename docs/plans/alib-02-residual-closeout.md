---
id: "PLAN-034VMQQ4mGuzIS1AUxu1MS"
type: "plan"
status: "completed"
title: "ALIB-02 residual closeout — Issue #86 DoD 照合と証拠固定の実装計画"
created: "2026-11-11"
updated: "2026-11-11"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/86"
  changes:
    added:
      - type: "file"
        path: "docs/specs/alib-02-residual-closeout.md"
      - type: "file"
        path: "docs/designs/alib-02-residual-closeout-design.md"
      - type: "file"
        path: "docs/plans/alib-02-residual-closeout.md"
      - type: "file"
        path: "docs/tasks/alib-02-residual-closeout.md"
      - type: "file"
        path: "docs/impl/ir/alib-02-residual-closeout.md"
      - type: "file"
        path: "evidence/issue-86-v5-migration/closeout-verification.md"
    modified:
      - type: "file"
        path: "docs/specs/alib-02-v5-snapshot-compat.md"
      - type: "file"
        path: "docs/plans/alib-02-v5-snapshot-compat.md"
      - type: "file"
        path: "docs/test-specs/alib-02-streaming-verified-import.md"
      - type: "file"
        path: "docs/impl/ir/alib-02-streaming-verified-import.md"
      - type: "file"
        path: "docs/impl/ir/alib-02-v5-snapshot-compat.md"
      - type: "file"
        path: "docs/impl/ir/alib-02-v5-snapshot-compat-slice.md"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/alib-02-residual-closeout.md"
  implemented-by:
    - "docs/tasks/alib-02-residual-closeout.md"
  depends-on:
    - "docs/designs/alib-02-residual-closeout-design.md"
  blocks:
    - "docs/tasks/alib-02-residual-closeout.md"
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/alib-02-v5-snapshot-compat.md"
    - "docs/test-specs/alib-02-streaming-verified-import.md"
    - "docs/test-specs/issue-86-verified-import-wiring.md"
  references: []
  defers: []
  deferred-by: []
---

# ALIB-02 residual closeout — Issue #86 DoD 照合と証拠固定の実装計画

## Goal

Issue #86 の受入条件/DoD 行を実在 fixture・テスト・canonical lane
結果と 1:1 照合し、doc chain・evidence・IR に簿記として固定する。
プロダクションコードの変更は含まない。

## Scope

単一 stream の docs/evidence-only 変更。分割不要。
編集境界は worker spec の占有リストに限定する。

## Upstream Documents

- `docs/specs/alib-02-residual-closeout.md`（本 plan が実装する spec）
- `docs/designs/alib-02-residual-closeout-design.md`（設計入力）
- Issue: <https://github.com/scarecrow173/synvelis/issues/86>

## Design Inputs

- `docs/designs/alib-02-residual-closeout-design.md`

## File Map

| File | 変更 |
| --- | --- |
| `docs/specs/alib-02-residual-closeout.md` | DoD/受入条件照合 ledger（新規） |
| `docs/designs/alib-02-residual-closeout-design.md` | 照合・gate 分類の設計（新規） |
| `docs/plans/alib-02-residual-closeout.md` | 本計画（新規） |
| `docs/tasks/alib-02-residual-closeout.md` | task checklist（新規） |
| `docs/impl/ir/alib-02-residual-closeout.md` | 照合結果・lane 結果・残存 gate の IR（新規） |
| `evidence/issue-86-v5-migration/closeout-verification.md` | lane 出力要約・再現手順（新規） |
| `docs/specs/alib-02-v5-snapshot-compat.md` | status `implemented` 化・upstream relation 補完 |
| `docs/plans/alib-02-v5-snapshot-compat.md` | `implemented-by` 充填（task + IR×2） |
| `docs/test-specs/alib-02-streaming-verified-import.md` | `import.rs` テスト数の実測追従（15→21） |
| `docs/impl/ir/alib-02-streaming-verified-import.md` | Follow-ups を closeout 済み実態に整合 |
| `docs/impl/ir/alib-02-v5-snapshot-compat.md` | related 追記・残件注記の整合 |
| `docs/impl/ir/alib-02-v5-snapshot-compat-slice.md` | Verification を closeout 再実測値に更新 |

## Implementation Sequence

- [x] Issue #86 本文と doc chain・実在テスト/fixture を照合し
      DoD ledger を作成
- [x] canonical lane 7 本を `mise run` で実走し結果を記録
- [x] spec/design/plan/task/IR の新規 doc chain を作成
- [x] 既存 editable doc の status/relations/checkbox を整合
- [x] `evidence/issue-86-v5-migration/closeout-verification.md` を追記
- [x] doc_status lint/audit・audit_impl_record を実走し新規
      finding なしを確認
- [x] Issue #86 へコメント・PR 作成（Issue close/merge はしない）

## Dependencies and Risks

- `mise run test-msrv` は rust 1.89 toolchain で workspace 全 test を
  再実行するため最長 lane。toolchain は `mise install` 済みを確認。
- doc 系監査は repo-wide baseline に既存 finding（legacy invalid-id
  系・一部 reciprocal 警告）を抱える。判定基準は「変更ファイルに
  新規 finding を増やさないこと」。
- 並行 Worker が同 repo で稼働中のため、占有リスト外の file に
  触れないことを diff で最終確認する。

## Verification

- `mise run fmt-check` / `check` / `lint` / `test` / `build` /
  `test-rt` / `test-msrv` — canonical、結果は IR Validation と
  evidence に記録。
- `doc_status.js lint` / `audit`（対象 type 絞り込み）・
  `audit_impl_record.js` — 変更 doc の整合確認。
- `git diff --stat` で占有リスト外の変更が無いことを確認。
