---
id: "DSN-alib04-git-remote-provider"
type: "design"
status: "approved"
title: "ALIB-04 — Git Remote Provider 設計（synvelis-library）"
created: "2026-11-09"
updated: "2026-11-09"
owners: ["scarecrow173"]
relations:
  source:
    - "docs/specs/alib-04-git-remote-provider.md"
  changes:
    added:
      - type: "dir"
        path: "crates/synvelis-library/src/git"
      - type: "file"
        path: "crates/synvelis-library/tests/git.rs"
      - type: "file"
        path: "crates/synvelis-library/tests/git_fixture.rs"
    modified:
      - type: "file"
        path: "crates/synvelis-library/src/lib.rs"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/alib-04-git-remote-provider.md"
  implemented-by: []
  depends-on:
    - "docs/specs/alib-01-asset-provider-contract.md"
    - "docs/specs/alib-02-streaming-verified-import.md"
    - "docs/specs/alib-03-catalog-preview-cache.md"
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
---

# ALIB-04 — Git Remote Provider 設計（synvelis-library）

## 1. 位置づけ

ALIB-01 の `AssetProvider` 契約に対する **Remote Provider adapter**。
Git object（commit/tree/blob）と Git LFS object という 2 種の
byte 源を、ALIB-01 の pin/fetch 契約と ALIB-02 の verified
import 契約へ写像する。provider は **read-only** であり、
project aggregate・working tree・index・HEAD に一切触れない。

権威の分担:

- remote の正本 = remote repository（mirror は derived cache
  であって正本ではない。mirror 全喪失 = `ensure_mirror` で
  再 clone に落ちるだけ）。
- asset identity = provider 層の `RemoteAssetRef`/locator/
  `ObjectIdentity`。project 側の `AssetId`/`SourceId`/
  `ImportId` とは型で分離（ALIB-01 踏襲）。
- byte 検証 = 2 段。provider 層は **object identity**
  （Git OID / LFS oid+size）で固定し、import 層が staging
  bytes の **SHA-256** を測定して `ContentDigest` とする。
  Git OID（sha1/sha256 repo format、`blob <len>\0` 前置の
  object hash）と `ContentDigest`（exact bytes の SHA-256）は
  **同一視しない** — LFS oid のみ SHA-256 なので一致する。

## 2. モジュール構成

```
src/git/mod.rs       GitRemoteProvider（AssetProvider impl）+
                     公開 config/info 型の re-export
src/git/url.rs       GitRemoteUrl（scheme allowlist・
                     credential-free 検証・file:// 化・
                     lfs objects root 解決）
src/git/locator.rs   <rev>:<path> 文法の parse/validate/
                     child locator 構成
src/git/objects.rs   ls-tree -z -l 出力 parse・mode/type
                     分類・Git LFS Pointer 判別/parse・
                     git argv/env 構築・stderr→error 分類
src/git/mirror.rs    managed bare mirror（ensure_clone/
                     refresh/run_git/resolve_commit/
                     list_tree/stat_entry/blob stream/
                     filter 検出・epoch→sha map）
src/git/lfs.rs       LfsBackend（local-path lfs/objects
                     file open・size/digest 検証委譲）
```

`src/lib.rs` に `mod git;` + 公開 re-export を追加するのみで、
既存 module・公開契約は変更しない（additive）。

## 3. 公開型

```rust
pub struct GitRemoteConfig {
    pub url: GitRemoteUrl,               // 検証済み remote
    pub mirror_dir: PathBuf,             // managed mirror 親 dir
    pub reference: String,               // Root scope の既定 rev
    pub credential_key: Option<CredentialKey>,
    pub on_unsupported_filter: UnsupportedFilterPolicy,
}

pub enum UnsupportedFilterPolicy {
    /// filter 非対応 remote は UnsupportedCapability で拒否
    /// （既定 — 全量取得を黙って行わない）。
    Reject,
    /// 全量 mirror を明示許容（mirror_mode=Full を info に記録）。
    AllowFullMirror,
}

pub struct GitProviderInfo {            // capability 証拠
n    pub remote_scheme: GitRemoteScheme, // File | Http | Https | SSH | Git | Other
    pub mirror_mode: MirrorMode,        // Partial | Full | NotCloned
    pub object_format: GitObjectFormat, // Sha1 | Sha256
    pub lfs_backend: LfsBackendKind,    // LocalObjects | Unsupported
    pub partial_clone_supported: bool,
}
```

`GitRemoteProvider::open(config) -> Result<Self>` は mirror を
作らない（lazy: 初回 provider call で `ensure_mirror`）。
`refresh(cx)` は `git fetch --prune origin` を明示実行する。

