---
id: "PLAN-alib07-import-cancel"
type: "plan"
status: "completed"
title: "ALIB-07 slice 2 実装計画 — import cancel checkpoint・crash fixture・matrix 簿記"
created: "2026-11-10"
updated: "2026-11-10"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/91"
  changes:
    added:
      - type: "file"
        path: "docs/specs/alib-07-import-cancel.md"
      - type: "file"
        path: "docs/designs/alib-07-import-cancel-design.md"
      - type: "file"
        path: "docs/plans/alib-07-import-cancel.md"
      - type: "file"
        path: "docs/tasks/alib-07-import-cancel.md"
      - type: "file"
        path: "docs/impl/ir/alib-07-import-cancel.md"
    modified:
      - type: "file"
        path: "crates/synvelis-library/src/import.rs"
      - type: "file"
        path: "crates/synvelis-library/tests/import.rs"
      - type: "file"
        path: "docs/test-specs/alib-07-fault-matrix.md"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/alib-07-import-cancel.md"
  implemented-by: []
  depends-on:
    - "docs/designs/alib-07-import-cancel-design.md"
  verified-by:
    - "docs/test-specs/alib-07-fault-matrix.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/plans/alib-07-fault-matrix.md"
    - "docs/specs/alib-07-fault-matrix.md"
    - "docs/specs/alib-02-streaming-verified-import.md"
---

# ALIB-07 slice 2 Plan — import cancel + crash fixture

## スコープ

- `crates/synvelis-library` の `src/import.rs`（commit 相注入点）・
  `tests/import.rs`・`tests/common/`（必要時）と docs チェーン。
- 他 crate・`Cargo.toml`/`Cargo.lock`/`mise.toml`/
  `.github/workflows/` は変更しない（生成物の再生成のみ許容）。
- 実 provider/実 OS/実 device/実 GUI は対象外であり、結果を
  `NotRun`/`Unsupported`/`NotImplemented` として記録する。

## ファイルマップ

| ファイル | 変更 |
| --- | --- |
| `crates/synvelis-library/src/import.rs` | `import_verified` の pre-commit 区間に `cx.check_interrupts()?` 追加 |
| `crates/synvelis-library/tests/import.rs` | publish 相 cancel・commit 相 cancel+resume・subprocess crash（2 crash point）の test 追加 |
| `docs/specs|designs|plans|tasks|impl/ir/alib-07-import-cancel*.md` | 本 slice の docs チェーン + IR |
| `docs/test-specs/alib-07-fault-matrix.md` | baseline SHA・PR 参照・LIB-IMPORT-04/08 行・cross-link 更新 |
| `docs/*/README.md` | managed index 再生成（doc-maintenance apply） |

## リスクと対策

- **pre-commit checkpoint が既存 test を壊す**: cancel token を
  立てない既存 test では `check_interrupts` は no-op。deadline
  未設定のため `BudgetExceeded` も発火しない。`mise run test` で
  全件確認する。
- **subprocess test の再帰・ハング**: env guard で child 経路を
  分岐し、child は必ず `process::exit` で終端する（fault 注入が
  機能しなかった場合も非 0 終了で fail-closed）。親は
  `output()` で exit status を assert する。
- **child/parent の intent 不一致**: `LocalFolderProvider::pin` は
  内容 digest 由来の `object_identity` を返し process 間で決定的。
  manifest 整合検査（`object_identity`/`asset`）を両側で一致させる
  ため、同一 fixture file・同一 locator・同一 ImportId を使う。
- **doc graph の既存 blocking findings**: repo 全体に pre-existing
  の broken relation がある（route_graph は blocked を返す）。
  本 slice の docs は新規 finding を増やさないことを
  `doc_status.js lint/audit` で確認し、既存不具合の修復は
  スコープ外とする（先行 slice と同じ扱い）。

## 検証マトリクス

| 検証 | コマンド |
| --- | --- |
| format | `mise run fmt-check` |
| 型 | `mise run check` |
| lint | `mise run lint` |
| 全 test | `mise run test` |
| RT lane | `mise run test-rt` |
| MSRV | `mise run test-msrv` |
| docs | `doc_status.js lint/audit` + `doc_maintenance.js apply` |
