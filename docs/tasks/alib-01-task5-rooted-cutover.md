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
  scope は保持 `RootedDir` 上の `metadata(WithinRoot)` で先に
  権威解決し（locator 自体の link containment もここで強制）、
  scope subtree を capability root とする scoped `RootedDir` を
  `open_with` で取得して `dir_identity`/`NodeId` の same-object
  照合を行う — `open_dir` は `WithinRoot` 境界が provider root
  のままで scope 内 link 判定を弱めるため使わず、root rename +
  同一 pathname への別 tree 設置による再 bind は `SourceChanged`
  で拒否する（旧 canonical-prefix scope 境界と同値を維持）。
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
  `["stat","open"]`/`["stat","open","read_dir","stat"]` へ更新
  （`open_scope` は held-resolution `metadata`=`"stat"` と
  取得 `open_with`=`"open"` の 2 地点）。
  scope 取得境界の swap 回帰を 3 件追加 — 保持 root 解決直前の
  `local_scope_acquisition_swap_cannot_escape_root`
  （`EscapesRoot`→`PermissionDenied`）、権威解決/取得間の
  `local_scope_open_swap_cannot_escape_root`（canonical 外
  →`PermissionDenied`）、root rename 退避+同一 pathname 別 tree
  設置の `local_scope_root_replacement_cannot_rebind_capability`
  （`dir_identity` 乖離→`SourceChanged`）。
- test-spec N1 feature・spec Known gap・design FS traversal 節・
  impl record を実態へ更新。

## 検証

- `cargo test -p synvelis-library` — 11/11 pass（10 swap regressions
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
- `RootedDir::read_dir` は listing を一括 `Vec` で返す。旧
  `fs::read_dir` の `take(remaining+1)` に相当する bound は
  `truncate(remaining+1)` → sort で復元し emit 件数・subset
  意味論（enumeration order prefix）は同値を維持するが、
  **列挙そのものの memory/work bound は RF API 側に存在しない**
  — `VfsDir::list`/`read_dir` は全件 `Vec` を materialize して
  から返すため巨大 dir で `max_entries`/cancel/deadline は列挙
  段階では効かない。raw-path fallback での回避は capability
  逸脱のため禁止として、streaming/bounded 列挙 API
  （例: callback/iterator または limit 付き `read_dir`）を RF
  側へ ask — この残余は **BLOCKED（RF API 待ち）** として明示
  （truncate による kept-window 復元は emit/subset bound のみで
  P1-2 を FIXED としない）。coordinator 回答（2026-11-07）:
  bounded/streaming 列挙 API は RF 責務として別 Issue で
  起票・実装する方針、`open_dir` が親 root 境界を共有するため
  scope-subtree re-root の必要性も同じ RF API 案へ含める。
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
  handle authority（RF の設計どおり）。対称的に、locator scope 付き
  browse/search は保持 capability の `NodeId` 照合を経るため、
  root rename + 同一 pathname への別 tree 設置（root 置換）では
  `SourceChanged` で拒否される — `BrowseScope::Root` が rename
  退避側を handle authority で読み続けるのとは意図的な非対称。
  scope 取得と競合する **in-root の** scope dir rename/recreate
  も同じく `SourceChanged` を返す（旧実装は新 object をそのまま
  serve していた — 意図的な厳格化）。
- weak-identity filesystem（birth/creation time や discriminating
  handle を持たず `IdentityStrength::Weak` となる backend — NTFS/
  statx 系以外）では locator scope 付き browse/search が
  `UnsupportedCapability` を返す — `NodeId` same-tick recreate を
  判別できず rebind 検出を黙って弱めるより RF `open_pinned` と
  同じ fail-closed を選ぶ（Root scope や blob/stat/open 系には
  影響しない）。
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
