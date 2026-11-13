---
id: "TSPEC-034WTsZKJhBKgzDNLtpIYq"
type: "test-spec"
status: "approved"
title: "VCS-06 real-git/FS fault rows tests"
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
  implements: []
  implemented-by: []
  depends-on:
    - "docs/designs/vcs-06-real-git-fs-fault-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "SPEC-034VNNGsOaycnOBjCAH3m1"
    - "SPEC-034V2wnQPWFJdKz68KlwJG"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/vcs-06-real-git-fs-fault.md"
  derived-by: []
  verifies:
    - "SPEC-034WTsO0gdR1Z2PjqV9OQ2"
  verified-by: []
  references:
    - "crates/synvelis-library/tests/vcs06_git_matrix.rs"
    - "crates/synvelis-project/tests/vcs06_restore_matrix.rs"
    - "crates/synvelis-project/tests/vcs06_apply_matrix.rs"
  defers: []
  deferred-by: []
---

# VCS-06 real-git/FS fault rows tests

## Purpose

VCS-06 fixture・fault matrix の未消化行のうちローカル検証可能な
分を pin する。fixture/headless 成功を実 GUI/OS durability/
provider/plugin-host/RT/consumer 受入に換算しないことを、
test の構造（`#[cfg(windows)]` guard・file:// local transport・
precondition assert）と ledger 記述の両方で担保する。

## Feature: VCS-GIT-REAL — real git repo 上の read path

### Rule GIT-REAL-R1: packed object/ref read path は layout に無関心

`git pack-refs --all` + `repack -ad` + `gc --prune=now` で
loose ref/object が packed layout に移った後も、fixed
revision 解決・file bytes read・history page・staged index
read が不変でなければならない。packed layout の成立
（`packed-refs` 存在・loose `refs/heads/main` 消失）を
precondition として assert する。

#### Example

`gc_pack_refs_and_repack_keep_fixed_revision_reads`:
main + side branch + tag を持つ repo で gc 系 3 command を
実行し、pinned HEAD・`Expr(c1)`/`Expr("v1")`/`Expr("side")`/
`Expr("main")` の解決・`read_revision_file` bytes・
`list_history` の c1 到達・`resolve_staged_snapshot` を確認。

### Rule GIT-REAL-R2: 真に prune された revision は RevisionGone

ref 削除 + reflog expire + `gc --prune=now` で object が真に
除去された revision の resolve は `VcsError::RevisionGone`
を返さなければならない — silent success・empty read・
再解決は禁止。object 不存在は `git cat-file -e` の失敗で
precondition assert する。

#### Example

`pruned_commit_is_revision_gone_not_silent`: doomed commit を
`reset --hard HEAD~1` + `reflog expire` + `gc --prune=now`
で除去し、`cat-file -e` 失敗を確認後
`resolve_fixed_revision(Expr(doomed))` が
`RevisionGone` を返し、HEAD 解決が影響を受けないことを確認。

### Rule GIT-REAL-R3: file:// remote の force-push/fetch で pin は不変

`file://` bare remote への push、第二 clone からの divergent
force-push、fetch 後も、pin 済み revision の OID/bytes は
不変で `origin/main` remote-tracking ref のみが前進し、
local HEAD は動かない。`file://` は認証・network を伴わない
local transport であり、この test は認証付き実 remote lane
の証拠にならない。

#### Example

`file_remote_force_push_fetch_preserves_pinned_revision`:
bare remote を `symbolic-ref HEAD refs/heads/main` で初期化
（clone が unborn HEAD にならないよう platform 既定 branch
不一致を回避）し、第二 clone の force-push 後に元 repo で
`fetch origin`。pinned OID/bytes 不変・`origin/main` が
force-push tip を指す・HEAD 不変を assert。

### Rule GIT-REAL-R4: linked worktree の index scope は分離

linked `git worktree` での stage は、元 worktree に bind した
provider の staged snapshot/status に現れてはならない。
linked root に bind した provider には自身の index
generation で staged change が見えなければならない。

#### Example

`linked_worktree_index_scope_is_isolated`: `git worktree add
--detach` で linked worktree を作り linked 側のみで
`proj/a.txt` を stage。元 provider の status が全 entry
clean・linked provider の status に staged row が現れることを
確認。

## Feature: Windows FS fault — restore staging / apply journal

### Rule FS-FAULT-R1: fault は typed failure であり silent corruption ではない

