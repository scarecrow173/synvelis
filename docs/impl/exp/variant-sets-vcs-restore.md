---
id: "EXP-variant-sets-vcs-restore"
title: "variant_sets restore-as-copy provenance 修復 設計検証メモ (synvelis#458)"
created: "2026-11-14"
updated: "2026-11-14"
---

# 実験記録: variant_sets を含む restore_as_copy の provenance 扱い（synvelis#458）

## E1 修復方針: nested identity 再採番 vs persisted 境界の規則見直し

**仮説**: `fork_project` が top-level `project_id` のみ再採番し
nested `VariantSet::origin`/`Branch::created_from` の
`SnapshotIdentity.project_id` を source project のまま残す
現行実装は、CB-01 の authored provenance 意味論と整合するか。
整合するなら validate 規則側を直すべきで、整合しないなら
`fork_project` で再帰再採番すべきか。

- 案 A（`fork_project` で nested `SnapshotIdentity.project_id`
  を再帰再採番）: restore 先 project 内の全 provenance を
  「同 project 由来」に書き換える。`validate_set_shape` の
  same-project 規則を一切緩めずに済むが、`SnapshotIdentity` は
  `{project_id, source, locator, digest, schema_version,
  projection_version, capture_generation}` の authored 参照で、
  `digest` は source snapshot bytes を、`locator` variant によっては
  source `project_id` を埋め込む。`project_id` だけを書き換えると
  digest/locator が指す実在 snapshot との対応が壊れ、「存在しない
  snapshot を source とする provenance」を捏造する。fork 元が
  実際にどの project のどの snapshot から分岐したかという
  CB-01 の監査情報が失われる。
- 案 B（persisted/load 境界で same-project 規則を緩和、採用）:
  `SnapshotIdentity` は write-only provenance で、snapshot 解決
  経路は存在しない（探索確認済み）。live op 境界
  （`VariantOp::Fork::validate`）では fork が live identity を
  by-construction で刻むため same-project を維持し、load oracle
  （`validate_persisted`）では source-project provenance を
  受理する `ProvenanceRule` を導入する。

**観察**: 案 A は「validation を通すために provenance を
falsify する」補正であり、AGENTS.md の「不正な状態を生む源を
修正する」原則に反する。違反していた不変条件は「persisted
project の provenance は同 project 由来」という過剰な一般化で
あり、真の不変条件は「live op が刻む provenance は同 project」
+「persisted provenance は authored 参照として verbatim」だった。
案 B はこの 2 つを境界ごとに分離し、restore copy が source
project の来歴を正直に保持する。design `variant-sets-vcs-restore`
D1–D4 として確定。

## E2 live op 境界での same-project 再課の要否

**仮説**: `validate_set_shape` を全面緩和すると live
`VariantOp::Fork` でも foreign provenance が通るか。

- 観察: `VariantOp::Fork::validate` は op 適用前の project に
  対して呼ばれ、fork 先 set の `origin`/`created_from` は
  `engine.rs` の fork 実装が live `SnapshotIdentity`（現在 project
  の capture）を刻む。よって live op では same-project が
  by-construction で成立し、規則を残しても実害はない。
- 判断: `ProvenanceRule::SameProject` を live op 側に残すのは
  「将来 fork 実装が外部 snapshot identity を受け取る経路が
  増えた場合の fail-closed」を維持する防御であり、コストは
  enum 分岐 1 つのみ。design D3 で維持を確定。

## E3 `stable_id_counts` の variant-set/branch 追加範囲

**仮説**: receipt の verbatim 検証に必要な count 追加は
`variant-set`/`branch` の 2 エントリで十分か。

- 観察: Issue AC は「issued `VariantSetId`/`BranchId`・branch
  revision・`BranchContentRef`・`last_adopt` が verbatim 保持」
  を envelope readback で検証することを要求。envelope 自体が
  `variant_sets` map 全体を保持するため readback は map 等価で
  完結するが、receipt（`StableIdReport`、policy "preserve"）が
  variant 系 ID を報告しないと「restore が発行済み stable ID を
  保持した」という receipt レベルの証拠が欠ける。
- 判断: `variant-set` = `variant_sets.len()`、`branch` = 全 set
  の `branches.len()` 合計の 2 エントリ追加で十分
  （branch revision/content ref は branch 数に内包され、
  個別 entity 種の count は不要）。
