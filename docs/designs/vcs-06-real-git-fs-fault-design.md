---
id: "DESIGN-034WTsUdd5SHHSF6EBJmfK"
type: "design"
status: "approved"
title: "VCS-06 real-git/FS fault rows — design"
created: "2026-11-13"
updated: "2026-11-13"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/97"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "SPEC-034WTsO0gdR1Z2PjqV9OQ2"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "DESIGN-TR6hK0f243aEk3CmFQlfp"
    - "SPEC-034VNNGsOaycnOBjCAH3m1"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/vcs-06-real-git-fs-fault.md"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "crates/synvelis-library/tests/vcs06_git_matrix.rs"
    - "crates/synvelis-project/tests/vcs06_restore_matrix.rs"
    - "crates/synvelis-project/tests/vcs06_apply_matrix.rs"
    - "evidence/reports/vcs-06-compat-evidence.md"
    - "evidence/reports/vcs-06-fixture-matrix-ledger.md"
  defers: []
  deferred-by: []
---

# VCS-06 real-git/FS fault rows — design

## Overview

本 slice は 3 系統からなる test/report/docs のみの変更で、
`src/` 契約は一切変更しない。oracle はすべて公開 API の外部観測
（provider の resolve/read/status、restore/apply の typed error
と recovery）である。

1. **real-git 行**: `VcsGitFixture`（temp dir の実 git repo +
   git CLI helper）上に、packed object/ref read path・
   `file://` remote の force-push/fetch・linked worktree の
   index scope・pruned revision の `RevisionGone` を pin する
   4 test を追加する。
2. **Windows FS fault 行**: `#[cfg(windows)]` guard 付きで、
   Win32 share mode（`share_mode(0)` 排他、`share_mode(0x1)`
   = `FILE_SHARE_READ` のみ許可）と read-only attribute による
   fault を restore staging / apply journal に注入し、typed
   failure・staging/journal 保持・destination 不在・解除後
   recovery を pin する 5 test を追加する。
3. **CI 証拠リフレッシュ + ledger**: `gh` 実測で covering run
   を更新し、ledger に新規行を記録する。

## real-git 行の設計

### Fixture 選択

`crates/synvelis-library/tests/common/vcs_git_fixture.rs` の
`VcsGitFixture` をそのまま使う（remote 無しの local worktree
前提を壊さない）。remote 操作は `common::git_fixture` の
`git()`/`plain_path()`/`path_to_file_url()` helper で行い、
fixture 自体への変更は不要だった（`vcs_git_fixture.rs` は
無変更）。

### packed/GC 行（VCS-GIT-REAL-01）

`pack-refs --all` → `repack -ad` → `gc --prune=now` を実行し、
packed layout の成立を precondition として assert する
（`packed-refs` 存在・loose `refs/heads/main` 消失）。その後、
pinned HEAD・oid/tag/branch 式の `RevisionSelector::Expr` 解決・
`read_revision_file` の bytes・`list_history` の packed commit
到達・`resolve_staged_snapshot` の index read が全て不変である
ことを確認する。provider は git plumbing 経由なので storage
layout に無関心であるべき、という契約を実測で pin する。

### pruned revision 行（VCS-GIT-REAL-02）

`reset --hard HEAD~1` + `reflog expire --expire-unreachable=now
--all` + `gc --prune=now` で object を真に除去する。
`git cat-file -e <oid>` の失敗を precondition として assert
した上で、provider の `resolve_fixed_revision(Expr(doomed))`
が `VcsError::RevisionGone` を返すことを確認する。sv#335
（PR #341）post-fix の意味論 — provider `NotFound` →
`RevisionGone`、silent success でも empty read でもない — に
合わせる。HEAD 解決が prune の影響を受けないことも確認する。

### file:// remote 行（VCS-GIT-REAL-03）

bare repo を `git init --bare` で作り、`symbolic-ref HEAD
refs/heads/main` で remote HEAD を `main` に向ける（platform
既定 branch との不一致で clone が unborn HEAD になるのを防ぐ
— 初回実行で実測して修正）。`file://` URL は
`path_to_file_url` 経由。第二 clone で divergent commit を作り
`push --force`、元 repo で `git fetch origin` 後、pin 済み
revision の OID/bytes 不変・`origin/main` が force-push tip を
指す・local HEAD 不変を assert する。`file://` は認証・
network を伴わない local transport であり、認証付き実 remote
lane を Pass に繰り上げないことを test コメントと ledger の
両方に明記する。

### linked worktree 行（VCS-GIT-REAL-04）

`git worktree add --detach <path>` で object store/refs を共有
し index/HEAD を分離した linked worktree を作る。linked 側で
`proj/a.txt` を変更して `git add` し、(a) 元 worktree に bind
した provider の staged snapshot/status に linked の stage が
現れないこと、(b) linked root に bind した別 provider には
自身の index generation で staged change が見えることを確認
する。`GitVcsConfig` を linked root で再構成するだけで provider
側の特別な対応は不要 — per-worktree index 分離は git の
`.git/worktrees/<name>/index` 構造に由来する。

## Windows FS fault 行の設計

### fault 注入の選択

target は staging/journal ディレクトリ**内のファイル**とする。
理由: Windows では directory 自体への read-only attribute は
配下ファイルの作成/置換を必ずしも阻害せず、信頼できる fault
境界にならない。ファイル単位の share mode と read-only
attribute は実際の locking/AV ソフト競合に近い fault であり、
restore/apply の typed failure boundary を直接叩ける。

