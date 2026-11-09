---
id: "SPEC-alib02-v5snap"
type: "spec"
status: "draft"
title: "ALIB-02 deferred — v5 snapshot 互換移行（SHA-256/旧 CRC digest 共存）"
created: "2026-11-09"
updated: "2026-11-09"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/86"
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
    - "docs/specs/alib-02-streaming-verified-import.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by:
    - "docs/specs/alib-02-streaming-verified-import.md"
---

# ALIB-02 deferred — v5 snapshot 互換移行

ALIB-02（`SPEC-034UviRHTzZZY5QqaoRYZV`）から deferred された残件の置き場。

## What Should Be Built（骨子）

旧 schema（v5 系 snapshot・CRC16 flat blob digest）で保存された Project が、
SHA-256 名前空間導入後も lossless または安全に degraded で読み込めること。
正常/不正/欠落/旧 CRC 一致でも Bytes 差の各 snapshot について、provenance・
asset/source ID・opaque bytes（plugin state 等）を保全する。

## Why Now

ALIB-02 は新規 import の SHA-256 識別子のみを導入し、既存 asset の digest
形式移行・snapshot 読み込み互換は意図的に範囲外とした。旧 digest asset は
引き続き `LocalBlobDir` の flat 解決で読めるため緊急性はないが、schema
version が進む前に移行契約が必要。

## Re-engagement trigger

`synvelis-project` の `SCHEMA_VERSION` を 8 から進める slice、または
旧 blob namespace の廃止・移行を扱う Issue が起票された時点。
