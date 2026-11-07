---
id: "SPEC-034T2RF305WAVSTAGED00X"
type: "spec"
status: "approved"
title: "SYN-RF305 WAV Export/Player StagedFile 移行"
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
  implements: []
  implemented-by: []
  depends-on:
    - "docs/specs/l2-master-sequential-offline-wav-export.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/syn-rf305-wav-export-staged-file-design.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/l2-master-sequential-offline-wav-export.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/syn-rf305-export-staged-publish-contract.md"
  references:
    - "https://github.com/scarecrow173/synvelis/issues/25"
    - "https://github.com/scarecrow173/ruviform/issues/305"
    - "https://github.com/scarecrow173/ruviform/pull/391"
    - "https://github.com/scarecrow173/ruviform/issues/295"
    - "https://github.com/scarecrow173/ruviform/issues/296"
  defers: []
  deferred-by: []
---

# SYN-RF305 WAV Export/Player StagedFile 移行

## What Should Be Built

Ruviform PR391（RF305）が `ruviform-media-io` の旧
`StagedOutput` / `PublishOptions` API を撤去する。Synvelis 側の
WAV Export 経路 — `crates/synvelis-render/src/export.rs`・
`spec.rs`（`OutputHooks`）と `crates/synvelis-player/tests/pc_b.rs`
の `RenderOutput` 直接構築 — は同 API に残存しており、そのまま
PR391 を merge すると Consumer が壊れる。本 spec は
`ruviform-io-file` の型状態付き staged publish
（`StagedFile` → `SealedFile` → `publish` →
`PublishReceipt`/`PublishFailure`）への Consumer 移行を定める。

- **staged 作成**: `StagedOutput::create` → `StagedFile::create`
  （`.rvf-tmp-*` sibling temp、`FileError` は `ExportError::Io`
  preflight 失敗として従来どおり写像）。
- **muxer 構築**: 旧 API が clone した `File` を muxer へ渡して
  いたのを、`for<'a> FnOnce(&'a mut StagedFile) -> Box<dyn
  Muxer + 'a>` 借用契約へ移行（`WavMuxer<&mut StagedFile>`。
  muxer drop が借用終了、`seal` の前提）。
- **verify**: 旧 path 型 hook（`&Path` で独自 `File::open`）を
  `FnOnce(&mut SealedFile, &OutputFacts)` へ。読取は
  `SealedFile::open_readback` の読取専用 surface 経由で byte を
  独立 buffer へ複写し、symphonia demux + `SourceReader`
  canonical decode で rate/channel/frame 数・PCM を検証する。
  検証失敗時は publish しない。
- **publish**: `FnOnce(SealedFile, PublishOptions) ->
  Result<PublishReceipt, PublishFailure>`。既定は
  `SealedFile::publish` へ委譲。`request.replace_existing` は
  `PublishMode::Replace`/`CreateNew` へ、旧
  `sync_before_publish: true` は `Durability::FileSync` へ
  写像する（DirSync 強化は Store #84 とは別経路、本移行では
  旧契約を忠実に維持）。
- **失敗写像**: `RenderError::Publish { source: PublishFailure,
  cleanup }` を typed のまま `ExportError::Render` 経由で
  `ExportStage::Publish` へ。effect（NotPublished/Published/
  Indeterminate）・phase・kind・durability・recovery は
  flatten しない。`PublishFailureKind::DestinationExists` は
  driver が `RenderError::DestinationExists` へ正規化し、
  SV 側 `Preflight` の `DestinationConflict` 早期拒否と両立する。

## Why Now

- RF main `e8adcc0` に `ruviform-io-file` の新 API と新
  `RenderOutput` 契約を含む PR391 候補 `d68b14d`（main merge 済
  refreshed candidate）がある。RF391 の breaking merge を可能に
  するには Consumer 側移行を先行する必要がある
  （coordinator 裁定順序: SV 移行 PR → Consumer/RF 候補 tuple
  検証 → RF391 merge）。
- SYN-IO-02（Store #84/PR178）で `ruviform-io-file` 移行の先例
  が main にあり、本 WAV 経路も同じ所有境界・失敗分類規約に従う。

## Users and Value

- Export caller（ExportPolicy/将来 UI）: publish 失敗が
  effect/phase/durability/recovery を保持した typed
  `RenderError::Publish` として届き、`Cancelled`/`Failed`/
  `Partial`/`Completed` の区別と既存 destination/raw source/
  Project 状態の保護が維持される。
