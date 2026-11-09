---
id: "IMPL-ALIB04GitRemoteProvider"
type: "impl"
status: "completed"
title: "ALIB-04 Git Remote Provider 実装記録"
created: "2026-11-09"
updated: "2026-11-09"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/88"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-library/src/git/mod.rs"
      - type: "file"
        path: "crates/synvelis-library/src/git/url.rs"
      - type: "file"
        path: "crates/synvelis-library/src/git/locator.rs"
      - type: "file"
        path: "crates/synvelis-library/src/git/objects.rs"
      - type: "file"
        path: "crates/synvelis-library/src/git/mirror.rs"
      - type: "file"
        path: "crates/synvelis-library/src/git/lfs.rs"
      - type: "file"
        path: "crates/synvelis-library/tests/common/git_fixture.rs"
      - type: "file"
        path: "crates/synvelis-library/tests/git.rs"
      - type: "file"
        path: "docs/specs/alib-04-git-remote-provider.md"
      - type: "file"
        path: "docs/designs/alib-04-git-remote-provider-design.md"
      - type: "file"
        path: "docs/plans/alib-04-git-remote-provider.md"
      - type: "file"
        path: "docs/tasks/alib-04-git-remote-provider.md"
      - type: "file"
        path: "docs/test-specs/alib-04-git-remote-provider.md"
      - type: "file"
        path: "docs/impl/ir/alib-04-git-remote-provider.md"
    modified:
      - type: "file"
        path: "crates/synvelis-library/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-library/tests/common/mod.rs"
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
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/alib-04-git-remote-provider.md"
    - "docs/designs/alib-04-git-remote-provider-design.md"
    - "docs/specs/alib-04-git-remote-provider.md"
    - "docs/tasks/alib-04-git-remote-provider.md"
    - "docs/test-specs/alib-04-git-remote-provider.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# ALIB-04 Git Remote Provider 実装記録

## Summary

Issue #88「ALIB-04 — Git Remote Provider」の実装。`synvelis-library` に
read-only の Git backed `AssetProvider` を追加した。immutable Commit/Tree
object を対象とし、mutable ref は各呼出で具象 commit sha に解決する。
Pin は `PinProof::ImmutableObject`(`commit:<sha>`)で、blob identity は
`git-blob:<oid>`、Git LFS は `git-lfs:<sha256-oid>`。selective fetch は
`git clone --mirror --no-checkout --filter=blob:none` の derived bare
mirror 上で `cat-file` の promisor lazy fetch に委譲する。LFS pointer
text は決して content として serve せず、実 object は SHA-256 と宣言
size で検証してから fetchable とする。Project VCS state・working
tree・index・HEAD は一切変更しない。

## Context

- Issue: scarecrow173/synvelis#88(親・設計オーナー: #9。関連: #85 ALIB-01
  契約・#86 ALIB-02 import・#87 ALIB-03 cache・#89 ALIB-05 GUI・#91 fault
  matrix・#102 workspace・#126 project・#127 recording/player)