- `share_mode(0)`: open handle が全 share を拒否 → 同じファイルの
  open/read/replace が `ERROR_SHARING_VIOLATION` 系で失敗。
- `share_mode(0x1)`（`FILE_SHARE_READ`）: read は許可するが
  write/delete/rename を拒否 → publish の `MoveFileEx` replace
  が失敗する境界を作れる。
- `set_readonly(true)`: `MoveFileEx` replace /
  journal record の再 store が失敗する。

### restore 側（VCS-RESTORE-09 相当、3 test）

`interrupt_restore` helper で staging を残した中断状態を作り、
resume（`restore_as_copy` 再実行）が以下を満たすことを確認する:

- `locked_staging_manifest_fails_typed_and_resume_recovers`:
  `restore.json` を `share_mode(0)` で hold → resume が
  `RestoreError::Io` で typed failure、staging 保持・dest 不在、
  handle 解放後 retry で `Published`。
- `locked_staging_head_blocks_publish_rename_and_recovers`:
  staged `HEAD` を `share_mode(0x1)` で hold（read は通る）→
  publish rename が typed failure、staging 保持・dest 不在、
  解放後 `Published`。
- `readonly_staging_manifest_is_rejected_before_publish`:
  `restore.json` を read-only 化 → resume が `RestoreError::Io`、
  staging 保持・dest 不在、attribute 復元後 `Published`。

### apply 側（VCS-APPLY-08 相当、2 test）

PPQN 960→480 の drift fixture（`UnsupportedField` で typed
failure する apply intent）を使い、persisted journal record
（`.json`）への fault が **mutation 前**に `ApplyError::Journal`
で失敗し、live project が不変（`ppqn == 960`）であること、
解除後に元の `UnsupportedField` が返ることを確認する:

- `locked_journal_record_fails_typed_before_mutation_and_recovers`:
  `share_mode(0)` hold → fresh `DirectoryApplyJournal` での
  retry が `ApplyError::Journal`。
- `readonly_journal_record_fails_typed_before_mutation_and_recovers`:
  read-only 化 → 同上。復元は `set_readonly(false)` ではなく
  fault 前に捕捉した `Permissions` を `set_permissions` で戻す
  （clippy `permissions_set_readonly_false` 対応 — Unix で
  world-writable 化する lint を `#[allow]` せず、元の permission
  を復元する形で解消）。

## COMPAT 証拠・ledger 設計

- `gh run list --workflow ci.yml` / `gh run view` / `gh api`
  で covering run を実取得し、run ID・head SHA・triggering
  actor・leg outcome・vcs06 binary の観測を
  `vcs-06-compat-evidence.md` に追記する。取得時点の snapshot
  として扱い、推測で補完しない。
- ledger は自分の追加行のみを編集する（Case ID 行の Expected/
  Observed/Status/Recovery）。`VCS-GIT-REAL-*` の residual
  （認証付き remote・実 provider E2E・OS perms matrix）と
  `VCS-RESTORE-REAL-*` residual（OS crash/power-loss・AV・
  実 plugin host・consumer）は NotRun のまま維持する。
- POSIX 側の FS fault 行は `#[cfg(windows)]` により非 Windows
  では存在しない — ledger では「Windows-only、POSIX は NotRun」
  と明記する。

## 変更ファイル

| ファイル | 種別 | 内容 |
| --- | --- | --- |
| `crates/synvelis-library/tests/vcs06_git_matrix.rs` | modify | real-git 4 test 追加 |
| `crates/synvelis-project/tests/vcs06_restore_matrix.rs` | modify | Windows FS fault 3 test 追加 |
| `crates/synvelis-project/tests/vcs06_apply_matrix.rs` | modify | Windows FS fault 2 test 追加 |
| `evidence/reports/vcs-06-compat-evidence.md` | modify | covering run 追記・verdict 更新 |
| `evidence/reports/vcs-06-fixture-matrix-ledger.md` | modify | 新規行追加・test 数更新 |
| `docs/{specs,designs,plans,tasks,test-specs,impl/ir}/vcs-06-real-git-fs-fault*.md` | new | doc chain |

## Risks and Trade-offs

- **file:// remote の過大解釈**: local transport の成功を認証
  付き remote の Pass にしないよう、test コメント・ledger・
  spec の 3 箇所で範囲を明記する。
- **GC の非決定性**: `gc --prune=now` が object を残す環境差を
  避けるため、pruned 行は `cat-file -e` の失敗を precondition
  assert し、packed 行は `packed-refs` 存在を assert する —
  「実行した」ではなく「状態が成立した」を測る。
- **Windows share mode の意味論差**: `share_mode(0x1)` が
  read を許し replace を拒否する挙動は Win32 固有。POSIX では
  `#[cfg(windows)]` で compile 自体を除外し、意味論の異なる
  lane を混ぜない。
- **clippy `permissions_set_readonly_false`**: `set_readonly
  (false)` は Unix で world-writable を招く lint 対象。
  `#[allow]` で隠さず、捕捉済み `Permissions` の復元で解消。
- **CI run の再現性**: `gh` 実測値のみ記録し、後続 run で
  結果が変わっても本記録は取得時点の snapshot とする。
