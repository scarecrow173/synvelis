---
id: "DESIGN-UpOaSmwX-5aEOp4rEEmXG"
type: "design"
status: "approved"
title: "synvelis#335 unborn HEAD 誤分類修正 — rev-parse --quiet 除去と NotFound 経路回復設計"
created: "2026-11-12"
updated: "2026-11-12"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/335"
  changes:
    added:
      - type: "file"
        path: "docs/designs/vcs-03-unborn-head-classification-design.md"
      - type: "file"
        path: "docs/plans/vcs-03-unborn-head-classification-plan.md"
      - type: "file"
        path: "docs/tasks/vcs-03-unborn-head-classification-task.md"
      - type: "file"
        path: "docs/test-specs/vcs-03-unborn-head-classification.md"
      - type: "file"
        path: "docs/impl/ir/vcs-03-unborn-head-classification.md"
    modified:
      - type: "file"
        path: "crates/synvelis-library/src/git/vcs.rs"
      - type: "file"
        path: "crates/synvelis-library/tests/vcs03_git_provider.rs"
      - type: "file"
        path: "docs/designs/vcs-03-git-read-provider-design.md"
      - type: "file"
        path: "docs/test-specs/vcs-03-git-provider-fixtures.md"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/vcs-03-unborn-head-classification.md"
  implemented-by:
    - "docs/plans/vcs-03-unborn-head-classification-plan.md"
  depends-on:
    - "docs/specs/vcs-03-unborn-head-classification.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/vcs-03-git-read-provider.md"
    - "docs/designs/vcs-03-git-read-provider-design.md"
    - "docs/designs/alib-04-git-remote-provider-design.md"
    - "docs/test-specs/vcs-03-git-provider-fixtures.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/vcs-03-unborn-head-classification.md"
  derived-by:
    - "docs/plans/vcs-03-unborn-head-classification-plan.md"
    - "docs/tasks/vcs-03-unborn-head-classification-task.md"
    - "docs/test-specs/vcs-03-unborn-head-classification.md"
    - "docs/impl/ir/vcs-03-unborn-head-classification.md"
  verifies: []
  verified-by: []
  references:
    - "crates/synvelis-library/src/git/vcs.rs"
    - "crates/synvelis-library/src/git/objects.rs"
    - "crates/synvelis-library/src/git/mirror.rs"
    - "crates/synvelis-library/tests/vcs03_git_provider.rs"
    - "crates/synvelis-library/tests/git.rs"
  defers: []
  deferred-by: []
---

# synvelis#335 unborn HEAD 誤分類修正 — rev-parse --quiet 除去と NotFound 経路回復設計

## Context

`GitVcsProvider` の revision 解決は `git rev-parse --verify --quiet
<expr>^{commit}` を使っていた。unborn HEAD（commit 0 件 repo）では
このコマンドが **exit 1 + stdout/stderr 空** で失敗する。`GitRunner::
run` は非 0 exit を `classify_failure(stderr, code)` に渡すが、
stderr が空では `classify_kind` の全パターンに合致せず、exit code
フォールバック（`Some(128) → TransientIo`、その他 → `TransientIo`）
に落ちる。結果 `ProviderErrorKind::TransientIo` →
`map_provider_error` → `VcsError::Unavailable { detail:
"TransientIo: git command failed" }`。

観測面: `working_status`（`head_oid` 経由）・
`resolve_fixed_revision(RevisionSelector::Head)`（`selector_oid` →
`head_oid`）・`resolve_fixed_revision(RevisionSelector::Expr)`・
`list_history`（start 省略時 `head_oid`、明示 selector 時
`resolve_oid`）が全て `Unavailable` になる。`detect_workspace` /
`GitVcsProvider::open` の probe head は `.ok()` で全失敗を `None`
に畳むため元々 `head_commit: None` を正しく返していた。

