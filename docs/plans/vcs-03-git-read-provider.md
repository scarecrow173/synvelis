---
id: "PLAN-D4Oqo3jPnK2GybwKhM5nwh"
type: "plan"
status: "approved"
title: "VCS-03 Git Read Provider implementation plan"
created: "2026-11-10"
updated: "2026-11-10"
owners: []
relations:
  source:
    - "DESIGN-7jUOgXVjk3gpAYkbIzdUfe"
  verified_by:
    - "TSPEC-K2QAlGFhpEHJiLP6StV4ta"
  implemented_by: []
  supersedes: []
  superseded_by: []
---

# VCS-03 Git Read Provider implementation plan

## Scope

`crates/synvelis-project/src/vcs/` 新設 + `lib.rs` export、
`crates/synvelis-library/src/git/vcs.rs` 新設 + 小規模 adapter 変更
（`context.rs`/`objects.rs`/`git/mod.rs`/`lib.rs`）、
`crates/synvelis-library/tests/common/vcs_git_fixture.rs` +
`tests/vcs03_git_provider.rs`、docs chain。範囲外 crate・新規依存なし。

## File map

- `crates/synvelis-project/src/vcs/types.rs` — provider-neutral 型一式。
- `crates/synvelis-project/src/vcs/mod.rs` — `VcsReadProvider`・
  `VcsError`・`VcsCancel`・re-export。
- `crates/synvelis-project/src/lib.rs` — `pub mod vcs;`。
- `crates/synvelis-library/src/git/vcs.rs` — `GitVcsProvider`/
  `GitVcsConfig`（`VcsReadProvider` + `VcsBytesPort` 実装、probe/
  binding/tree/index/status/history/LFS/cancel/bounds）。
- `crates/synvelis-library/src/git/objects.rs` — `classify_kind` に
  `"not a git repository"` → `NotFound`。
- `crates/synvelis-library/src/context.rs` —
  `CancellationToken::from_shared`。
- `crates/synvelis-library/src/git/mod.rs`、`src/lib.rs` — export。
- `crates/synvelis-library/tests/common/vcs_git_fixture.rs` — temp
  worktree fixture（SHA-1/SHA-256 init・commit/branch/tag/merge/
  conflict/gitlink/LFS pointer+object・provider factory）。
- `crates/synvelis-library/tests/vcs03_git_provider.rs` — 受入 4 行の
  fixture test 19 件 + read-only/session oracle。
- docs chain — spec/design/plan/task/test-spec/impl record + index。

## Dependency graph

contract (`synvelis-project::vcs`) → adapter (`synvelis-library::git::vcs`)
→ fixture (`tests/common/vcs_git_fixture`) → suite
(`tests/vcs03_git_provider`) → docs/impl record。

## Risk register

| risk | 対応 |
| --- | --- |
| Git object ID と project `ContentDigest` の混同 | `object_format`/`oid_len`・`sha256:` prefix 検証・両者を比較しない契約を型 doc + test で固定 |
| index への意図しない書込（status refresh 等） | `GIT_OPTIONAL_LOCKS=0` + oracle test（index hash/status/HEAD/staged 前後一致） |
| LFS pointer text の content 誤返却 | pointer-size + parse + `check-attr` 3 条件 + local object size 検証。fail-safe は LFS 側 |
| partial stage/untracked/missing LFS の silent Equal | `Absent`/`Unknown` → VCS-01 `Missing`/`Degraded`/`PartialStage` 写像を test で固定 |
| foreign repo/binding の revision 誤用 | `repository_id`+`context`+`object_format` binding 検査 → `ForeignBinding` |
| cursor replay / mutable ref の pin 解除 | cursor fingerprint bind・`resolve_fixed_revision` 一回解決のみ |
| 実 OS/remote/auth/GC | scope 外・`NotRun`（Issue #97） |

## Verification matrix

- `mise run fmt-check` / `check` / `lint` / `test` / `test-rt` /
  `build` / `test-msrv`（存在する lane）。
- `tests/vcs03_git_provider.rs` 19 test が受入 4 行を headless fixture で
  green にすること。
- docs index 再生成（`doc_maintenance.js plan/apply` per type）+
  `doc_status.js audit`。

## Rollback

新規 module/adapter/test/docs のみで既存挙動への変更は
`objects.rs` の failure 分類 1 pattern と `context.rs` の追加 method
のみ — いずれも additive で revert 容易。