- Ruviform PR391: Consumer が壊れないことが merge 前提条件。

## Scope

### In Scope

- `crates/synvelis-render/src/spec.rs` — `OutputHooks` 全 seam
  （`WrapSink`/`DynVerify`/`DynPublish`/`DynMuxerCtor`）の新型化。
- `crates/synvelis-render/src/export.rs` — `StagedFile` 作成・
  muxer 借用・verify/publish hook・`PublishOptions` 写像・
  receipt 構築。
- `crates/synvelis-render/src/verify.rs` — `reopen_check` を
  `&mut SealedFile` + `open_readback` 経由の独立 decode に更新。
- `crates/synvelis-render/tests/l2_e2e.rs` — fault-injection
  helper と publish 失敗注入を新契約へ整合（fabricated error
  を置かず、実 OS adapter の typed `PublishFailure` を使う）。
- `crates/synvelis-player/tests/pc_b.rs` — `RenderOutput` 直接
  構築の新契約化。
- 両 crate の `ruviform-io-file` 依存宣言、`Cargo.lock` の
  ruviform 解決 SHA 更新、ui vendor 再同期。

### Out of Scope

- GUI / Native Theme / Asset Provider / RF398 修復（別 owner）。
- `ExportPolicy`/`Project`/`Playback`/`RenderSpec`/`Receipt`/
  `Frame`/PCM 精度/RT/保存済 Revision の意味論変更。
- Ruviform 側の編集、新旧 API 共存 Facade・旧 API 名を残す
  型 alias、generic OS/fsync/publish のアプリ側複製。
- 新規 E2E 系の全面実装、既存 Issue の再 open。
- `Indeterminate` の Consumer 側決定的注入（`PublishFailure`
  に公開 constructor がなく、OS adapter/mock は RF 側責務。
  分類・evidence 保全経路のみ SV 側で pin する）。

## Requirements

- 不変条件: WAV finalize 成功 → muxer 借用解除 → `seal` →
  readback 独立 decode 検証 → `publish`。finalize/Drop だけでは
  `Completed` にしない。readback 失敗は publish を起こさない。
- `Published`/`Indeterminate` の temp は自動削除しない
  （driver/RecoveryHandle 契約どおり）。`NotPublished` は
  driver が best-effort cleanup する。
- 旧 API 名（`StagedOutput`・旧 `PublishOptions` 形状・
  `sync_before_publish`・`open_boxed_muxer` 経由の `File`
  clone）は workspace 検索で残らない。
- `mise run fmt-check`/`check`/`lint`/`test` が新契約下で pass。
- 既存 L2/X/player 試験・golden・高精度 fixture を弱めない。

## Acceptance Criteria

- [ ] `crates/` 内に `StagedOutput`・旧 `PublishOptions` 参照が
  0 件、`mise run check` が pass する。
- [ ] `WavMuxer<&mut StagedFile>` が構築され、muxer drop 後に
  `seal()` → `open_readback` 検証 → `publish` の順序が driver
  契約どおり実行される。
- [ ] L2-05 fault injection（writer full・flush 失敗・verify 拒否・
  publish 失敗・cancel）が全て typed な非 Completed outcome と
  なる。publish 失敗は実 OS adapter の `PublishFailure`
  （`NotPublished`）で、temp 残滓なし・既存 destination 不変。
- [ ] `Completed` receipt が `RenderReceipt`（`PublishReceipt`
  保持）と `VerifiedOutput` を含み、verify/readback 失敗では
  receipt を発行しない。
- [ ] `Cargo.lock` の全 ruviform crate が検証 SHA で一致し、
  `check-ruviform-rev` が pass。manifest には恒久的な
  branch/rev pin・vendor コピーを残さない。

## Deferred Design Concerns

- publish `Durability` の `FileAndDirectorySync` 強化 — 旧契約
  維持を優先し別 Issue 判断（Store #84 は別経路で採用済み）。
- `Indeterminate` の決定的注入試験 — RF 側 mock/fault 責務。
- GUI/Asset/他 consumer の残旧 API — 別 owner 範囲。

## Sources and Relations

- Issue #82（Consumer 設計正本）、Closed #25（既存 Export 機能・
  本追随の先行機能）、Ruviform #305/#391（旧 API 撤去側）、
  Ruviform #295/#296（`StagedFile`/OS publish 契約）。
- `docs/specs/l2-master-sequential-offline-wav-export.md`
  （本移行が変更する Export 契約の正本）。
