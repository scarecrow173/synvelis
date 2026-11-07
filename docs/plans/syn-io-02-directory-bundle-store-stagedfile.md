---
id: "PLAN-034T2c5vy3vKJPENF8AA50"
type: "plan"
status: "approved"
title: "SYN-IO-02 DirectoryBundleStore StagedFile 移行計画"
created: "2026-11-06"
updated: "2026-11-07"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/84"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/syn-io-02-snapshot-head-staged-publish.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/project-asset-persistence-architecture.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/syn-io-02-snapshot-head-staged-publish.md"
    - "docs/designs/syn-io-02-directory-bundle-store-stagedfile-design.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/syn-io-02-store-publish-contract.md"
  references: []
  defers: []
  deferred-by: []
---

# SYN-IO-02 DirectoryBundleStore StagedFile 移行計画 Implementation Plan

## Goal

`DirectoryBundleStore` の独自 `.tmp`+`sync_all`+`fs::rename` を
`ruviform-io-file` の `StagedFile`/`SealedFile`/`publish` へ移管し、
snapshot immutable CreateNew・HEAD atomic Replace・
FileAndDirectorySync・分類済み失敗伝播を既存 port/format 不変で
実現する。

## Scope

単一実装ストリーム（task1: store.rs 移行 + 契約テスト）。分割不要 —
変更面は `store.rs`、依存 manifest 2 箇所、`Cargo.lock`、契約
テスト 1 ファイルに閉じる。

## Upstream Documents

- `docs/specs/syn-io-02-snapshot-head-staged-publish.md`（spec）
- Issue #84（Acceptance 正本）・#82（Consumer 設計正本）

## Design Inputs

- `docs/designs/syn-io-02-directory-bundle-store-stagedfile-design.md`（approved）
- `docs/designs/project-asset-persistence-architecture.md`（HEAD
  atomic replace + dir sync・orphan・ACK-lost の根拠）

## File Map

- `Cargo.toml` — `[workspace.dependencies]` に `ruviform-io-file` 追加。
- `crates/synvelis-project/Cargo.toml` — `ruviform-io-file.workspace = true` 追加。
- `Cargo.lock` — `mise run check`/`cargo generate-lockfile` で再生成。
- `crates/synvelis-project/src/store.rs` — `write_atomic` 撤去、
  staged publish helper + `PublishFailure`→`Error::Store` 写像。
- `crates/synvelis-project/tests/store_publish.rs` — 契約テスト
  （新規。CreateNew 拒否・HEAD ordering・orphan・外部変更・
  残滓・再 Open・golden byte 維持）。

## Implementation Sequence

- [ ] workspace/メンバー manifest に `ruviform-io-file` を追加し
  `cargo generate-lockfile`（mise 経由 build）で lock 更新。
- [ ] `store.rs`: `write_atomic` を `publish_staged` helper に置換
  （snapshot=CreateNew、HEAD=Replace、両者
  FileAndDirectorySync+RequireAtomic）。`PublishFailure` 写像は
  effect/phase/durability/op id/temp path をメッセージ転写、
  Indeterminate は cleanup しない。
- [ ] 契約テスト作成（test-spec の Rules/Examples に対応）。
- [ ] `mise run fmt` → `fmt-check`/`check`/`lint`/`test` 実行。
- [ ] 独立レビュー（reviewer A/B → validator → judge）実施、
  blocker 解消。

## Dependencies and Risks

- ruviform-io-file は pin revision `49de02cc` で公開済み —
  Cargo.lock 解決は既定 branch 追従（他 ruviform dep と同じ
  git source。lock の解決 SHA が全 ruviform crate で一致する
  ことを確認）。
- Windows `FILE_FLAG_BACKUP_SEMANTICS` dirsync は NTFS で機能。
  非対応 FS では `Published` 失敗として正直に報告（policy
  対象、設計済み）。
- 同一 revision 再 save は immutable name 契約へ強化 —
  `DestinationExists` は byte 完全一致または `id_watermark` のみ
  差異の同一論理 snapshot（単調増 refresh）のみ受理（gateway
  journal replay 回復と `draw_raw_id` 予約の永続化を維持）、
  内容差異は拒否。controller 経由の通常 save は revision 進行で
  衝突しない。

## Task Breakdown

- `docs/tasks/syn-io-02-task1-store-stagedfile-migration.md` —
  全実装・テスト・検証を含む単一タスク。

## Verification

- [ ] `mise run fmt-check`（cargo fmt --all -- --check）
- [ ] `mise run check`（cargo check --workspace --all-targets）
- [ ] `mise run lint`（cargo clippy --workspace --all-targets -D warnings）
- [ ] `mise run test`（cargo test --workspace --locked）
- [ ] 独立レビュー完了・blocker なし。

## Open Questions / Gaps

- 実 OS の DirSync 失敗注入は ruviform 側 fault-injection 責務
  （mock は crate 内部・実電源断は未実施）。consumer 側は
  「任意の publish 失敗で HEAD を進めない・失敗分類を伝播」に
  加え、「同一 byte 衝突受理は勝者 byte の完全 publish 再実行
  で耐久性を再確立し、完遂不能なら `Err`・HEAD 不変」（E12/
  E13、存在のみの証明排除）を決定的に pin する。OS 別挙動は
  ruviform #296/#306 側の検証に委譲する。

## Self-Review

- [x] Every spec requirement maps to a task.
- [x] No placeholders remain.
- [x] File paths, relations, status values, and terminology are consistent.

## Implementation Handoff

- Recommended mode: inline execution（単一タスク・単一ファイル主体）
- Independent work streams: none
- User approval required before dispatch: yes → worker-guide により
  review-protocol の独立レビューで代替
- If declined or unavailable: execute inline using the same
  dependency graph and verification matrix.

## Review Handoff

- `PublishFailure` 写像で temp path を `Error::Store` に含める方針
  （診断情報・個人情報なし）。
- HEAD への dirsync 要求の妥当性（FileSync では DirSync 失敗が
  発生し得ず AC の「DirSync 失敗時 HEAD 非前進」が空文になる）。
