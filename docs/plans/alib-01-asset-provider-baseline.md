---
id: "PLAN-luVzqvmmFtXrtfQQ7Sqb6k"
type: "plan"
status: "completed"
title: "ALIB-01 実装計画 — Asset Provider 契約・Local/Project 基準 Provider"
created: "2026-11-07"
updated: "2026-11-07"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/85"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/alib-01-asset-provider-contract.md"
  implemented-by: []
  depends-on:
    - "docs/designs/alib-01-asset-provider-contract-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/adr/alib-01-synvelis-library-crate-boundary.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/alib-01-asset-provider-contract.md"
---

# ALIB-01 Plan

## Task decomposition

1. **alib-01-task1** — crate skeleton + 契約型: workspace members
   追加、`id`/`refs`/`capability`/`observation`/`browse`(cursor
   符号化)/`context`/`error`/`provider`(trait,PinnedRead,FetchCompletion)/
   `registry`/`digest`(sha256 verify reader)。unit tests。
2. **alib-01-task2** — `LocalFolderProvider`: root canonical 固定、
   locator resolve/traversal 拒否、bounded DFS browse + frontier
   cursor、stat、pin(sha256)、open_pinned(EOF 照合)、name search、
   preview/range、cancel/deadline/budget。実 FS テスト。
3. **alib-01-task3** — `ProjectAssetsProvider`: `ProjectCatalog` port
   + `Mutex<ProjectController>`/`ProjectCatalogSnapshot` 実装、
   `ProjectBlobStore`/`LocalBlobDir`、browse(stat/pin/open/preview)、
   typed Unsupported search。実 controller + snapshot 双方でテスト。
4. **alib-01-task4** — 受入・回帰検証: registry 結合、AC1〜AC4
   の実測、`mise run` 正規検証、impl record。
5. **alib-01-task5-rooted-cutover** — N1(Refs #85)修復:
   `LocalFolderProvider`/`LocalBlobDir` を Ruviform
   `ruviform-io-file::rooted` capability（消費 rev `5d48298b`、
   既定 main 追従）へ移行し component-swap TOCTOU を解消。
   7 件の swap 回帰を `#[ignore]` 解除・GREEN 強制（scope 取得境界
   および root 置換 rebind 等の追加回帰を含め計 10 件）。

## File map

- add: `crates/synvelis-library/{Cargo.toml,src/*.rs,tests/*.rs}`、
  `docs/{discovery,specs,adr,designs,plans,tasks,test-specs,impl/ir}/alib-01-*`
- modify: `Cargo.toml`(members)、`Cargo.lock`、各 docs `README.md`
  （generated index 再生成）

## Risk register

- **共有ファイル競合**: ルート `Cargo.toml`/`Cargo.lock`・docs index
  は他 worker と共有。merge 時に共存させ、lock は
  `mise run check` で再解決、index は `doc_maintenance` で再生成。
- **Windows path 差異**: canonical は `\\?\` prefix。locator 照合は
  canonical 同士で行い、表示名のみ OsString を利用。
- **symlink loop**: visited canonical dir set で切断。root 外 target
  は列挙のみ・open 拒否で一貫。
- **pin/open race**: byte 同一性（sha256）を固定根拠とし、
  expected_size を open 時 precondition、fstat で inode に依存
  しない。rename+同 bytes は同 content として受理（asset の意味論
  は bytes）。bytes 差異は IntegrityMismatch。
- **cursor サイズ**: frontier が肥大し得る。depth 上限内で
  sibling 数依存。budget 超過時は next_cursor=None の Partial で
  明示終端（偽の全件を返さない）。

## Rollback

新規 crate は削除で完全 rollback。既存 crate への変更なし
（`synvelis-project` 変更不要）。`Cargo.lock` は
`git checkout -- Cargo.lock` + `mise run check` で復帰。

## Verification matrix

AC1〜AC4 ↔ `crates/synvelis-library/tests/{contract,local,project}.rs`
（test-spec 参照）。正規検証: `mise run fmt-check && mise run check
&& mise run lint && mise run test`。`CARGO_BUILD_JOBS=2` で重い
build の並列を制限。
