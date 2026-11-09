---
id: "SPEC-alib04-git-remote-provider"
type: "spec"
status: "approved"
title: "ALIB-04 — Git Remote Provider（fixed Commit/Tree・Git LFS・selective fetch・auth）（synvelis-library slice）"
created: "2026-11-09"
updated: "2026-11-09"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/88"
    - "https://github.com/scarecrow173/synvelis/issues/9"
    - "https://github.com/scarecrow173/synvelis/issues/85"
    - "https://github.com/scarecrow173/synvelis/issues/86"
    - "https://github.com/scarecrow173/synvelis/issues/87"
    - "https://github.com/scarecrow173/synvelis/issues/91"
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
  implements: []
  implemented-by: []
  depends-on:
    - "docs/specs/alib-01-asset-provider-contract.md"
    - "docs/specs/alib-02-streaming-verified-import.md"
    - "docs/specs/alib-03-catalog-preview-cache.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/alib-02-v5-snapshot-compat.md"
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

# ALIB-04 — Git Remote Provider（synvelis-library slice）

## What Should Be Built

`synvelis-library` に、ALIB-01 の `AssetProvider` 契約を実装する
**最初の Remote Provider** `GitRemoteProvider` を追加する。
Git remote の **固定 Commit/Tree** を browse/stat/pin/fetch し、
Git LFS Pointer を実 Byte へ解決して ALIB-02 の verified import
へ供給する。Git の作業ツリー全量 checkout、Project の VCS
commit/branch 変更を Import の前提としない。

対象は library 契約と実装のみ（offline git fixture:
local bare repo / `file://` remote / bundle）。実 Git
server（git://・http(s)・ssh 実 network）・実 OS・GUI・Hosted
API 最適化・OS Credential Store adapter は本 slice では実装
しない — capability として正直に宣言し、NotRun/Unsupported を
Pass にしない。

## Why Now

ALIB-01（#85）で provider 境界・`Coverage`/cursor 契約、
ALIB-02（#86, PR #220 merged）で verified import・SHA-256
namespace、ALIB-03（#87, PR #223 merged）で catalog/preview
cache が揃った。Issue #88 はそれらに接続する最初の Remote
Provider であり、#91 の LIB-PIN「Git fixed commit/tree, Git LFS
real bytes」と LIB-SEC「Git process argv safety」の対象でも
ある。VCS-03（#94）は本 provider の transport を再利用する
前提で分離済み。

## Users and Value

- **後続 slice の実装者**（#89 GUI Import・#91 Fault Matrix）:
  `AssetProvider` 越しに Git asset を pin→import できる。
  `GitProviderInfo` で mirror mode・object format・LFS backend
  の capability 証拠を取得できる。
- **Project 利用者**: branch/ref を選んで browse し、必要な
  blob/LFS object だけを選択取得して Project へ import できる。
  remote の更新・force-push・deletion が既存 Import を壊さない。
- **セキュリティレビュア**: Git OID（sha1/sha256）と Project
  `ContentDigest`（SHA-256 of exact bytes）が別型・別保証で
  あること、token が URL/Provenance/log に残らないこと、
  process argv が固定配列で shell 連結しないことを fixture と
  test で確認できる。

## Scope

### In Scope

- `GitRemoteProvider`（`AssetProvider` 実装、
  `ProviderKind::GitRemote`、`AccessMode::ReadOnly`）。
- `GitRemoteConfig`/`GitRemoteUrl`: remote URL 検証
  （scheme allowlist・credential-free・`ext::`/`fd::` 等の
  command 実行 transport 拒否）、managed bare mirror
  （`git clone --mirror --filter=blob:none --no-checkout`）、
  `refresh()`（`git fetch --prune origin`）、credential
  `CredentialKey` 参照（resolve は `ProviderContext` 経由、
  `http.<url>.extraHeader` を env config で注入、argv/URL/
  provenance/log に secret を残さない）。