破られた不変条件: VCS-03 design（`vcs-03-git-read-provider-design.md`
resolve 節）は `RevisionSelector::Head` の unborn を `RevisionGone`、
`WorkingStatus.head_commit`/`WorkspaceInfo.head_commit` の `None` を
正規表現と定める。`--quiet` は「失敗を静かに exit 1 で返す」ため、
stderr パターン駆動の `classify_kind` が意味を確定できない入力を
生んでいた — **呼出側で意味が確定する `--quiet` 経路だけが誤分類の
源**である。

## Scope

### In Scope

- `vcs.rs` の `resolve_oid`・`head_oid`・`probe_workspace` にある
  `rev-parse --verify --quiet` の `--quiet` 除去（3 箇所、同一
  `HEAD^{commit}`/`<expr>^{commit}` idiom の統一）。
- `tests/vcs03_git_provider.rs` への 0-commit repo 回帰 test
  追加（`VcsGitFixture::new()` がそのまま unborn repo）。
- `docs/designs/vcs-03-git-read-provider-design.md`・
  `docs/test-specs/vcs-03-git-provider-fixtures.md` の整合更新。

### Out of Scope

- `classify_kind`/`classify_failure`/`GitRunner`（`objects.rs`）—
  共有 classifier を変えると mirror provider（ALIB-04）の
  stderr 由来 `NotFound` 分類契約に波及する。今回は call-site
  修正で十分。
- `mirror.rs` — 既に `--verify`（`--quiet` なし）で stderr 駆動
  分類に乗っており正しい。
- unborn repo の `rev-list`/`cat-file`/`ls-tree`/`ls-files`/
  `check-attr`/`status` 経路 — 解決済み oid または index/worktree
  直接読みであり rev-parse に依存しない。unborn で正常動作する
  ことは回帰 test で確認。

## Components and Boundaries

### `--quiet` 除去の根拠（実測）

scratch repo（`git init`、commit 0 件）での実測:

| argv | unborn HEAD | `deadbeef^{commit}` | `HEAD~99^{commit}`（commit 後） |
| --- | --- | --- | --- |
| `rev-parse --verify --quiet X` | exit 1, stdout/stderr 空 | exit 1, 空 | exit 1, 空 |
| `rev-parse --verify X` | exit 128, stderr `fatal: Needed a single revision` | 同左 | 同左 |

`fatal: Needed a single revision` は `classify_kind` の `NotFound`
パターン（`objects.rs`）に合致する。`--quiet` を外すだけで、
既存の authoritative な分類器が正しい kind を返す — 分類規則の
複製も新設も不要。

### 修正境界の選択

Issue の優先順に従い (a) `--quiet` 除去を採用。(b) call-site で
exit-1+空 stderr を検出する方式は、`GitRunner::run` が失敗時に
`ProviderError`（kind+sanitized detail）のみを返し exit code・
生 stderr を露出しないため、call-site での判別には runner API の
拡張が必要になる。意味が確定するのは git の診断出力であり、
`--quiet` を外してその診断を既存分類器に渡す方が、責務境界
（意味確定 = git+classifier、意味写像 = vcs.rs）として一貫する。

### `classify_kind` を変えない判断

`tests/git.rs`（ALIB-04 mirror provider）は `rev-parse --verify`
（`--quiet` なし）の stderr 由来 `NotFound` 分類を assertion で
固定している（deleted branch/path → `NotFound`）。共有
classifier で silent exit-1 を `NotFound` に倒すと、mirror 経路を
含む全 git 呼出の失敗意味が変わる blast radius を持つ。vcs.rs
側の `--quiet` 経路は「意味が確定するのに静黙化していた」局所
欠陥であり、共有層ではなく呼出層で直すのが正しい境界。

### 棚卸し結果（`--quiet` 付き rev-parse）

- `vcs.rs` `resolve_oid`（`RevisionSelector::Expr` 経路）— 修正。
- `vcs.rs` `head_oid`（`working_status`/`list_history`/
  `selector_oid(Head)` 経路）— 修正。
