---
id: "SPEC-034WTsO0gdR1Z2PjqV9OQ2"
type: "spec"
status: "approved"
title: "VCS-06 real-git/FS fault rows + CI evidence refresh (synvelis#97)"
created: "2026-11-13"
updated: "2026-11-13"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/97"
    - "SPEC-vcs06-fixture-fault-matrix"
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
    - "SPEC-034VNNGsOaycnOBjCAH3m1"
    - "SPEC-034V2wnQPWFJdKz68KlwJG"
    - "SPEC-pkRnrZ2RKJfPW1ymwMSENS"
    - "SPEC-034VpB76W8P3twLKj0HlHB"
  refines: []
  refined-by: []
  derives-from:
    - "https://github.com/scarecrow173/synvelis/issues/97"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "evidence/reports/vcs-06-fixture-matrix-ledger.md"
    - "evidence/reports/vcs-06-compat-evidence.md"
    - "crates/synvelis-library/tests/vcs06_git_matrix.rs"
    - "crates/synvelis-project/tests/vcs06_restore_matrix.rs"
    - "crates/synvelis-project/tests/vcs06_apply_matrix.rs"
  defers: []
  deferred-by: []
---

# VCS-06 real-git/FS fault rows + CI evidence refresh (synvelis#97)

## What Should Be Built

VCS-06 fixture・fault matrix の未消化行のうち、ローカルで検証可能な
行を補完する test-only slice。`src/` 変更を伴わない。

1. **VCS-GIT-REAL 部分縮小**（ネットワーク不要の real-git 行）:
   `crates/synvelis-library/tests/vcs06_git_matrix.rs` に実 git
   CLI repo 上の 4 行を追加する。
   - `git pack-refs --all` + `repack -ad` + `gc --prune=now` 後の
     fixed-revision 解決（packed object/ref の read path）。
   - `file://` bare remote + 第二 clone からの divergent
     force-push + `git fetch` で、pin 済み revision の
     OID/bytes 不変と `origin/main` の前進を確認。
   - linked `git worktree` の index スコープ分離（linked 側の
     stage が元 worktree の provider に見えない）。
   - ref 削除 + reflog expire + `gc --prune=now` で真に object を
     除去した revision が `VcsError::RevisionGone` を返す
     （silent success ではない）。
2. **RESTORE/APPLY Windows FS fault 行**:
   `crates/synvelis-project/tests/vcs06_restore_matrix.rs` と
   `vcs06_apply_matrix.rs` に `#[cfg(windows)]` の fault test を
   追加する。`std::os::windows::fs::OpenOptionsExt::share_mode`
   による排他/共有限定 hold と read-only attribute により、
   staging manifest・staged HEAD・journal record の fault で
   typed failure + recovery を実測する。非 Windows では実行
   されず NotRun と正直に分離する。
3. **COMPAT 証拠リフレッシュ**: `gh run list`/`gh run view` で
   #323 merge 後の covering ci.yml run を取得し、
   `evidence/reports/vcs-06-compat-evidence.md` の COMPAT-03/04
   行を実結果で更新する。
4. **ledger 更新**: 追加した全行を
   `evidence/reports/vcs-06-fixture-matrix-ledger.md` に
   Pass/NotRun で記録する。

あわせて doc chain（spec/design/plan/task/test-spec/impl-ir）を
`vcs-06-real-git-fs-fault` 命名で作成する。

## Why Now

headless fixture lane は PR #268/#306/#323/#338/#340 でほぼ統合
済みだが、ledger の `VCS-GIT-REAL-*` 行は「real remote/auth/
force-push/GC/OS perms = not measured / NotRun」のまま残り、
RESTORE/APPLY の Windows FS fault も未消化だった。また
`vcs-06-compat-evidence.md` は「#323 covering run 未実走・
NotRun pending（2026-11-12）」で止まっており、その後 daily-ci
cron と手動 dispatch で covering run が実走済みである。
ネットワーク不要の real-git 行と Windows FS fault 行はローカルで
検証可能なため、本 slice で補完する。

## Users and Value

- Issue #97 の受入担当者: packed/GC・file:// remote・linked
  worktree・pruned revision・Windows FS fault の Expected/
  Observed を ledger 行として統合できる実測証拠を得る。
- CI 運用担当者: COMPAT-03/04 が最新の covering run の
  required/observed leg 区別つき実測で更新される。

## Scope

### In Scope

- `crates/synvelis-library/tests/vcs06_git_matrix.rs`（+4 test）
- `crates/synvelis-library/tests/common/vcs_git_fixture.rs`
  （共有 fixture が必要な場合のみ）
- `crates/synvelis-project/tests/vcs06_restore_matrix.rs`
  （+3 Windows-only test）
- `crates/synvelis-project/tests/vcs06_apply_matrix.rs`
  （+2 Windows-only test）
- `evidence/reports/vcs-06-compat-evidence.md`（COMPAT 行更新）
- `evidence/reports/vcs-06-fixture-matrix-ledger.md`（新規行追加）
- 本 spec / design / plan / task / test-spec / impl-ir チェーン

