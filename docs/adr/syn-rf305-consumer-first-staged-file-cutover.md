---
id: "ADR-034T2RF305WAVSTAGED01C"
type: "adr"
status: "accepted"
title: "SYN-RF305 — WAV Export Consumer-first StagedFile cutover と publish durability 写像"
created: "2026-11-07"
updated: "2026-11-07"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/82"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/syn-rf305-wav-export-staged-file.md"
  implemented-by:
    - "docs/designs/syn-rf305-wav-export-staged-file-design.md"
  depends-on:
    - "docs/specs/l2-master-sequential-offline-wav-export.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/syn-rf305-wav-export-staged-file.md"
    - "docs/designs/syn-rf305-wav-export-staged-file-design.md"
  refines: []
  refined-by: []
  derives-from:
    - "https://github.com/scarecrow173/ruviform/issues/305"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/syn-rf305-export-staged-publish-contract.md"
  references:
    - "https://github.com/scarecrow173/synvelis/issues/25"
    - "https://github.com/scarecrow173/ruviform/issues/305"
    - "https://github.com/scarecrow173/ruviform/pull/391"
  defers: []
  deferred-by: []
metadata:
  adr:
    decision-makers: ["scarecrow173"]
    consulted: []
    informed: []
---

# SYN-RF305 ADR — WAV Export Consumer-first StagedFile cutover

## Status

Accepted (2026-11-07)

## Context and Problem Statement

Ruviform PR391（RF305）は `ruviform-media-io` の旧
`StagedOutput`/`PublishOptions` を撤去し、`ruviform-audio::render`
の `RenderOutput` を `StagedFile`/`SealedFile` 型状態契約へ置き換える。
Synvelis の WAV Export 経路（`synvelis-render` + `synvelis-player`
`pc_b`）は旧 API のままで、PR391 merge 時点で Consumer が壊れる。
いつ・どの形で Synvelis 側を移行するかが本 ADR の決定対象。

## Decision

**Consumer-first cutover を採用する。** Synvelis 側を先に
`ruviform-io-file` 契約へ移行し、最終候補 SHA
`d68b14d082128b66ec1dd1328f82e8251dfba694`（RF main `e8adcc0`
+ cutover merge + docs-only 更新。code は中間候補 `7ead144` と
同一）で compile/test 証拠を取る。manifest は既定
ブランチ追従のまま、`Cargo.lock` が候補 SHA を記録する。

## Considered Options

- **Option A — Consumer-first cutover（採用）**: SV 側を先に新 API
  へ移行し、候補 SHA で検証。PR391 merge 後は `cargo update` で
  main HEAD へ再解決するだけで完了する。旧 API 名を残す
  Facade/alias を一切作らない。
- **Option B — PR391 merge 後追従**: RF 側 merge まで待ってから
  移行。Consumer が broken な期間が生じ、PR391 の merge 前提
  （Consumer 互換証拠）を満たせない。不採用。
- **Option C — 新旧互換 Facade**: `StagedOutput` 名を残す型 alias
  で main と候補の双方で compile 可能にする。旧 `RenderOutput`
  （`StagedOutput`/path verify/`Result<(),MediaError>` publish）
  と新契約（`StagedFile`/`&mut SealedFile`/`PublishReceipt`/
  `PublishFailure`）は構造的に非互換であり、型 alias では橋渡し
  不能。禁止条件「互換 Facade・型 alias で旧 API 名を残さない」
  にも抵触。不採用。
- **Option D — publish durability を `FileAndDirectorySync` へ
  強化**: 旧 `sync_before_publish` は file sync のみを要求して
  いた。`FileSync` は旧契約の忠実写像であり、`FileAndDirectorySync`
  は destination の directory entry 耐久性まで要求する強化。
  本移行は behavior change を持ち込まない Consumer cutover で
  あり、durability 要件の強化は Export の要求議論として別 Issue
  で決定する。**`FileSync` を採用**（pc_b の
  `sync_before_publish:false` は `BestEffort` へ写像）。

## Consequences

- `Cargo.lock` の ruviform 全 crate が候補 SHA `d68b14d` に揃う。
  解除条件は RF391 が main へ merge され通常の `cargo update`
  で再解決される時点 — PR 本文と IR に方法/SHA/解除条件を記録。
- `PublishFailure` は `RenderError::Publish` 経由で typed のまま
  `ExportError::Render` に保持される（phase/kind/effect/
  durability/recovery を flatten しない）。
- `Indeterminate` の Consumer 側決定的注入は `PublishFailure`
  非公開 constructor のため不可 — typed 分類経路を pin するに
  留め、生成責務は RF 側 mock/driver 試験へ。
