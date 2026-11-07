---
id: "TSPEC-034T2RF305WAVSTAGED01B"
type: "test-spec"
status: "approved"
title: "SYN-RF305 Export StagedFile Publish 契約試験"
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
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/syn-rf305-wav-export-staged-file-design.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/syn-rf305-wav-export-staged-file.md"
  derived-by: []
  verifies:
    - "docs/specs/syn-rf305-wav-export-staged-file.md"
    - "docs/designs/syn-rf305-wav-export-staged-file-design.md"
    - "docs/adr/syn-rf305-consumer-first-staged-file-cutover.md"
  verified-by: []
  references:
    - "https://github.com/scarecrow173/synvelis/issues/25"
    - "https://github.com/scarecrow173/ruviform/issues/305"
    - "https://github.com/scarecrow173/ruviform/pull/391"
  defers: []
  deferred-by: []
---

# SYN-RF305 Export StagedFile Publish 契約試験

## 対象

`crates/synvelis-render`（`export.rs`/`spec.rs`/`verify.rs`/
`tests/l2_e2e.rs`）と `crates/synvelis-player/tests/pc_b.rs` の
`ruviform-io-file` staged publish 移行。

## 既存試験の保持（全て非削減）

- `l2_e2e.rs` L2-01..08 全試験、`x_e2e.rs`、`pc_b.rs` を新契約下で
  pass — 正常 `Completed`・PCM 高精度 fixture・golden・capability
  evidence・snapshot freeze/Project 非変化回帰は不変。

## 所有境界ごとの観測点

| ケース | 観測 | 既存/新規 |
| --- | --- | --- |
| 正常 `Completed` | `RenderReceipt`（`publish: PublishReceipt`）+ `VerifiedOutput`（`reopened`） | 既存 L2-01..03 |
| destination 既存 + `CreateNew` | `PreflightError::DestinationConflict`・destination byte 不変・temp 残滓なし | 既存（事前拒否） |
| destination 既存 + `Replace` | 正常完了・旧 byte 置換 | 既存 |
| writer full / flush 失敗 | `Output`/`Finalize`/`Drain` stage・destination 不生成・temp 残滓なし | 既存（`BoundedSink`/`FailFlushSink` を `&mut StagedFile` 型へ） |
| verify 拒否（`expect_pcm=false`） | `ExportStage::Verify`・publish 不実行・destination 不生成 | 既存 |
| publish 失敗 | `ExportStage::Publish` + `RenderError::Publish{source: PublishFailure}`・`effect() == NotPublished`・destination directory 保全・temp 残滓なし | 既存（注入方法を実 adapter 失敗へ更新） |
| cancel | `Cancelled{phase}`・destination 不生成・temp 残滓なし | 既存 |

## 新契約固有の観測点

- **型境界**: `WavMuxer<&mut StagedFile>` が `make_muxer` 借用契約
  で構築され、muxer drop 後に `staged.seal()` → verify → publish
  の順序が driver により強制される（compile 証拠 + `Completed`
  試験の成功が順序成立の間接証拠）。
- **readback 独立性**: 既定 verify が `SealedFile::open_readback`
  経由で読取 byte を独立 buffer へ複写し、symphonia demux +
  `SourceReader` canonical decode で rate/channel/frames/PCM を
  検証する（`VerifiedOutput.reopened == true` は receipt に保持）。
- **publish typed 保全**: publish 失敗注入が `PublishFailure` の
  `effect()`/`kind()`/`phase()`/`info()` を flatten せず
  `ExportError::Render` 内に保持することを `effect() ==
  PublishEffect::NotPublished` で pin。
- **旧 API 排除**: `crates/` 配下に `StagedOutput`・旧
  `PublishOptions` 型・`open_boxed_muxer` 参照が 0 件
  （grep sweep）。

## カバレッジ外（明示）

- `PublishEffect::Indeterminate` の決定的注入: `PublishFailure`
  に公開 constructor がなく、`ruviform-io-file` の fault-injection
  mock は `#[cfg(test)]` internal。Consumer 側で誠実な生成経路が
  なく、RF 側 mock/driver 試験の責務。本試験が pin するのは
  「publish 失敗の typed 分類・evidence 保全」経路であり、
  `Indeterminate`/`Published` も同一コード経路を通る。
- 実電源断・クラッシュ中の OS 実機回復 — adapter 契約（RF 側）。

## 実行

- `mise run test`（workspace `--locked`）— `l2_e2e`/`x_e2e`/`pc_b`
  含む全 binary 0 fail。
- `mise run fmt-check` / `check` / `lint` / `check-headless-deps` /
  `check-ruviform-rev` / `test-rt`。