### Out of Scope

- `crates/*/src/` の変更（公開 API の外部観測契約のみを oracle
  にする）
- 認証付き実 remote（http/ssh auth）・実ネットワーク — REAL
  remote/auth lane は NotRun 維持
- 実 GUI/IME・plugin host・RT・consumer/precision・他 OS 実機・
  電源断/物理 fault・VCS Writer/3-way merge
- `evidence/reports/vcs-06-*` 以外の evidence、`docs/*/README.md`
  以外の範囲外ファイル
- Issue #97 の受入 checkbox 編集・Close

## Requirements

- real-git 行は `VcsGitFixture` と既存 common git helper のみを
  使い、ネットワーク・認証を必要としない。`git gc` 系の行は
  packed layout が実際に成立したこと（`packed-refs` 存在・loose
  ref 消失・`cat-file -e` による object 不存在）を precondition
  として assert する。
- file:// remote 行は pin 済み revision の OID/bytes 不変・
  `origin/main` の前進・local HEAD 不変を assert し、
  `file://` が認証付き実 remote ではないことを test コメントと
  ledger で明記する。
- Windows FS fault 行は `#[cfg(windows)]` で guard し、typed
  failure（`RestoreError::Io` / `ApplyError::Journal`）・
  staging/journal の保持・destination 不在・fault 解除後の
  recovery を assert する。dir 自体の read-only は Windows で
  信頼できる fault 境界にならないため、staging/journal 内の
  対象ファイルを lock する設計とし、その理由をコメントに残す。
- COMPAT 証拠は `gh` で実取得した run ID・head SHA・leg
  outcome・日時のみを記録し、required/observed 区別を維持する。
- Pass* を Pass にしない、NotRun/Unsupported を Pass にしない、
  fixture/headless 成功を実 GUI/OS durability/provider/
  plugin-host/RT/consumer 受入に換算しない。

## Acceptance Criteria

- [ ] `vcs06_git_matrix.rs` の 4 test が実 git repo 上で pass し、
  packed/GC・file:// remote・linked worktree・RevisionGone を
  pin する。
- [ ] `vcs06_restore_matrix.rs` の 3 test と
  `vcs06_apply_matrix.rs` の 2 test が Windows で pass し、
  typed failure + staging/journal 保持 + recovery を pin する。
  非 Windows では `#[cfg(windows)]` により実行されない。
- [ ] `evidence/reports/vcs-06-compat-evidence.md` が最新の
  covering run（run ID・head SHA・leg outcome）で更新される。
- [ ] `evidence/reports/vcs-06-fixture-matrix-ledger.md` に新規
  行が Pass/NotRun で記録される。
- [ ] `mise run fmt-check` / `check` / `lint` / `test`（
  synvelis-library・synvelis-project）を canonical 検証として
  実行し結果を impl-ir に記録する。
- [ ] doc chain 6 file が `vcs-06-real-git-fs-fault` 命名で作成
  され、index が doc-maintenance で再生成される。

## Deferred Design Concerns

- 認証付き http/ssh remote・実 provider E2E・OS perms matrix —
  実ネットワーク/認証情報が必要で本 slice のローカル検証範囲外。
- POSIX 側の FS fault 行 — share-mode/readonly の意味論が
  Windows と異なるため別 lane（NotRun）。
- 電源断・物理 fault・実 OS crash durability — 外部 gate。
- 実 GUI/IME・plugin host・RT・consumer/precision・他 OS 実機 —
  #114/#176/#219/#294 等の外部 gate 残存。
- VCS Writer/3-way merge — 未実装（Blocked、#231 系）。

## Sources and Relations

- `docs/specs/vcs-06-fixture-fault-matrix.md`（親 matrix spec）。
- `docs/specs/vcs-06-evidence-completion.md`（COMPAT 証拠収集
  slice の先例）。
- `docs/specs/sf-06-cross-gate-oracle-case-ledger.md`（verdict
  語彙 `Pass`/`Pass*`/`Fail`/`NotRun`/`Unsupported`/`Blocked`）。
- `crates/synvelis-library/src/git/vcs.rs`（`resolve_oid` が
  provider `NotFound` を `VcsError::RevisionGone` に写す契約、
  sv#335 post-fix semantics）。
- `crates/synvelis-project/src/restore/mod.rs`（staging
  manifest resume・`RestoreError` typed failure）。
- `crates/synvelis-project/src/apply/`（`DirectoryApplyJournal`
  の persisted record と `ApplyError::Journal`）。
- `.github/workflows/ci.yml` + `tools/ci-summary.mjs`
  （required/observed leg 定義の正本）。
- synvelis#97（統合受入 Issue、本 slice は Open のまま）。
- synvelis#335（unborn HEAD 修正 — PR #341 merge 済み、
  `Ok(None)`/`RevisionGone` 意味論の前提）。
