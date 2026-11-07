---
id: "TASK-SV180RootedCutover2k26"
type: "task"
status: "done"
title: "ALIB-01 task5 — Ruviform rooted capability への containment 移行（N1 修復）"
created: "2026-11-07"
updated: "2026-11-07"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/85"
    - "https://github.com/scarecrow173/ruviform/issues/398"
    - "https://github.com/scarecrow173/ruviform/pull/401"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/alib-01-asset-provider-baseline.md"
  implemented-by: []
  depends-on:
    - "docs/plans/alib-01-asset-provider-baseline.md"
    - "docs/specs/alib-01-asset-provider-contract.md"
    - "docs/designs/alib-01-asset-provider-contract-design.md"
    - "docs/test-specs/alib-01-asset-provider-contract.md"
    - "docs/tasks/alib-01-task2-local-provider.md"
    - "docs/tasks/alib-01-task3-project-provider.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
---

# ALIB-01 task5 — rooted capability への containment 移行

## 内容

N1（Refs #85）の恒久修復。`canonicalize`+containment 照合→path 再
traverse の 2 段構成を、Ruviform `ruviform-io-file::rooted` の
handle-relative capability（ruviform#398 → PR401 merge `f483fe1c`、
消費 rev は後続 main `5d48298b`/RF403）へ全面移行する。

- `synvelis-library` に `ruviform-io-file`（workspace 既定 main 追従
  git dep）を追加。SHA 固定・vendor・再実装・`cap-std` 等の追加 dep
  は行わない。
- `LocalFolderProvider` は `RootedDir` を保持し、locator は
  `RelativePath` 文法で検証。`stat`=`metadata(WithinRoot)`、
  `pin`/`open_preview`/`open_pinned`=`open_regular(WithinRoot)`+
  `RootedFile`/`NodeInfo` の同一 object facts。browse descend は
  実 dir のみ `open_dir(NoFollow)` の scoped handle 経由。locator
  scope は scoped `RootedDir` として開き、scope subtree が
  capability root = 旧 canonical-prefix scope 境界と同値。
- `LocalBlobDir` は呼出毎に store `RootedDir` を開き
  `metadata`/`open_regular(WithinRoot)`。SV 側の name/device 検証は
  維持。
- `FileErrorKind`→`ProviderErrorKind` の型付き対応
  （`EscapesRoot`→`PermissionDenied`、`Unsupported`→
  `UnsupportedCapability`、`IdentityMismatch`→`SourceChanged`、
  `LoopDetected`→`InvalidRequest`、`LimitExceeded`→`BudgetExceeded`、
  `Io` は source の `io::ErrorKind` で精緻化）。
- `tests/containment_swap.rs` の 7 件から `#[ignore]` を除去し
  deterministic fault barrier の地点タグを rooted seam
  （`"stat"`/`"open"`/`"read_dir"`）へ更新。seam sanity の期待列を
  `["stat","open"]`/`["stat","read_dir","stat"]` へ更新。
  scope 取得境界（`open_scope` の `RootedDir::open_with`）の swap
  回帰 `local_scope_acquisition_swap_cannot_escape_root` を追加
  — capability 取得そのものが root 外を掴まないことを
  `PermissionDenied` で固定。
- test-spec N1 feature・spec Known gap・design FS traversal 節・
  impl record を実態へ更新。

## 検証

- `cargo test -p synvelis-library` — 9/9 pass（8 swap regressions
  GREEN 強制 + seam sanity、いずれも `#[ignore]` なし）、
  local 33・project 21・contract・types 全非回帰。
- `mise run fmt`/`fmt-check`/`check`/`lint`/`test` 全 workspace pass、
  `check-ruviform-rev`（`5d48298b`・26 sources）・
  `check-headless-deps` pass。
- `mise run test-msrv` は fail — `cargo-platform@0.3.3` が
  rustc 1.91 を要求する既存 main 由来の不整合（本差分の
  `Cargo.lock` delta に当該 pkg なし）。別途 follow-up。

## 互換差（診断済み・test-spec/impl record に記録）

- `RelativePath` は旧 `parse_locator` より厳格（Windows 予約名・
  空白端・末尾 `.`・>255B component・非 UTF-8 を拒否）— 該当名は
  locator 化不可。browse では partial として skip。
- browse の link 処理: 範囲内 dir link は `Container` と表示するが
  下降はしない（canonical 実 path 側で既に列挙されるため出力は
  同値）。escaping link の facts は target 側ではなく link 自身の
  最小 facts を返す（旧実装は target facts を報告していた）。
- `RootedDir::read_dir` は listing を一括 `Vec` で返す（旧
  `fs::read_dir` の incremental take 相当の逐次制限は RF API に
  存在しない）。emit 件数の `max_entries` bound は維持。
- dangling symlink は Windows/Unix で `Missing` に統一（旧 Unix
  経路は列挙から黙って skip していた）。
- dir entry の `byte_len` は `NotApplicable`（旧実装は dir size を
  報告していた）。
- RF の `WithinRoot` は canonical-root component の lexical prefix
  で absolute link target を判定する — `/var` vs `/private/var`・
  8.3 名・`subst` drive 等の非 canonical alias は kernel 上
  in-root であっても `EscapesRoot` を返し得る（保守的縮小、
  隠さない）。
- scoped directory handle は inode authority を保持し、取得後の
  外部 rename でも読み取り継続する — pathname 継続包含ではなく
  handle authority（RF の設計どおり）。
- `LocalFolderProvider::new` の open 失敗は `Io`→`TransientIo` で
  正直に分類（旧実装は canonicalize 全失敗を `NotFound` に畳んで
  いた）。
- trailing `/` は `RelativePath` が directory marker として受理
  （旧実装は空 component として拒否 — acceptance 拡大で同じ
  object を指す）。
- pin は `ConditionalStrong{ContentHash}` の content pin 契約を
  維持 — object-identity `NodePin` は契約上の pin 証拠ではなく、
  weak identity FS での追加制限もない（必要になれば別契約）。
- browse DFS は未訪問 dir の scoped handle を stack に保持する
  （`max_entries`/`max_open_dirs` で bounded、枯渇時は
  `truncated` で fail-honest）。
