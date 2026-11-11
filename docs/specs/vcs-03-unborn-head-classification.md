---
id: "SPEC-rlAMvOKGLT0EwfekD1VvH"
type: "spec"
status: "proposed"
title: "synvelis#335 GitVcsProvider unborn HEAD 誤分類 — rev-parse --quiet 除去による NotFound 経路回復"
created: "2026-11-12"
updated: "2026-11-12"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/335"
    - "SPEC-PvLluOn4j6y1qpdtQ0aw8A"
    - "DESIGN-7jUOgXVjk3gpAYkbIzdUfe"
  changes:
    added:
      - type: "file"
        path: "docs/specs/vcs-03-unborn-head-classification.md"
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
  implements: []
  implemented-by:
    - "docs/designs/vcs-03-unborn-head-classification-design.md"
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/vcs-03-git-read-provider.md"
    - "docs/designs/vcs-03-git-read-provider-design.md"
    - "docs/test-specs/vcs-03-git-provider-fixtures.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by:
    - "docs/designs/vcs-03-unborn-head-classification-design.md"
    - "docs/plans/vcs-03-unborn-head-classification-plan.md"
    - "docs/tasks/vcs-03-unborn-head-classification-task.md"
    - "docs/test-specs/vcs-03-unborn-head-classification.md"
    - "docs/impl/ir/vcs-03-unborn-head-classification.md"
  verifies: []
  verified-by: []
  references:
    - "crates/synvelis-library/src/git/vcs.rs"
    - "crates/synvelis-library/src/git/objects.rs"
    - "crates/synvelis-library/tests/vcs03_git_provider.rs"
    - "crates/synvelis-library/tests/common/vcs_git_fixture.rs"
  defers: []
  deferred-by: []
---

# synvelis#335 GitVcsProvider unborn HEAD 誤分類 — rev-parse --quiet 除去による NotFound 経路回復

## What Should Be Built

`crates/synvelis-library/src/git/vcs.rs` の `GitVcsProvider` が使う
`git rev-parse --verify` 呼出から `--quiet` を除去し、解決不能な
revision（unborn HEAD・未知 ref・commit へ peel できない式）を
fatal stderr（`Needed a single revision` 等）経由で既存の
`classify_kind` → `NotFound` 分類へ乗せる。これにより設計不変条件
（`docs/designs/vcs-03-git-read-provider-design.md`: unborn HEAD は
`RevisionSelector::Head` → `RevisionGone`、`head_commit:
Option<String>` の `None` が正規表現）を回復する。

`--quiet` は失敗時に stderr を空のまま exit 1 で返すため、
`classify_kind` の stderr パターンに一切合致せず exit code フォール
バックで `TransientIo` に落ち、`VcsError::Unavailable`（detail:
`TransientIo: git command failed`）になっていた。0-commit repo で
`working_status`/`resolve_fixed_revision`/`list_history` が全て
Unavailable で失敗し、`syn.vcs.visual refresh` が service_error に
なるのが Issue #335 の観測症状。

## Why Now

Issue #95 の production `set_vcs_backend` 配線（PR #334）で実経路
から初めて観測された。`git init` 直後の 0-commit repo は VCS
導入フローの正当な初期状態であり、そこで History 表示が Failed に
なるのは回帰として扱うべき欠陥。発見元 worker は「既知制限」として
記録済みで、本 Issue がその解消タスク。

## Users and Value

- Synvelis 利用者: commit 0 件の repo を開いた直後から VCS History
  表示が正しく振る舞う（`RevisionGone`/`head_commit: None` の
  typed 表現）。
- `synvelis-library` の VCS-03 read provider: `--quiet` 付き
  rev-parse という「silent failure を NotFound に載せられない」
  罠が除去され、ALIB-04 mirror provider と同じ stderr 駆動の
  分類 idiom に揃う。

## Scope

### In Scope

- `vcs.rs` の `resolve_oid`・`head_oid`・`probe_workspace` の
  `rev-parse --verify` 呼出から `--quiet` を除去（同一 idiom へ
  統一）。
- 0-commit repo fixture（`VcsGitFixture::new()` は `git init` +
  config のみで commit 0 件）を使った回帰 test を
  `tests/vcs03_git_provider.rs` に追加。
- `docs/designs/vcs-03-git-read-provider-design.md` と
  `docs/test-specs/vcs-03-git-provider-fixtures.md` の unborn
  意味論の現状整合。

### Out of Scope

- 共有 `classify_kind`（`src/git/objects.rs`）の変更 — silent
  exit-1 を `NotFound` に変えると ALIB-04 mirror 経路
  （`tests/git.rs`）の stderr 由来分類契約に blast radius が及ぶ。
  今回は call-site の `--quiet` 除去で契約を満たす。
- `VcsError`/`VcsReadProvider` 等 `synvelis-project` 側 API、
  `synvelis-app`/`synvelis-player`/`synvelis-gateway`。
- `tests/common/git_fixture.rs`・`vcs_git_fixture.rs` の意味変更
  （共有 fixture は additive のみ。今回は変更不要）。
- Issue close・merge・`Cargo.lock`・docs index（`docs/*/README.md`）。

## Requirements

- unborn HEAD の `working_status` が `Ok` で `head_commit: None`
  を返すこと（`Unavailable` にしない）。
- `RevisionSelector::Head` の `resolve_fixed_revision` が unborn で
  `VcsError::RevisionGone` を返すこと。
- `RevisionSelector::Expr` の解決不能（unborn branch 名・HEAD
  alias・未知 ref）が `VcsError::RevisionGone` を返すこと。
- `list_history` が unborn で `VcsError::RevisionGone` を返すこと
  （start 省略時・明示 `Head` start 時の両方）。
- unborn repo でも index/staged 系（`resolve_staged_snapshot`・
  `index_fingerprint`）が正常に動くこと。
- commit 済み repo の全既存挙動（resolve/history/status/byte
  reads）を回帰しないこと。
- `classify_kind`・`GitRunner`・mirror provider を変更しないこと。

## Acceptance Criteria

- [ ] `tests/vcs03_git_provider.rs` に 0-commit repo の回帰 test
  が追加され、修正前は `Unavailable { detail: "TransientIo: git
  command failed" }` で失敗・修正後に pass することが実走で
  証拠化される。
- [ ] `mise run fmt-check`/`check`/`lint`/`test`/`test-rt`/
  `check-app-native`/`lint-app-native`/`test-msrv`/`build` が pass
  する。
- [ ] `git.rs` の ALIB-04 系 assertion（stderr 由来 `NotFound`
  分類）が全件 pass のままであること（共有 classifier 無変更の
  ため blast radius なし、`mise run test` で実測）。
- [ ] 外部 gate（実 remote/provider・実 OS 耐久・GUI・RT 性能）は
  Pass 扱いせず NotRun として記録する。

## Deferred Design Concerns

- `probe_workspace` の head probe は全失敗を `None` に畳む
  informational 経路であり、unborn 以外の失敗（例: 一時的な IO
  障害）も `head_commit: None` になる既存挙動は本 slice では
  変更しない（設計上 HEAD は probe 時点の情報提供のみ）。
- unborn repo で `resolve_staged_snapshot` が `complete=true`・
  entries 空を返す semantics の UI 側の扱いは VCS-04 以降の
  範囲。