- 前提: `547fe08e`(ALIB-02 #220・ALIB-03 #223 merge 済み)以降の
  `origin/main` から `feature/88-alib04-git-provider` を作成。
- 全検証は offline の `file://` fixture(実 work repo + bare remote +
  手配置の `lfs/objects` store)で実施。実 Git server・network・実 OS
  credential store・GUI は NotRun/Unsupported。

## Implementation

### Changes

- `src/git/url.rs`: `GitRemoteUrl`/`GitRemoteScheme`。scheme allowlist
  (file/http/https/ssh/git + scp-like SSH + local path→`file://`)。
  `ext::`/`fd::`・password userinfo・非 SSH userinfo・malformed
  scp-like を拒否。credential は URL に埋め込まない。
- `src/git/locator.rs`: `<rev>:<path>` 文法。rev は安全な ref 部分集合、
  path は traversal・空 component・`\`・制御文字・過長を拒否。`:` は
  path 内で合法(git 自体が最初の `:` で split)。
- `src/git/objects.rs`: `GitRunner`(固定 argv・prompt 無効・credential
  env 注入・cancel polling・bounded stdout)と `ls-tree -z -l` parser
  (size column の空白 padding を whitespace 分割で許容)、mode/type
  分類(blob/executable/symlink/tree/gitlink)、LFS pointer parse、
  stderr→`ProviderErrorKind` 分類、diagnostic の secret redaction。
- `src/git/mirror.rs`: `GitMirror` — `ensure`/`refresh`/
  `resolve_commit`/`list_tree`/`stat_entry`/`blob_head`/`open_blob`/
  `lfs_tracked`/browse epoch。`--mirror` clone は一度だけ、
  `fetch --prune` は明示 refresh のみ。filter 判定は
  `rev-list --missing=print` の object census + `for-each-ref`/
  `ls-tree -r` による has-blobs 判定で、server が `--filter` を
  silent ignore した場合を `Full` として検出し、`Reject` policy では
  `UnsupportedCapability` を返す(`AllowFullMirror` のみ full mirror
  を明示許容)。
- `src/git/lfs.rs`: local `lfs/objects/<fanout>/<oid>` backend。
  oid 形式検証・宣言 size precheck・downstream SHA-256 は
  `VerifyingReader` が実施。object 未取得は `NotFound` であり
  pointer text を content として返さない。
- `src/git/mod.rs`: `GitRemoteProvider`(`AssetProvider` 実装)。
  browse/stat/pin/open_pinned/open_preview、capabilities
  (search は `UnsupportedCapability` — provider-side index 非保有)、
  cursor は JSON payload に provider/scope/query/principal/epoch/
  offset fingerprint を束縛。`git_info()` で mirror mode・object
  format・LFS backend を正直に公開。
- `src/lib.rs`: `mod git` + additive re-export のみ。依存追加なし
  (Cargo.toml/Cargo.lock 不変)。
- `tests/common/git_fixture.rs`: git CLI のみで構築する offline
  fixture(work repo + bare remote + LFS object 手配置 + gitlink +
  symlink)。ambient global git-lfs(template hook・`filter.lfs.*`)を
  fixture-local config で無効化し、committed `payload.bin` が
  pointer text verbatim であることを self-check。git 非存在時は
  `None` で明示 skip。
- `tests/git.rs`: 25 test — browse/stat/pin/open の固定 commit
  roundtrip、LFS pointer→実 object 検証、partial clone の selective
  fetch 証拠(`rev-list --missing=print`)、unfiltered remote の
  policy 分岐、mutable ref 解決と pin 安定性、force-push 後の
  pinned object 到達性、import_verified 接続、cursor 拒否、
  cancellation/budget/retry 分類、secret 非漏洩、worktree/index/
  HEAD/remote 非破壊。

### Decisions During Implementation

- Git object id(SHA-1/SHA-256)は `ContentDigest` ではない —
  `git-blob:`/`git-lfs:` の `ObjectIdentity` scheme で分離し、
  blob pin は `IdentityBound`、LFS pin のみ `StreamVerified`
  (oid = SHA-256)を主張する。
- LFS pointer は metadata — `pin`/`open`/`preview` の全経路で
  pointer text を content として serve しない。pointer-shaped blob
  で `check-attr` が失敗した場合は LFS 側に fail-safe 判定する。
- `--filter` silent ignore は clone exit code では検出不能
  (file:// transport は warning のみで成功)。mirror config の
  `partialclonefilter` で「要求されたか」を、object census で
  「実際に partial か」を判定し、両者を分離した。
- `ls-tree -l` は size column を空白 padding するため parser は
  whitespace run で split する。`-l` は列挙 blob の lazy fetch を
  引き起こす — selective の粒度は「browse した tree 直下」までで、
  未 browse の subtree blob は promisor-missing のまま残る。
- `read_to_end` は std が `ErrorKind::Interrupted` を retry するため
  mid-stream cancel の typed 検証は `read` + completion slot で行う
  (`VerifyingReader`/`FetchSlot` の ALIB-01/02 契約どおり)。
- Windows では `TempTree` canonicalize の `\\?\` verbatim prefix を
  git の path 引数が拒否するため `plain_dir`/`plain_path` で除去。
  `:` 入り filename は git-for-windows が U+F03A に変換するため
  fixture から除外(locator 文法の `:` 合法性は unit test で担保)。

## Verification

- `mise run fmt-check` / `check` / `lint` / `test` / `test-rt`:
  全て pass(workspace 全 target、locked dependency graph)。
- `mise run test-msrv`: 結果は PR 記載の最終検証値を参照。
- `cargo test -p synvelis-library --test git`: 25/25 pass
  (offline `file://` fixture、実 git CLI 使用)。
- NotRun/Unsupported: 実 Git server(HTTP/SSH/git protocol 対 live
  host)・実 OS credential store・GUI・実 network 障害 — 設計上の
  対象外。`synvelis-app`/workspace/project/recording/player/gateway/
  render/theme/poc・`.github/`・root manifest は未変更。