- Locator 文法 `<rev>:<path>`（rev = branch/tag/commit-ish、
  path = tree 内 `/` 区切り、`:`・`..`・NUL・制御文字・`\` を
  拒否）。`BrowseScope::Root` は config の既定 reference を
  解決し、`Locator` scope は `<rev>:<path>` を対象 tree とする。
- browse: `git ls-tree -z -l` による 1 level 列挙
  （メタデータ中心・bounded・決定的 sort・offset cursor）。
  epoch→解決済み commit sha を provider が保持し、page 継続は
  **pin 時点の commit に固定**（branch が動いても paging 中の
  byte 対象を変えない）。tree→Container、blob→Asset、
  symlink(120000)→via_symlink 付き Asset、gitlink(160000/
  `commit` type)→fetch 不可の正直な Unavailable、非 UTF-8
  名・budget 打ち切り→`Coverage::Partial`。
- stat: `git ls-tree -z -l <sha> -- ':(literal)<path>'` で
  mode/type/oid/size を観測。`modified`/media facts は
  tree が保持しないため `Unknown`（捏造しない）。
- pin: rev→commit sha 解決→tree entry 確認→blob head
  （≤1KiB）で Git LFS Pointer を判別。
  - 通常 blob: `PinProof::ImmutableObject{revision:"commit:<sha>"}`、
    `object_identity="git-blob:<oid>"`、`expected_size`=tree
    size、`digest_hint=None`（Git OID は SHA-256 of bytes では
    ない — Project `ContentDigest` とは別保証）。
  - LFS Pointer: `object_identity="git-lfs:<oid sha256>"`、
    `expected_size`=pointer の `size`、`digest_hint=
    sha256:<oid>`（LFS oid は内容の SHA-256 なので pin 時点で
    強 digest を持てる）。
- open_pinned: `git cat-file blob <oid>` の stdout を
  `VerifyingReader` で包む streaming fetch（promisor mirror
  では未取得 object のみ lazy fetch — selective）。
  LFS asset は `<remote>/lfs/objects/aa/bb/<oid>` を直接開き
  `StreamVerified{sha256:<oid>}` で size+digest を EOF 検証。
  LFS object 欠落・認証不可・size/SHA-256 mismatch は
  Verified に昇格しない typed error。
- open_preview: blob は先頭から `offset` まで読み捨てる
  bounded stream、LFS は file seek。明示的に Unverified。
- search: provider-side index を持たないため
  `UnsupportedCapability{RequiresIndex}`（偽の空結果を返さない）。
- `GitProviderInfo`: mirror mode（partial/full/unknown）、
  object format（sha1/sha256）、LFS backend、remote scheme、
  filter 対応可否の capability 証拠。
- Git fixture（`tests/git_fixture.rs`）: temp dir に
  work repo + bare remote + `file://` mirror を `git` CLI で
  構築し、branch 更新・force-push・deletion・submodule・
  symlink/odd path・LFS missing/corrupt・auth 失敗・cancel/
  retry を再現。`git`/`git-lfs` 非存在環境では test を
  skip せず fixture 構築失敗として正直に落とさず、環境判定で
  NotRun 相当の skip を明示する。
- docs チェーン（spec/design/plan/task/test-spec）。

### Out of Scope

- `synvelis-app`/`synvelis-workspace`/`synvelis-project`/
  `synvelis-recording`/`synvelis-player`/`synvelis-render`/
  `synvelis-gateway`/`synvelis-theme`/`recording-poc` への配線。
  Project Commit/Asset 正本との統合は #86 の公開契約
  （`import_verified`）で行い、本 slice は provider+pin の
  供給側のみ。
- 実 Git server/network（git://・http(s)・ssh 実通信）、
  GitHub 等 Hosted API 最適化、OS Credential Store adapter、
  LFS batch API 独自実装、HTTP range fetch。
- Git WorkTree/Index/HEAD の変更（mirror は `--mirror`
  bare clone で remote の投影のみ。project 側 VCS 状態に
  一切触れない）。
- Ruviform Generic File/Network Security の重複実装
  （process 起動・redaction は本 crate の provider 層責務として
  最小実装し、汎用化は Ruviform Issue へ還元）。
- 未取得 LFS Byte を Verified Asset とすること（Issue 明示禁止）。
- 実 OS・実デバイス・GUI・AudioLens/RT/Golden の変更
  （非回帰は既存 test で確認）。

## Requirements

- Commit 固定→Tree browse→単一 blob と Git LFS 実 Byte の
  選択取得→SHA-256 検証→Project Import→offline 再生が
  fixture で通る。選択外 media を無断で全量同期しない
  （`blob:none` partial mirror + object 単位 lazy fetch +
  LFS は対象 oid のみ）。
- Mutable branch/ref は pin で実 Commit SHA + Tree/Blob OID
  へ解決され、以後 open_pinned は pin の object のみを読む。
  remote 更新・force-push・deletion が既存 pin/import を
  変えない（再 pin は明示操作）。
