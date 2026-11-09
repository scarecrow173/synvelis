---
id: "PLAN-alib04-git-remote-provider"
type: "plan"
status: "in-progress"
title: "ALIB-04 実装計画 — Git Remote Provider（synvelis-library slice）"
created: "2026-11-09"
updated: "2026-11-09"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/88"
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
    - "docs/designs/alib-04-git-remote-provider-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/alib-01-asset-provider-contract.md"
    - "docs/specs/alib-02-streaming-verified-import.md"
    - "docs/specs/alib-03-catalog-preview-cache.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/alib-04-git-remote-provider.md"
---

# ALIB-04 Plan

## Task decomposition

### Task 1 — git 基盤 module（url/locator/objects/mirror/lfs）

- [ ] `src/git/url.rs`: `GitRemoteUrl` parse/validate
      （scheme allowlist・userinfo 拒否・`ext::`/`fd::` 拒否・
      local path→`file://` 正規化・`lfs_objects_root`）。
- [ ] `src/git/locator.rs`: `<rev>:<path>` 文法、child locator、
      scope→(rev,path) 分解。
- [ ] `src/git/objects.rs`: `ls-tree -z -l` parser・entry 分類・
      LFS Pointer parse・`run_git`（固定 argv/env/credential
      注入/interrupt 確認）・stderr→`ProviderErrorKind` 分類・
      `GitProviderInfo` 補助。
- [ ] `src/git/mirror.rs`: `ensure_mirror`（`clone --mirror
      --filter=blob:none --no-checkout`）・filter 検出
      （`--batch-all-objects` で blob 有無）・`refresh`
      （`fetch --prune`）・`resolve_commit`・`list_tree`/`stat`
      wrapper・epoch→sha map・`GitProviderInfo` 組立。
- [ ] `src/git/lfs.rs`: `LfsBackend`（local path
      `lfs/objects` file open）+ unsupported scheme error。

### Task 2 — provider + exports

- [ ] `src/git/mod.rs`: `GitRemoteProvider`（`AssetProvider`
      実装: browse/stat/pin/open_pinned/open_preview/search/
      capabilities）、`GitRemoteConfig`/`GitProviderInfo`/
      `UnsupportedFilterPolicy`/`GitRemoteScheme`/
      `GitObjectFormat`/`MirrorMode`/`LfsBackendKind`。
- [ ] `src/lib.rs`: `mod git;` + re-export（additive のみ）。

### Task 3 — fixture + tests

- [ ] `tests/git_fixture.rs`: temp git repo 構築 helper
      （work/bare/`file://` mirror/LFS/submodule/secrets）。
- [ ] `tests/git.rs`: contract 相当の browse→stat→pin→
      open_pinned→bytes、LFS 実 Byte、selective fetch 証拠
      （未選択 object が mirror に来ない）、pin 固定性
      （branch 更新後も同 bytes）、import_verified 接続、
      failure 分類（LFS missing/corrupt/mismatch/auth/
      cancel/filter 非対応）、redaction 検査、cursor paging。

### Task 4 — 検証・docs 仕上げ

- [ ] `mise run fmt-check`/`check`/`lint`/`test`/`test-rt`/
      `test-msrv`/`build`（+ 存在すれば app-native 系）。
- [ ] managed index 再生成・impl record・`git fetch origin
      main && git merge origin/main` 後の再検証。

## File map

| path | 役割 |
| --- | --- |
| `crates/synvelis-library/src/git/mod.rs` | provider + 公開 config/info |
| `crates/synvelis-library/src/git/url.rs` | remote URL 検証 |
| `crates/synvelis-library/src/git/locator.rs` | locator 文法 |
| `crates/synvelis-library/src/git/objects.rs` | git 出力 parse・argv/env・error 分類 |
| `crates/synvelis-library/src/git/mirror.rs` | mirror lifecycle・resolve/list/stat |
| `crates/synvelis-library/src/git/lfs.rs` | LFS object open |
| `crates/synvelis-library/src/lib.rs` | module+re-export（additive） |
| `crates/synvelis-library/tests/git_fixture.rs` | fixture builder |
| `crates/synvelis-library/tests/git.rs` | acceptance tests |

依存追加なし（`Cargo.toml`/`Cargo.lock`/`mise.toml` は
変更しない — git 呼出は `std::process::Command`、fixture も
同様。dev-dep 追加も不要）。

## Risk register

| risk | 対応 |
| --- | --- |
| file:// transport が filter を無視する環境 | fixture remote に `uploadpack.allowFilter`/`allowAnySHA1InWant` を設定。非対応時は `UnsupportedFilterPolicy` で分岐を test |
| Windows で `git lfs`/`git` 非存在 | fixture が `Option` を返し test は明示 skip（NotRun を Pass にしない） |
| symlink fixture が Windows 権限で作れない | 既存 `try_symlink` helper で skip 判定、結果を正直に記録 |
| huge blob の pin 時 lazy fetch | `max_bytes` で pin 前に `BudgetExceeded`。object 単位の選択取得であり repo 全量ではないことを spec に明記 |
| git stderr に secret/URL が混入 | classify は分類タグのみ detail 化。credential は env config 注入で argv/URL に出ない |
| mirror HEAD unborn（`--mirror` clone） | `reference="HEAD"` は `ls-remote --symref` で remote HEAD 解決。既定は明示 ref 推奨を docs に記載 |

## Verification matrix

| 検証 | 手段 |
| --- | --- |
| fmt/lint/check/test | `mise run fmt-check check lint test build test-rt test-msrv` |
| selective fetch | fixture で選択 blob のみ lazy fetch されることを `count-objects`/pack 列挙で確認 |
| LFS 実 Byte | pointer→oid/size→`lfs/objects` file→sha256+size EOF 検証→import |
| pin 固定性 | branch 更新・force-push 後も `open_pinned` が同 bytes |
| failure 分類 | LFS missing/corrupt/mismatch/auth/cancel/filter 非対応の typed error |
| redaction | credential 解決値が argv/env(一部)/URL/log/error に出ないことを検査（env config は secret を含むが外部露出なし — test では注入経路の存在と URL 非含有を確認） |
| 非回帰 | 既存 `tests/*.rs` 全 pass（ALIB-01/02/03） |
| NotRun/Unsupported | 実 Git server/network・実 OS・GUI・http(s) remote・LFS batch API を明記 |

## Rollback

全変更は `crates/synvelis-library/` 内の additive module +
`tests/` + `docs/` のみ。revert commit で完全に戻せる。
`Cargo.toml`/`Cargo.lock`/`mise.toml`/`.github/` は不変。
