---
id: "IMPL-034WTApAlyZicn0xwYz0ic"
type: "impl"
status: "completed"
title: "VCS-06 real-git/FS fault rows slice (synvelis#97)"
created: "2026-11-13"
updated: "2026-11-13"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/97"
  changes:
    added:
      - type: "file"
        path: "docs/specs/vcs-06-real-git-fs-fault.md"
      - type: "file"
        path: "docs/designs/vcs-06-real-git-fs-fault-design.md"
      - type: "file"
        path: "docs/plans/vcs-06-real-git-fs-fault-plan.md"
      - type: "file"
        path: "docs/tasks/vcs-06-real-git-fs-fault-task.md"
      - type: "file"
        path: "docs/test-specs/vcs-06-real-git-fs-fault.md"
      - type: "file"
        path: "docs/impl/ir/vcs-06-real-git-fs-fault.md"
    modified:
      - type: "file"
        path: "crates/synvelis-library/tests/vcs06_git_matrix.rs"
      - type: "file"
        path: "crates/synvelis-project/tests/vcs06_restore_matrix.rs"
      - type: "file"
        path: "crates/synvelis-project/tests/vcs06_apply_matrix.rs"
      - type: "file"
        path: "evidence/reports/vcs-06-compat-evidence.md"
      - type: "file"
        path: "evidence/reports/vcs-06-fixture-matrix-ledger.md"
    deleted: []
    renamed: []
    moved: []
    generated:
      - type: "file"
        path: "docs/specs/README.md"
      - type: "file"
        path: "docs/designs/README.md"
      - type: "file"
        path: "docs/plans/README.md"
      - type: "file"
        path: "docs/tasks/README.md"
      - type: "file"
        path: "docs/test-specs/README.md"
      - type: "file"
        path: "docs/impl/ir/README.md"
  implements:
    - "docs/tasks/vcs-06-real-git-fs-fault-task.md"
    - "docs/plans/vcs-06-real-git-fs-fault-plan.md"
    - "docs/designs/vcs-06-real-git-fs-fault-design.md"
    - "docs/specs/vcs-06-real-git-fs-fault.md"
  implemented-by: []
  depends-on:
    - "docs/designs/vcs-06-real-git-fs-fault-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/ir/vcs-06-evidence-completion.md"
    - "docs/specs/vcs-06-fixture-fault-matrix.md"
    - "docs/specs/sf-06-cross-gate-oracle-case-ledger.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/plans/vcs-06-real-git-fs-fault-plan.md"
  derived-by: []
  verifies:
    - "docs/test-specs/vcs-06-real-git-fs-fault.md"
  verified-by:
    - "docs/test-specs/vcs-06-real-git-fs-fault.md"
  references:
    - "evidence/reports/vcs-06-compat-evidence.md"
    - "evidence/reports/vcs-06-fixture-matrix-ledger.md"
    - ".github/workflows/ci.yml"
    - "tools/ci-summary.mjs"
  defers:
    - "認証付き http/ssh remote・実 provider E2E・OS perms matrix — 実ネットワーク/認証情報が必要（NotRun）"
    - "POSIX 側 FS fault 行 — share-mode/readonly 意味論が異なる別 lane（NotRun）"
    - "電源断/物理 fault・実 OS crash durability・実 GUI/plugin host/RT/consumer — 外部 gate（NotRun）"
    - "本 branch の新規行に対する ci.yml covering run — merge 後の次回 dispatch 待ち"
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# VCS-06 real-git/FS fault rows slice (synvelis#97)

## Summary

synvelis#97 VCS-06 の fixture・fault matrix 未消化行のうち
ローカル検証可能な分を補完する test-only slice。`src/` 変更・
新規 dependency なし。3 系統を実施した:

1. **real-git 4 行**（`vcs06_git_matrix.rs`）: packed/GC 後の
   fixed-revision read・pruned revision の `RevisionGone`・
   `file://` remote force-push+fetch での pin 不変・linked
   worktree index scope 分離。
2. **Windows FS fault 5 行**（`vcs06_restore_matrix.rs` +
   `vcs06_apply_matrix.rs`、`#[cfg(windows)]`）: staging
   manifest/HEAD・journal record への share-mode/read-only
   fault で typed failure + recovery を実測。
3. **COMPAT 証拠リフレッシュ + ledger**: covering run
   43428177865（main `7c206289`）全 leg success を記録し、
   ledger に新規行を追加。

doc chain（spec/design/plan/task/test-spec/impl-ir）を
`vcs-06-real-git-fs-fault` 命名で作成した。

## Context