- LFS Pointer だけ・LFS missing・SHA/size mismatch・
  auth expired・切断・cancel・retry の結果が typed error
  として正確。Project/Working Tree は変更しない。
- Git OID（repo object format）と Project `ContentDigest`
  （exact bytes の SHA-256）は別型・別保証として扱う。
  submodule/symlink/gitlink/tree entry の種別を確認し、
  外部任意 URL・path traversal・redirect・cross-origin
  credential forwarding を防ぐ。
- Credential は App-managed store（`CredentialResolver`）
  から接続 scope に応じ短命取得。token を remote URL・
  Project Provenance・log・error detail に残さない。
  任意 `git` command を shell 文字列連結しない
  （固定 argv 配列 + `--` separator + 値の grammar 検証）。
- Partial clone/filter 非対応 host では Unsupported または
  明示 fallback/予算超過拒否 — repository 全量取得を
  黙って行わない（clone 後に local object 走査で filter
  無視を検出し `GitRemoteConfig` の policy に従う）。
- 既存 ALIB-01/02/03 API・挙動を破壊しない（全て additive）。

## Acceptance Criteria

- [ ] offline fixture（local bare repo + `file://` remote、
  `uploadpack.allowFilter`/`allowAnySHA1InWant` 有効）で
  branch 更新・force-push・同名異 object・deletion・
  submodule・symlink/odd path を再現し、固定 Commit/Tree
  browse・単一 blob/LFS 実 Byte の選択取得・SHA-256 検証・
  Project Import・offline 再生が通る。
- [ ] LFS Pointer のみ・LFS missing・SHA/size mismatch・
  auth expired・切断・cancel・retry の各 failure が typed
  error として正確で、Verified に昇格しない。
- [ ] `pin` が branch→実 Commit SHA+OID に固定し、
  以後の `open_pinned`/`import_verified` が remote 更新後も
  同じ byte を返す。Project/Working Tree/Index/HEAD 不変。
- [ ] provider capability（browse/stat/search/pin/range/
  preview/fetch/checksum）と検索 partiality、
  `GitProviderInfo` の mirror mode/object format/LFS backend/
  filter 対応、secret redaction（URL/argv/log/provenance に
  token 非含有）を test で証拠化。
- [ ] `mise run fmt-check`/`check`/`lint`/`test`/`test-rt`/
  `test-msrv`/`build` が全て pass。既存 test の非退行
  （AudioLens/RT/Golden 含む）。実 Git server/network・
  実 OS・GUI は NotRun/Unsupported として記録。

## Deferred Design Concerns

- http(s)/ssh/git:// remote の実通信・GitHub Hosted API・
  LFS batch API・OS Credential Store | 本 slice は
  `file://`/local path remote のみ実測。scheme は capability
  で区別し、未検証を Pass としない | 実 network を持つ
  検証 slice（#91 consumer 別判定へ伝達）| network 依存を
  fixture に持ち込まない。
- `git lfs fetch` 経由の mirror 内 LFS object 複写 | 本 slice
  は remote の `lfs/objects` を直接読む（standalone-file
  agent と同じ byte 源・oid 単位で選択的）。remote が
  file/path でない場合は Unsupported | http LFS が必要に
  なった時点 | path 解決の汎用性より byte 源の正直さを優先。
- huge blob の pin 時 full-object lazy fetch | blob 単位の
  選択取得であり repo 全量ではない。`max_bytes` で pin 前に
  budget 拒否 | 実測で pin コストが問題化した時点 | YAGNI。
- VCS-03（#94）との transport 共有 | 本 provider の
  commit/tree/blob read は再利用可能な形にするが、VCS
  Provider（History/Working Tree 権限）とは別 Port | #94 |
  共有不可境界（Working Tree/Index/HEAD）を越えない。

## Sources and Relations

- Issue #88（本 spec の直接の根拠）、#9（Asset Library 親・
  ALIB 詳細契約）、#85/ALIB-01、#86/ALIB-02、#87/ALIB-03、
  #91（Fault Matrix consumer）、#94（VCS-03 transport 再利用
  前提）。
- `docs/specs/alib-01-asset-provider-contract.md`、
  `docs/specs/alib-02-streaming-verified-import.md`、
  `docs/specs/alib-03-catalog-preview-cache.md`、
  対応する designs。
- `git` CLI（clone/fetch/ls-remote/ls-tree/cat-file/rev-parse）
  と `git-lfs`（fixture 構築のみ。provider は `lfs/objects`
  の file read で byte を取得し git-lfs API を呼ばない）。