## 4. GitRemoteUrl 検証

- `://` を含む場合: scheme ∈ {`file`,`http`,`https`,`ssh`,`git`}
  のみ。authority の userinfo（`@` 以前の `user:pass@`）を
  **拒否** — credential は URL に埋め込まない。
  `ext::`/`fd::`/`fd` transport は git が任意 command を
  起動し得るため scheme allowlist で排除。
- `://` を含まない場合: scp-like（`host:path`）または local
  path として受理。local path は canonicalize して
  `file://` URL 化する（plain path clone は `--local` で
  filter を無視するため、transport 強制が必須）。
- Windows file URL は `file:///C:/...` 形に正規化
  （git-lfs native binary が MSYS path を解釈できない
  実測に基づく）。
- `lfs_objects_root()`: `file://`/local path remote のみ
  `<path>/lfs/objects` を返す。他 scheme は `None`
  （LFS byte 取得 Unsupported）。

## 5. Process 起動の安全境界（LIB-SEC 対応）

全 git 起動は `src/git/objects.rs` の単一 `run_git` 経路:

- `Command::new("git")` + **固定 argv 配列**。shell 文字列
  連結・`sh -c` は使わない。ユーザ由来値（rev/path/oid）は
  grammar 検証済みのものだけ argv に入れ、path 引数は
  `--` separator の後に置く。
- env: `GIT_TERMINAL_PROMPT=0`、`GIT_ASKPASS=`/`SSH_ASKPASS=`
  空、`GIT_CONFIG_COUNT/KEY_n/VALUE_n` で
  `credential.helper=`（空=無効化）・`http.followRedirects`
  を注入。credential 解決値は `http.<remote-url>.extraHeader`
  の env config のみ（argv・remote URL・error detail に出ない。
  URL-prefix scoped config なので redirect 先の別 origin へは
  header が送られない = cross-origin forwarding 防止）。
- credential は `cx.credentials.resolve(key)` の短命結果を
  call ごとに使い、`OpaqueCredential.context()` を header
  value としてのみ解釈する。context 文字列を log/error/
  provenance/locator に書かない。
- stderr は classify 関数で typed `ProviderErrorKind` に
  写像する際、credential を含み得る行を detail に入れない
  （detail は分類ラベル + git の短い理由のみ。remote URL は
  userinfo 拒否済みで安全）。

## 6. Mirror 管理（`git/mirror.rs`）

- `ensure_mirror`: `<mirror_dir>/<provider-id-tag>/` が
  git repo でなければ `git clone --mirror --filter=blob:none
  --no-checkout <url> <dir>`。clone stderr/stdout を捕捉し、
  "filtering not recognized" を検出した場合は clone 後走査で
  最終判定する（git は filter 無視時も promisor config を
  書くため config だけでは判別不能 — 実測済み）。
- filter 検出: `git cat-file --batch-all-objects
  --batch-check='%(objecttype)'` で local object を列挙し
  `blob` が 1 件でもあれば filter 不適用 = full mirror。
  `UnsupportedFilterPolicy::Reject` → `UnsupportedCapability`、
  `AllowFullMirror` → 続行し `GitProviderInfo.mirror_mode=
  Full` を記録。判定不能（空 repo 等）は `Unknown`。
- `refresh`: `git fetch --prune origin`（mirror refspec
  `+refs/*:refs/*` が clone 時に設定済み）。branch 更新・
  force-push・deletion は次回 resolve/browse に反映される。
  provider call 自体は自動 fetch しない（明示 refresh のみ
  — browse が黙って remote を叩き続けない）。
- `resolve_commit(rev)`: `git rev-parse --verify
  <rev>^{commit}`。`HEAD` は mirror で unborn になり得る
  ため、config.reference=`"HEAD"` の場合は clone/refresh 時に
  `git ls-remote --symref <url> HEAD` で remote HEAD 対象を
  解決して実 ref 名に置き換える。sha・branch・tag・
  `refs/...` 完全形を受理。失敗 → `NotFound`（未 fetch の
  可能性を detail に明示し refresh を促す）。
- epoch→sha map: browse 継続 cursor 発行時に
  `epoch → (commit_sha, tree_path)` を `Mutex<BTreeMap>` に
  記録（`MAX_LIVE_EPOCHS` 上限は local provider と同じ
  512）。次 page は **記録した sha** で `ls-tree` するため、
  branch が動いても paging 対象 commit は固定される。

## 7. Browse/Stat（objects.rs + mirror.rs）