- `vcs.rs` `probe_workspace` head probe — `.ok()` で全失敗を
  `None` に畳む informational 経路。`--quiet` は挙動に無関係
  だが、同一 idiom 統一のため除去（semantic 不変）。
- `mirror.rs` `resolve_commit`/`stat_entry` — `--quiet` なし、
  既に正しい。変更なし。
- 他に `--quiet` 付き git 呼出なし（crate 全体を grep 済み）。

## Data and Control Flow

修正後の失敗分類:

1. `rev-parse --verify <expr>^{commit}` が unborn/未知 ref/非
   commit peel で exit 128 + `fatal: Needed a single revision`
   （stderr）。
2. `GitRunner::run` → `classify_failure` → `classify_kind` が
   `NotFound` を返す。
3. `resolve_oid` は `NotFound` → `VcsError::RevisionGone`。
   `head_oid` は `NotFound` → `Ok(None)`。
4. `selector_oid(Head)` は `Ok(None)` → `RevisionGone { "HEAD is
   unborn" }`。`list_history` start 省略も同じ `RevisionGone`。
5. `working_status` は `head_commit: None` で `Ok` を返す。

真の backend 障害（spawn 失敗・timeout・kill・壊れた object store
等）は従来どおり `TransientIo`/`Cancelled`/その他 kind で
`Unavailable` 等へ写像される — 分類の緩和はない。

## Non-Functional Constraints

- `VcsError`/`VcsReadProvider`/`VcsBytesPort` の API 不変。
- `classify_kind`・`GitRunner`・`mirror.rs`・`synvelis-project`
  他 crate 不変。
- read-only 保証不変（rev-parse は plumbing read のみ）。
- 新規依存なし。`--quiet` 除去は git の標準的失敗診断を有効化
  するだけで、追加の process・I/O・待機を生まない。

## Risks and Trade-offs

- `rev-parse --verify`（`--quiet` なし）は失敗時に stderr へ
  fatal 行を書く。内容は `fatal: Needed a single revision` 等の
  定型で、`sanitize_detail` が 160 文字・URL userinfo 除去済み
  の detail に落とすため credential 漏洩経路は増えない。
- `<expr>` が git にとって別種の fatal を出す場合（例:
  `expected commit type` 系）でも exit 128 + stderr 非空であり、
  `classify_kind` のフォールバックは従来どおり `TransientIo` —
  `--quiet` 時代と同じ分類で回帰しない（悪化もしない）。
- `probe_workspace` の head probe は全失敗 → `None` のまま。
  `--quiet` 除去で stderr が出るが `.ok()` が捨てるため観測差
  なし。

## Deferred Design Concerns

- `head_oid`/`resolve_oid` が `NotFound` 以外の「revision 不在」
  を返し得るかの網羅的監査は classify_kind のパターン依存であり、
  本 slice では実測した unborn/未知 ref/範囲外 peel の 3 系で
  `Needed a single revision` に収束することを確認済み。将来 git
  の診断文が変わった場合の検出は CI の回帰 test が担う。
- `detect_workspace`/`open` が unborn repo を `NotRepository`
  ではなく正常検出することは既存 test
  （`detect_workspace_reports_identity_and_format` 等）と今回の
  unborn test で固定。

## Verification Notes

- [x] scratch repo で `--quiet` 有無の exit code/stdout/stderr を
  実測（unborn・未知 oid・範囲外 peel の 3 系）。
- [x] 回帰 test を修正前コードで実行 → `working_status` が
  `Unavailable { detail: "TransientIo: git command failed" }`
  で決定的に失敗（Issue #335 の観測 signature と一致）。
- [x] 修正後 `cargo test -p synvelis-library --test
  vcs03_git_provider`（補助証拠、canonical = `mise run test`）
  で 20/20 pass。