staging manifest・staged file・journal record への Windows
固有 fault（share-mode 排他/限定 hold・read-only attribute）
は、`RestoreError::Io` / `ApplyError::Journal` の typed
failure として surface し、publish/mutation を完了しては
ならない。

#### Example

`locked_staging_manifest_fails_typed_and_resume_recovers`:
`restore.json` を `share_mode(0)` で hold した状態の resume が
`RestoreError::Io` を返し staging 保持・dest 不在であること。

### Rule FS-FAULT-R2: staging/journal は fault を生き延びる

fault による typed failure の後も staging dir・journal record
は残り、fault 解除後の retry が正常系（`Published` /
元の typed apply failure）に回復しなければならない。

#### Example

`locked_journal_record_fails_typed_before_mutation_and_recovers`:
PPQN drift fixture の初回 `UnsupportedField` failure で
journal record を persist した後、record を `share_mode(0)`
で hold → fresh `DirectoryApplyJournal` の retry が
`ApplyError::Journal`（mutation 前・`ppqn == 960` 不変）→
解放後は元の `UnsupportedField` が返る。

### Rule FS-FAULT-R3: Windows-only は `#[cfg(windows)]` で正直に分離

share-mode/read-only の意味論は Win32 固有であり、POSIX で
同じ fault は再現できない。対象 test は `#[cfg(windows)]`
で guard し、非 Windows では compile・実行されない
（NotRun として ledger に明記）。

#### Example

全 5 test が `#[cfg(windows)] #[test]` — CI の macOS/ubuntu
leg では binary に含まれず、Windows leg でのみ実行される。

### Rule FS-FAULT-R4: fault 対象はファイル単位 — dir read-only は境界にしない

Windows の directory read-only attribute は配下ファイルの
作成/置換を必ずしも阻害しないため、fault 対象は staging/
journal 内の具体ファイル（`restore.json`・staged `HEAD`・
journal `.json` record）とする。この設計理由を test コメント
に残す。

#### Example

`locked_staging_head_blocks_publish_rename_and_recovers`:
staged `HEAD` を `share_mode(0x1)`（`FILE_SHARE_READ` のみ
許可 — read は通るが replace/rename が拒否される）で hold し、
publish の rename 境界で typed failure を確認。

## Feature: VCS-COMPAT — CI 証拠の実測更新

### Rule COMPAT-R1: 実測値のみ記録する

`vcs-06-compat-evidence.md` は `gh` CLI で実際に取得した
run ID・head SHA・triggering actor・leg outcome・日時のみを
記録する。推測・将来 run の結果を含めない。

#### Example

run 43428177865（head `7c206289`・manual `workflow_dispatch`・
actor `scarecrow173`）の全 leg outcome と vcs06 binary 観測を
転記し、旧 run 42938180676 の windows failure は履歴として
保存する。

### Rule COMPAT-R2: fixture/headless・CI lane を実機受入に換算しない

CI leg の pass・local `mise run test` の pass は workflow/
ローカル検証の証拠であり、実 GUI・実 OS durability・実
provider/consumer・RT の受入ではない。本 branch の新規行は
merge 後の次回 dispatch まで CI lane NotRun であることを
明記する。

#### Example

compat evidence と ledger の双方に「本 branch の新規 test 行は
ci.yml run 未収録 = CI lane NotRun（PR head CI は trigger
無しで NotRun by design）」を記録する。

## Guarantees

- 全 9 test が Windows/rustc 1.99.0 で pass し、canonical
  `mise run test` が両 package で exit 0。
- ledger の新規行が Expected/Observed/Status/Recovery を持ち、
  residual lane（認証付き remote・POSIX FS fault・実
  OS/GUI/plugin/RT/consumer）が NotRun/Unsupported のまま。
- `src/`・`.github/`・`mise.toml`・`Cargo.toml`・
  `POLICIES.md`・範囲外 evidence/docs に差分がない。

## Non-goals

- 認証付き http/ssh remote・実ネットワーク・実 provider E2E。
- POSIX FS fault・電源断/物理 fault・実 OS crash durability。
- 実 GUI/IME・plugin host・RT・consumer/precision・他 OS 実機。
- VCS Writer/3-way merge・Issue #97 の Close。

## Risk

- file:// remote の過大解釈 → test コメント・ledger・spec で
  local transport の範囲を 3 箇所明記。
- GC/share-mode の環境差 → precondition assert（`packed-refs`
  存在・`cat-file -e` 失敗）で「状態成立」を測る。
- Windows-only lane が他 OS で silent に抜ける → `#[cfg]` の
  存在自体を test-spec rule として pin。