- `list_tree(sha, path)`: `git ls-tree -z -l <sha>:<path>`
  （path 空は `<sha>`）。`-z` で raw name、`-l` で size。
  出力 1 行 = `<mode> SP <type> SP <oid> SP <size|-> TAB
  <name> NUL`。`max_entries` 超過時は stdout 読み取りを
  打ち切り child を kill → `truncated`（`Coverage::Partial`）。
  sort は収集後に `sort_entries`（local provider と同一
  決定的規則）。
- entry 分類: `040000 tree`→Container、`100644/100755 blob`→
  Asset、`120000 blob`→Asset+via_symlink、`160000 commit`
  (gitlink)→Asset+`Availability::Unavailable{NotApplicable,
  "git submodule"}`、その他 mode/type→Other として
  Unavailable。`commit` type は submodule として扱う
  （tree entry 種別の確認 — Issue 要求）。
- `EntryFacts`: `byte_len`=ls-tree size（tree は `-` →
  `Unknown{NotApplicable}`）、`modified`=`Unknown{NotProvided}`
  （git tree は mtime を持たない）、`kind`=観測値、
  `checksum_hint`=`Observed "git-blob:<oid>"`（観測された
  object id であり SHA-256 ではないことを scheme prefix で
  区別）、media=`all_unknown`。
- `stat`: `git ls-tree -z -l <sha> -- ':(literal)<path>'`
  で単一 entry を取得（pathspec glob を literal magic で
  無効化）。0 件→`NotFound`、複数→先頭のみ採用せず
  `Internal`（起こり得ないはずだが fail-closed）。
  locator path に `:` を含む場合は文法で拒否済み。

## 8. Locator 文法（locator.rs）

- 形式: `<rev>:<path>`、`<rev>:`（root）、`<rev>`（root と
  同義）。rev は `:` を含まない（最初の `:` で分割）。
- rev grammar: `[A-Za-z0-9][A-Za-z0-9._/-]{0,255}` かつ
  `..`・`//`・先頭 `-`・末尾 `/`・`.lock` 終端を拒否
  （git ref 規則の安全部分集合 + sha hex も受理）。
- path grammar: `/` 区切り、各成分は空・`.`・`..`・NUL・
  制御文字・`\`・`:` を拒否、成分長 ≤255、全体 ≤2048。
  先頭 `/`・末尾 `/` は正規化で拒否（root は path 空のみ）。
- child locator: `<rev>:<parent>/<name>`。name は ls-tree
  の raw bytes を UTF-8 検証したもののみ（非 UTF-8 は
  listing で skip + `truncated`）。

## 9. Pin/Fetch（LFS 含む）

- `pin`: locator→`(rev,path)`→`resolve_commit`→`stat_entry`
  →blob のみ受理（tree/gitlink/不在は typed error）。
  `ls-tree` size > `cx.limits.max_bytes` → `BudgetExceeded`
  （fetch 前に拒否）。blob head を `git cat-file blob <oid>`
  で ≤`LFS_POINTER_SCAN_LIMIT`(1024)B 読み、LFS Pointer
  format（`version https://git-lfs.github.com/spec/v1` +
  `oid sha256:<64hex>` + `size <u64>`、全体 ≤1024B、size
  が pointer size と矛盾しない）に合致すれば LFS asset。
- LFS pin: `expected_size`=pointer `size`、`digest_hint`=
  `ContentDigest::parse("sha256:<oid>")`、`object_identity`=
  `"git-lfs:<oid>"`、`expected_size` が `max_bytes` 超過
  なら `BudgetExceeded`。
- blob pin: `expected_size`=tree size、`digest_hint=None`、
  `object_identity`=`"git-blob:<oid>"`。
- `open_pinned`: `PinProof::ImmutableObject` 以外・foreign
  provider・未知 object_identity scheme → `InvalidRequest`。
  - `git-blob`: `git cat-file blob <oid>` spawn、stdout を
    `Read` 化し `VerifyingReader`（`expected_digest`=
    `digest_hint`（通常 None）、`expected_size`、
    `max_bytes`、cancel/deadline）で包む。claims は
    `IdentityBound{scheme:"git-object-id"}`（oid が内容
    アドレスで byte 完全性は import 層の SHA-256 に委譲）。
    promisor 未取得 object は cat-file が lazy fetch する
    = selective fetch。
  - `git-lfs`: `LfsBackend::open(oid,size)` で
    `<remote>/lfs/objects/aa/bb/<oid>` を `File` open
    （size 事前照合→不一致 `IntegrityMismatch`、不在
    `NotFound`、LFS 未対応 scheme `UnsupportedCapability`）。
    `VerifyingReader` に `expected_digest=sha256:<oid>`・
    `expected_size=size` を渡し `StreamVerified`。
    いずれの失敗でも Verified に昇格しない。
