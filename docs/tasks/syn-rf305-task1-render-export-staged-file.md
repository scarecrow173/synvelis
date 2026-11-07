---
id: "TASK-034T2RF305WAVSTAGED01A"
type: "task"
status: "done"
title: "SYN-RF305 task1 render/player WAV Export StagedFile 移行"
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
    - "docs/plans/syn-rf305-wav-export-staged-file.md"
  implemented-by: []
  depends-on:
    - "docs/plans/syn-rf305-wav-export-staged-file.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/syn-rf305-export-staged-publish-contract.md"
  references: []
  defers: []
  deferred-by: []
---

# SYN-RF305 task1 render/player WAV Export StagedFile 移行

## Work

`crates/synvelis-render` と `crates/synvelis-player` の旧
`ruviform_media_io::{StagedOutput, PublishOptions}` 使用を、
`ruviform-io-file` の `StagedFile`/`SealedFile`/`PublishOptions`/
`PublishReceipt`/`PublishFailure` 契約へ移行する。

- `spec.rs`: `WrapSink`/`DynVerify`/`DynPublish`/`DynMuxerCtor` を
  `for<'a>`/`&mut SealedFile`/`PublishReceipt`/`PublishFailure`
  型へ更新し、`OutputHooks` ドキュメントを整合。
- `export.rs`: `StagedFile::create`・`WavMuxer<&mut StagedFile>`・
  `verify(&mut SealedFile)`・`SealedFile::publish`・
  `PublishMode`/`Durability::FileSync` 写像・非 Clone
  `RenderReceipt` 対応。
- `verify.rs`: `reopen_check` を `open_readback` → byte 複写 →
  `Cursor` 独立 demux/decode へ。
- `l2_e2e.rs`: `BoundedSink`/`FailFlushSink` を `&mut StagedFile`
  借用型へ。publish 失敗は destination directory による実
  `PublishFailure`（`NotPublished`）注入へ。
- `pc_b.rs`: `RenderOutput` 直接構築を新契約へ。dev-dep 追加。
- `Cargo.lock` の ruviform 全 crate を候補 SHA `d68b14d` へ揃え、
  `ui/vendor` 再同期。

## Verification

- `mise run fmt-check` / `check` / `lint` / `test`（workspace,
  `--locked`）
- `mise run check-headless-deps` / `check-ruviform-rev` / `test-rt`
- 旧 API 名の workspace 検索ゼロ
- test-spec: `docs/test-specs/syn-rf305-export-staged-publish-contract.md`

## Evidence

`docs/impl/ir/syn-rf305-render-export-staged-file.md`