Implements `docs/tasks/vcs-06-real-git-fs-fault-task.md` per
`docs/specs/vcs-06-real-git-fs-fault.md` and
`docs/designs/vcs-06-real-git-fs-fault-design.md`, verified by
`docs/test-specs/vcs-06-real-git-fs-fault.md`（synvelis#97 の
coverage 補完 slice。headless fixture 行列は PR
#268/#306/#323/#338/#340 で記録済みで、本 slice は残る
ネットワーク不要 real-git 行と Windows FS fault 行のみを対象
とする。Issue #97 は `*-REAL-*` 行・実 GUI/実 OS/実
provider/consumer・VCS Writer/3-way merge の外部 gate 残存で
Open のまま）。

## Implementation

### Changes

- `crates/synvelis-library/tests/vcs06_git_matrix.rs`（+4 test、
  3→7）:
  - `gc_pack_refs_and_repack_keep_fixed_revision_reads`
    （VCS-GIT-REAL-01）: `pack-refs --all`/`repack -ad`/
    `gc --prune=now` 後に `packed-refs` 存在・loose
    `refs/heads/main` 消失を precondition assert し、pinned
    HEAD・oid/tag/branch 式解決・file bytes・history・
    staged index が不変であることを確認。
  - `pruned_commit_is_revision_gone_not_silent`
    （VCS-GIT-REAL-02）: `reset --hard HEAD~1` + `reflog expire`
    + `gc --prune=now` で object を真に除去（`cat-file -e`
    失敗を precondition assert）し、
    `resolve_fixed_revision(Expr(doomed))` が
    `VcsError::RevisionGone` を返すことを確認。sv#335
    post-fix semantics に一致。
  - `file_remote_force_push_fetch_preserves_pinned_revision`
    （VCS-GIT-REAL-03）: `file://` bare remote（
    `symbolic-ref HEAD refs/heads/main` で初期化）への push、
    第二 clone の divergent force-push、`git fetch` 後に
    pin 済み OID/bytes 不変・`origin/main` 前進・local HEAD
    不変を確認。local transport のみで認証付き実 remote を
    含まないことをコメント明記。
  - `linked_worktree_index_scope_is_isolated`
    （VCS-GIT-REAL-04）: `git worktree add --detach` で index
    分離された linked worktree の stage が元 provider の
    status に現れず、linked root の provider には自身の
    index generation で見えることを確認。
- `crates/synvelis-project/tests/vcs06_restore_matrix.rs`
  （+3 test、8→11、全て `#[cfg(windows)]`）:
  - `locked_staging_manifest_fails_typed_and_resume_recovers`:
    `restore.json` を `share_mode(0)` で hold → resume が
    `RestoreError::Io`・staging 保持・dest 不在・解放後
    `Published`。
  - `locked_staging_head_blocks_publish_rename_and_recovers`:
    staged `HEAD` を `share_mode(0x1)`（FILE_SHARE_READ のみ）
    で hold → publish rename が typed failure・解放後
    `Published`。
  - `readonly_staging_manifest_is_rejected_before_publish`:
    manifest read-only → `RestoreError::Io`・属性復元後
    `Published`。
- `crates/synvelis-project/tests/vcs06_apply_matrix.rs`
  （+2 test、8→10、全て `#[cfg(windows)]`）: PPQN 960→480
  drift fixture で journal record への fault が mutation 前に
  `ApplyError::Journal` で失敗し `ppqn == 960` 不変、解除後に
  元の `UnsupportedField` が返ることを
  `locked_journal_record_fails_typed_before_mutation_and_
  recovers` / `readonly_journal_record_fails_typed_before_
  mutation_and_recovers` で確認。
- `evidence/reports/vcs-06-compat-evidence.md`: covering run
  43428177865（head `7c206289`・manual workflow_dispatch・
  actor `scarecrow173`・2026-11-13）の全 leg outcome と
  vcs06 binary 観測を追記。run 43422470644 も同日 covering
  success、43420276313 は cancelled で証拠なしと記録。
  COMPAT-03 verdict を最新 run の Pass に更新（旧 run の
  failure は履歴保存）。本 branch の新規行は CI lane NotRun
  と明記。
- `evidence/reports/vcs-06-fixture-matrix-ledger.md`:
  VCS-GIT-REAL-01..04・VCS-APPLY-08・VCS-RESTORE-09 行を追加、
  residual（認証付き remote・POSIX FS fault・実 OS/GUI/
  plugin/RT/consumer）を NotRun 維持、COMPAT-03 行更新、
  fixture test 数を 34→43 に更新。
- docs チェーン 6 file — `vcs-06-real-git-fs-fault` 命名。
  `docs/*/README.md` index は doc-maintenance の
  `index apply` で再生成。

### Decisions During Implementation

- `file://` remote の bare repo は `git init --bare` 直後に
  `symbolic-ref HEAD refs/heads/main` を実行 — platform 既定
  branch（master）との不一致で clone が unborn HEAD・空
  checkout になるのを初回 test 実行で実測して修正した
  （`git_fixture.rs` の既存パターンと同型）。
- pruned revision 行は `reflog expire --expire-unreachable=now
  --all` を gc 前に実行 — reflog が object を保持するため、
  expire なしでは `cat-file -e` が成功してしまい precondition
  が成立しない。`cat-file -e` 失敗を assert して「真に除去
  された」ことを測る。