- `open_preview`: asset→stat 相当の解決→blob は
  `cat-file` stream を `offset` まで読み捨て（`max_bytes`
  計上）後 `range.len` まで、LFS は `File::seek`+`take`。
  `VerificationPolicy::Unverified`。
- `search`: default `UnsupportedCapability{RequiresIndex}`
  （provider-side index なし — 空結果を偽装しない）。

## 10. Error 分類（stderr → ProviderErrorKind）

`run_git` が非 0 終了時に stderr を走査し、先に一致した
分類を採用する（secret を含み得る生 stderr は detail に
入れない — 分類タグ + 汎用 detail のみ）:

- `could not resolve host`/`connection refused`/`failed to
  connect`/`early eof`/`rpc failed`/`timeout`/`operation
  timed out` → `Offline`（`TransientIo` 相当の
  `RetryAdvice::After`）
- `authentication failed`/`could not read username`/`403`/
  `401`/`access denied` → `AuthExpired`（`AfterAuth`）
- `permission denied` → `PermissionDenied`（`Never`）
- `repository not found`/`not found`/`does not appear to be
  a git repository` → `NotFound`
- `rate limit`/`429`/`too many requests`/`secondary rate` →
  `RateLimited`（`After(1s)`）
- `filtering not recognized`/`filter not supported` →
  `UnsupportedCapability`
- cancel token/deadline → `Cancelled`/`BudgetExceeded`
- その他非 0 → `TransientIo`（`After`）
- spawn 自体の失敗（git 不在等）→ `TransientIo`。

`rev-parse`/`ls-tree`/`cat-file` の object 不在は git の
"Not a valid object name"/"does not exist"/"path ... does not
exist" → `NotFound`/`RevisionGone`（pin 対象が remote で
消えた場合は `RevisionGone`+`AfterUserRePin`）。

## 11. 検証計画（fixture）

`tests/git_fixture.rs` が `std::process::Command` で git CLI
を直接起動して fixture を構築する（crate 依存追加なし）:

- `GitFixture::new()`: temp dir に work repo（`git init`）+
  bare remote（`git init --bare`）+ push。remote に
  `uploadpack.allowFilter=true`・`uploadpack.allowAnySHA1InWant
  =true` を設定（file:// transport で filter/lazy fetch を
  有効化 — 実測確認済み）。
- LFS: work repo で `git lfs install --local` +
  `git lfs track "*.wav"` + commit + push（remote の
  `lfs/objects` に実 object が置かれる）。
- `git`/`git lfs` 非存在時は fixture 構築を `Option` で返し
  test 側で明示 skip（NotRun を Pass にしない）。
- 変種: branch 更新（追加 commit push→`refresh`→新 sha）、
  force-push（`push --force`→同）、同名異 object（同 path
  別内容→blob oid 相違）、deletion（branch 削除→fetch
  --prune→resolve 失敗）、submodule（`git submodule` または
  手動 gitlink commit — submodule add は network 不要の
  local path で可）、symlink（unix のみ・Windows は
  開発者モード依存 → `try_symlink` 既存 helper で skip
  判定）、odd path（空白・Unicode・`..`類似名・長い名）。
- 失敗系: LFS missing（remote `lfs/objects` から object
  削除）、LFS corrupt（object bytes 改竄→sha256 mismatch）、
  pointer のみ（size field 改竄）、auth（credential_key
  未解決→`AuthExpired`/resolver error 伝播、resolved 値が
  argv/URL/log に出ないことを検査）、切断（mirror_dir を
  破壊→ensure 再 clone or TransientIo）、cancel
  （`CancellationToken::cancel` で `Cancelled`）、
  filter 非対応（allowFilter を remote から外した別 remote
  →`UnsupportedCapability` or `AllowFullMirror` で
  `mirror_mode=Full`）。

`tests/git.rs` の test 群は ALIB-01 contract test と同じ
「browse→stat→pin→open_pinned→bytes」の列を Git provider に
適用し、さらに import_verified への接続（pin→ImportIntent
→staging→verify→publish→commit）を project fixture で
通す（ALIB-02 の `tests/import.rs` の組立を再利用）。

## 12. 非対象の明示

- http(s)/ssh/git:// 実通信・GitHub Hosted API・LFS batch
  API・OS Credential Store: capability/info で区別し
  NotRun/Unsupported。`GitRemoteScheme` は分類のみ。
- VCS Provider（#94）・Working Tree/Index/HEAD 操作:
  本 crate は一切触れない。
- `synvelis-app` 配線・GUI: #89 の範囲。
