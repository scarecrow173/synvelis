---
id: "EXP-rcd02-blob-sha256-reconcile"
type: "experiment"
status: "completed"
title: "RCD-02 Blob SHA-256 — publish 失敗の observation-reconcile と port 注入 seam"
created: "2026-11-15"
updated: "2026-11-15"
owners: []
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
    - "docs/impl/ir/rcd-02-blob-sha256.md"
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/rcd-02-blob-sha256-design.md"
    - "docs/test-specs/rcd-02-blob-sha256-digest.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "docs/tasks/rcd-02-task4-blob-sha256.md"
  defers: []
  deferred-by: []
---

# RCD-02 Blob SHA-256 — publish 失敗の observation-reconcile

## 仮説

`StagedBlobPublisher` の `BlobPublishError` が運ぶ
`PublishEffect`（`NotPublished`/`Published`/`Indeterminate`）を
そのまま `PublishError` へ写像するより、失敗報告後に destination
blob を `open_blob_verified` で観測して採用可否を決める方式なら、
効果分類の解釈差（Published=保証失敗、Indeterminate=結果不明）を
吸収しつつ fail-closed を保てる — ただし「観測で verified bytes
が無いのに retry すると二重 publish にならないか」は検証が要る。

## 方法

`publish_recorded_with` に `&dyn BlobPublisher` +
`ProviderContext` の port を設け、fault-injecting double で
効果分類ごとの publish 結果を注入する test
（`rcd02_blob_digest.rs`）を先に書き、reconcile 実装を
red→green で確認した。

## 観測結果

- `PublishMode::CreateNew` + content-addressed destination の
  ため、観測後の retry は「同名 verified blob が実在すれば
  publisher 側も既存 blob を再検証して reuse」または「不在なら
  新規 publish」で収束し、二重 publish・silent overwrite は
  起きない（library 契約 + test で確認）。
- `Published` effect の失敗（publish 後の保証失敗）でも
  destination が verified bytes として実在すれば採用できる —
  観測 reconcile により分類を信用しない設計が機能した
  （`published_effect_failure_is_reconciled_by_observation`）。
- `Indeterminate` で destination が実在すれば採用、
  `MissingMedia` なら 1 回だけ retry、再失敗後の再観測でも
  不在なら fail-closed（`indeterminate_publish_is_reconciled_
  by_observation`・`unlanded_publish_retries_once_then_fails_
  closed`）。
- tampered blob は `open_blob_verified` の hash mismatch で
  `CorruptMedia` fail-closed — filename を信用しない
  （`tampered_sha256_blob_fails_closed`）。

## 結論

採用: publish 失敗は effect 分類を message に保持しつつ、
採用可否は destination の verified 観測のみで決める
observation-reconcile。retry は `MissingMedia` 確定時の 1 回に
限定し、最終失敗は必ず観測後に `PublishError` へ写像する。