- Windows FS fault の対象は dir ではなく dir 内ファイル —
  Windows の directory read-only は配下ファイルの作成/置換を
  阻害しないため信頼できる fault 境界にならない。share-mode
  hold（`share_mode(0)`/`share_mode(0x1)`）と file read-only
  で staging manifest・staged HEAD・journal record を直接
  lock する設計とし、理由を test コメントに残した。
- read-only 復元は `set_readonly(false)` ではなく fault 前に
  捕捉した `Permissions` を `set_permissions` で戻す — clippy
  `permissions_set_readonly_false`（Unix で world-writable 化）
  を `#[allow]` せず、元 permission の復元で解消した。
- `#[cfg(windows)]` guard により POSIX compile から除外 —
  share-mode/readonly の意味論差を lane 混同しない。
- COMPAT 記録は `gh api` で triggering actor まで実測し、
  daily-ci cron ではなく manual `workflow_dispatch`
  （actor `scarecrow173`）であることを正直に記録した。

## Related Experiments

### Adopted

<!-- none（既存 fixture/helper と公開 API の外部観測のみ —
探索的変更なし）。 -->

### Rejected

<!-- none -->

## Validation

`origin/main` @ `7c206289`（#376–#379 CI/policy merge 済み）
を base とする `test/97-vcs06-real-git-fs-fault` で実測
（Windows 11 x64・NTFS、pinned rustc 1.99.0、git
2.54.0.windows.1）:

- `mise run fmt-check` — Pass（`cargo fmt --all -- --check`、
  exit 0）
- `mise run check synvelis-library` — Pass（`cargo check
  --locked --all-targets -p synvelis-library`、exit 0）
- `mise run check synvelis-project` — Pass（同、exit 0）
- `mise run lint synvelis-library` — Pass（`cargo clippy
  --locked --all-targets -p synvelis-library -- -D warnings`、
  exit 0）
- `mise run lint synvelis-project` — Pass（同、exit 0。
  `permissions_set_readonly_false`・`similar_names` の 2 lint
  を権限復元・変数改名で解消）
- `mise run test synvelis-library` — Pass（`cargo test
  --locked -p synvelis-library`、exit 0。`vcs06_git_matrix`
  7 test を含む全 suite green）
- `mise run test synvelis-project` — Pass（`cargo test
  --locked -p synvelis-project`、exit 0。`vcs06_restore_matrix`
  11・`vcs06_apply_matrix` 10 test を含む全 suite green）

補助証拠（canonical = mise、直接 cargo は補助）:
`cargo test -p synvelis-library --test vcs06_git_matrix` —
7 passed、`cargo test -p synvelis-project --test
vcs06_restore_matrix --test vcs06_apply_matrix` — 21 passed。

CI 証拠（`gh` 実測、local 検証とは別 lane）:
ci.yml run 43428177865（main `7c206289`）全 leg success、
vcs06 全 6 binary が ubuntu/windows/macos/msrv/rt-verify で
実行・pass。本 branch の新規行は当該 run に未収録（merge 後
の次回 dispatch 待ち）。

NotRun / Unsupported（test-spec/spec 宣言どおり、今回の
fixture・CI 記録は以下を証明しない）:

- `*-REAL-*` residual（認証付き http/ssh remote・実 provider
  E2E・OS perms matrix）— NotRun。
- POSIX FS fault 行 — `#[cfg(windows)]` により非 Windows では
  存在しない（NotRun）。
- 実 GUI/IME・plugin host・RT・consumer/precision・他 OS
  実機・電源断/物理 fault — NotRun（外部 gate）。
- COMPAT-02（consumer precision）— NotRun。
- VCS Writer/3-way merge — Blocked 維持。
- 本 branch の新規行に対する ci.yml covering run — merge 後
  の次回 dispatch まで NotRun。

## Risks

- `file://` remote の成功を認証付き実 remote の Pass に
  しない境界は test コメント・ledger・spec の 3 箇所で維持。
  ledger verdict の最終判定は Coordinator。
- `gc --prune=now`/share-mode の環境差は precondition assert
  （`packed-refs` 存在・`cat-file -e` 失敗・typed error 一致）
  で「状態成立」を測る設計で緩和。
- CI 証拠は取得時点（2026-11-13）の snapshot — 後続 run で
  結果が変わっても本記録は実測時点のものとして保持。

## Follow-ups

- merge 後の次回 ci.yml dispatch で本 branch の新規行を含む
  vcs06 binary の covering run を確認し、COMPAT 証拠を再更新
  する（Coordinator/後続 slice）。
- POSIX FS fault・認証付き remote・実 OS/GUI/plugin/RT/
  consumer lane — 外部 gate として残存。
- synvelis#97 残件（本 slice で Close しない）: 上記外部
  gate の統合受入（Close は Coordinator 判断）。
