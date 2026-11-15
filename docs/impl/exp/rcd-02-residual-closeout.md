---
id: "EXP-rcd02-residual-closeout-consolidate"
type: "experiment"
status: "completed"
title: "RCD-02 residual closeout — AC 照合の consolidate 手法と index dup row 解消"
created: "2026-11-15"
updated: "2026-11-15"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/116"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by:
    - "docs/impl/ir/rcd-02-residual-closeout.md"
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/rcd-02-residual-closeout-design.md"
    - "docs/test-specs/rcd-02-residual-closeout.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/rcd-02-residual-closeout.md"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "docs/tasks/rcd-02-residual-closeout.md"
    - "evidence/sv116/rcd-02-closeout-verification.md"
  defers: []
  deferred-by: []
---

# RCD-02 residual closeout — AC 照合の consolidate 手法と index dup row 解消

## 仮説

RCD-02 の AC 照合は、新規 fixture/test を追加して再 derive する
より、既存 2 ledger（`evidence/reports/
rcd-02-durability-recovery-review-ledger.md`・
`evidence/sv116/rcd-02-blob-sha256-ledger.md`）の fixture 参照を
consolidate し、test 名の実在だけを `rg` で確認する方が、
簿記 slice の誠実性と工数の両方で優れる — ただし「ledger の
列挙が現行 main の test 名と一致するか」は検証が要る。

## 方法

- Issue #116 本文の受入条件 4 行を抽出し、各行について既存
  ledger の Case/Expected/Observed 表と `crates/*/tests/` の
  実在 test 名を `rg -n "fn \w+"` で照合した。
- `docs/test-specs/README.md` の dup row（同一
  `TSPEC-034UrlOOfZIvsdF1bdNXfE`）について、正本（各
  test-spec file の frontmatter = `approved`）を確認し、
  `doc_maintenance.js index apply --dir docs/test-specs`
  の再生成で managed index 側を `approved` に揃える方針を
  採った（手編集はしない）。ただし marker 管理外の保存
  tail 領域に stale `proposed` 重複行が残ることが後に
  判明した（下記観測結果）。

## 観測結果

- ledger の列挙 test 名は全て現行 main に実在する
  （`recovery_review.rs` 16・`project_adapter.rs` 13・
  `staged_publish.rs` 7・`vs_c.rs` 16・`vs_d.rs` 22・
  `rcd02_blob_digest.rs` 10・`real_fs_probe.rs` 2）。再 derive
  なしに 1:1 対応付けが成立した。
- `docs/test-specs/README.md` は `doc-suite:generated-index`
  marker 付きの生成物で、managed index（line 100）と marker
  管理外の保存 tail 領域（line 230、PR #212 由来の重複
  table）に同一 TSPEC ID が併存していた。正本 frontmatter
  は `approved`。`index apply` 再生成で managed index 側は
  `approved` に揃ったが、tail 領域の stale `proposed` 行は
  generator が verbatim 保存し `index plan` が in-sync を
  返すため修復できず、残存 finding として記録した
  （手編集禁止のため PR 本文「未対応の作業」に明記）。
- AC-3（CRC32/SHA-256 分離・Legacy v5/Asset/Project Schema
  migration 相互参照・旧 Snapshot/HEAD 保護）は RCD-02 単体
  ではなく ALIB-02（synvelis#86）・project 側 HEAD CAS
  （synvelis#84）にまたがる横断行であることが照合で判明 —
  RCD-02 側の責務分のみを Pass とし、横断部分は gate 記名の
  NotRun/Deferred として分離した。

## 結論

採用: AC 照合は既存 ledger の consolidate + test 名実在確認の
みで行い、新規 fixture/test は作らない。test-specs index の
managed 領域は index 再生成で正本 `approved` に揃え、
tail 領域の stale `proposed` 重複行は generator 管理外の
ため残存 finding として記録する（手編集しない）。
AC-3 の横断部分は gate 記名で分離する。
