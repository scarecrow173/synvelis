---
id: "PLAN-034T2RF305WAVSTAGED00Z"
type: "plan"
status: "approved"
title: "SYN-RF305 WAV Export StagedFile 移行計画"
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
    - "docs/designs/syn-rf305-wav-export-staged-file-design.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/syn-rf305-export-staged-publish-contract.md"
  references: []
  defers: []
  deferred-by: []
---

# SYN-RF305 WAV Export StagedFile 移行計画

## 目的

`docs/specs/syn-rf305-wav-export-staged-file.md` に従い、WAV
Export/player の旧 `ruviform_media_io::{StagedOutput,
PublishOptions}` 使用を `ruviform-io-file` の
`StagedFile`/`SealedFile`/`PublishOptions`/`PublishFailure`/`PublishReceipt`
契約へ移行し、Ruviform PR391 の merge 前提となる Consumer 互換を
先に確立する。

## 前提・入力

- RF 候補 SHA `d68b14d082128b66ec1dd1328f82e8251dfba694`
  （main `e8adcc0` + cutover merge）。`Cargo.lock` で全 ruviform
  crate をこの SHA に揃え、`sync-ruviform-ui` で vendor 再同期。
- 所有範囲: `synvelis-render`（`export.rs`/`spec.rs`/`verify.rs`/
  `l2_e2e.rs`）と `synvelis-player`（`pc_b.rs`）。Store #84
  （`synvelis-project/src/store.rs`）は統合済みで今回の範囲外。

## 作業単位

| Task | 内容 | 依存 |
| --- | --- | --- |
| task1 | render/player 全 call site の移行・依存宣言・lock 更新・fault-injection 整合・正規検証・レビュー | 候補 SHA |

## 実施手順（task1 内の順序）

1. `Cargo.lock` を `cargo update -p ruviform-media-io --precise
   d68b14d…` で全 ruviform crate 一括更新 → `sync-ruviform-ui` →
   `check-ruviform-rev`。
2. `spec.rs`/`export.rs`/`verify.rs` を新 hook/lifecycle 契約へ
   変更、`ruviform-io-file` 依存を両 Cargo.toml へ追加。
3. `l2_e2e.rs`（sink helper・publish 失敗注入）と `pc_b.rs`
   （`RenderOutput` 直接構築）を整合。
4. `mise run fmt-check`/`check`/`lint`/`test` 実測。
   `check-headless-deps`・`test-rt` も関連 lane として実施。
5. 独立レビュー（A/B → Validator → Judge）→ 指摘修復 →
   Conventional Commit → branch push → 日本語 PR。

## 検証

- 正規 task: `fmt-check`/`check`/`lint`/`test`（workspace,
  `--locked`）+ `check-headless-deps` + `check-ruviform-rev`。
- 試験契約: `docs/test-specs/syn-rf305-export-staged-publish-contract.md`
  に従い、既存 L2/X/player 試験を全 pass で保持し、publish
  失敗を実 adapter の `PublishFailure` で pin する。

## リスクと対処

- **新旧 `RenderOutput` 契約の非互換**: lock を候補 SHA へ揃える
  ため、main 上の旧契約との両立実装は作らない（Facade 禁止）。
  解除条件は RF391 merge 後の再解決。
- **`PublishFailure` 非公開 constructor**: Consumer 側では実 OS
  adapter 経由の失敗（directory destination）のみ使用する。
  `Indeterminate` 注入は RF mock 側の責務として test-spec に明記。

## 完了条件

- spec の Acceptance Criteria 全項・レビュー通過・PR 作成。
