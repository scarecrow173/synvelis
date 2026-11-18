---
id: "SPEC-548StripDomLowr02"
type: "spec"
status: "approved"
title: "Issue #548 spec — strip-domain inner op lowering（CB-06 後続）"
created: "2026-11-19"
updated: "2026-11-19"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/548"
    - "docs/discovery/issue-548-strip-domain-lowering.md"
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
    - "docs/discovery/issue-548-strip-domain-lowering.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/issue-548-strip-domain-lowering.md"
    - "docs/test-specs/cb-06-track-bus-adapter.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/discovery/issue-548-strip-domain-lowering.md"
  derived-by:
    - "docs/designs/issue-548-strip-domain-lowering.md"
    - "docs/plans/issue-548-strip-domain-lowering.md"
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# Issue #548 spec — strip-domain inner op lowering

## 目的

CB-06 の strip-fields scope で、mask 宣言済みの strip-domain
inner op が wire capability 表経由で lower され、candidate
edit として受理される positive 経路を gateway に開く。
`cb06_strip_gateway.rs` の fail-closed 証拠を positive
mask-gated edit の検証へ置き換え、CB-06 の残 AC に寄与する。

## 受入条件（Issue #548 転記）

- [ ] strip-fields scope の inner op が wire capability 表経由で
      lower され、positive mask-gated edit が typed reject
      ではなく candidate edit として受理される
- [ ] `crates/synvelis-gateway/tests/cb06_strip_gateway.rs` の
      fail-closed 証拠（variant-unsupported reject）が positive
      経路の検証に置き換わる
- [ ] 既存の boundary 証拠（scope digest / branch revision pin /
      adopt provenance）は維持される

## Non-goals

- `synvelis-project` 側の adapter 本体・mask・
  `cb06_strip_fields.rs` の変更（consume のみ）
- routing/send/connection/sidechain/bus op の candidate 受理、
  scope entity removal（別 slice）
- strip 以外の domain の lowering 追加・positive 化
- `crates/synvelis-gateway/src/` の `prepare.rs`/
  `prepare_engine.rs` 以外の変更
- 実 PCM/実 plugin・実 GUI・実機/RT 検証（外部 gate）
- docs index の編集・再生成（Coordinator が統合時に実施）

## 制約

- PR 本文・コミットメッセージは `Refs #548` のみ。
  `Closes`/`Fixes`/`Resolves` 系は使わない。Issue は OPEN 維持。
- 他 Issue/リポジトリ参照は inline code 形式。
- 新規依存・feature 追加なし。
- capability id・schema・TargetContract・effect bounds は
  GW-04 op 表慣例（`synvelis.<domain>.<op>`、version 1.0、
  書き込み先資産に合わせた `TargetContract`、正直な effect
  bounds）に従う。
- `op_facts` に reads + preconditions を正直に宣言し、
  fail-closed（unsupported observable effects）を維持する。
