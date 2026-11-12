---
id: "IR-alib07-fault-matrix"
type: "impl"
status: "completed"
title: "ALIB-07 Fault Matrix — headless 実装記録（synvelis-library）"
created: "2026-11-10"
updated: "2026-11-11"
owners: ["scarecrow173"]
relations:
  implements:
    - "docs/plans/alib-07-fault-matrix.md"
    - "docs/designs/alib-07-fault-matrix-design.md"
    - "docs/specs/alib-07-fault-matrix.md"
  verified-by:
    - "docs/test-specs/alib-07-fault-matrix.md"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-library/tests/common/webdav_fixture.rs"
      - type: "file"
        path: "docs/specs/alib-07-fault-matrix.md"
      - type: "file"
        path: "docs/designs/alib-07-fault-matrix-design.md"
      - type: "file"
        path: "docs/plans/alib-07-fault-matrix.md"
      - type: "file"
        path: "docs/tasks/alib-07-fault-matrix.md"
      - type: "file"
        path: "docs/test-specs/alib-07-fault-matrix.md"
      - type: "file"
        path: "docs/impl/ir/alib-07-fault-matrix.md"
    modified:
      - type: "file"
        path: "crates/synvelis-library/tests/common/mod.rs"
      - type: "file"
        path: "crates/synvelis-library/tests/git.rs"
      - type: "file"
        path: "crates/synvelis-library/tests/import.rs"
      - type: "file"
        path: "crates/synvelis-library/tests/webdav.rs"
    deleted: []
    renamed: []
    moved: []
    generated: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# ALIB-07 Implementation Record

## Summary

Synvelis #91（ALIB-07 Asset Library Fault Matrix）の headless
slice（PR #236 で merge 済み）。`crates/synvelis-library` の
headless test/fixture のみを拡張し、製品コードは変更しない。
Issue #91 が固定する Case ID（LIB-ID/BROWSE/PIN/IMPORT/MIG/
CACHE/SEC/QOS、計 42 class）に対する headless 再現可能な
fault/golden case を追加し、Expected / Observed / Pass-Fail-
NotRun を Case ID 単位で `docs/test-specs/
alib-07-fault-matrix.md` に記録した。`webdav_fixture.rs` の
fixture 抽出により git 3 件・webdav 9 件・import 2 件の新規
test を追加した。

## Context

- Issue: <https://github.com/scarecrow173/synvelis/issues/91>
  （ALIB-07。親・設計オーナー: #9。前提 spec: #85 ALIB-01
  契約・#86 ALIB-02 import・#87 ALIB-03 cache・#88 ALIB-04 git
  provider・#90 ALIB-06 WebDAV）。
- 上流 doc chain: `docs/specs|designs|plans|tasks|
  test-specs/alib-07-fault-matrix*`。
- 境界: `crates/synvelis-library` の test/fixture/docs のみ。
  Provider 実装本体・GUI・実 network/実 WebDAV server/実 Git
  remote/OS credential store/実 device は対象外で、
  `NotRun`/`Unsupported`/`NotImplemented` として正直に記録する
  （Pass に昇格させない）。
- 後続 slice: `docs/impl/ir/alib-07-import-cancel.md`
  （PR #256、LIB-IMPORT-04 publish/commit 相 cancel・
  LIB-IMPORT-08 subprocess crash）。

## Implementation

本 slice は製品コードを変更しない。`crates/synvelis-library` の
headless test/fixture のみを拡張し、Issue #91 の Case ID 別
証拠を `docs/test-specs/alib-07-fault-matrix.md` に記録した。

### Fixture 抽出

`crates/synvelis-library/tests/common/webdav_fixture.rs` を新設し、
`tests/webdav.rs` 内の in-process mock HTTP server
（`MockServer`/`MockRequest`/`MockResponse`/`Handler`）と DAV 応答
builder を移動。`MockResponse::raw` を追加して fixture の
head writer を介さない malformed response を注入可能にした。
`tests/common/mod.rs` に `pub mod webdav_fixture` を登録。

### 追加 test

- `tests/git.rs`（3 件）: offline remote typed failure +
  mirror 正直な stale、remote rename + pin/fixed-commit 不変、
  hostile locator/URL fail-closed（argv 構築前拒否・`-` 始まり
  path のデータ解決・`ext::`/`fd::`/userinfo 拒否）。
- `tests/webdav.rs`（8 件 + identity encoding 1 件）: offline
  typed、auth revoke → `AuthExpired`、malformed response head、
  truncated Content-Length、malformed chunked、same-origin
  redirect credential 維持、cross-origin redirect 接続前拒否
  （foreign hit=0）、`accept-encoding: identity` 宣言。
- `tests/import.rs`（2 件）: stage 相 cancel の typed
  `Cancelled` + 同一 ImportId resume、cross-provider 同一 bytes
  の `BlobMaterialization::Reused` による blob 共有。

## Related Experiments

### Adopted

なし（本 slice は実験を伴わない test/fixture/docs 作業。
`metadata.experiments` は adopted/rejected とも空で、
`docs/impl/exp/` に関連する実験ログは存在しない）。

### Rejected

なし。

## Validation

- `cargo test --test webdav`（library）: 41 passed / 0 failed
  （新規 9 case 含む全件 green）。
- `cargo test --test webdav requests_declare_identity_encoding`:
  1 passed（個別確認）。
- `cargo check --test webdav`（コンパイル確認済み）。
- `mise run fmt-check`: exit 0。
- `mise run check`: exit 0。
- `mise run lint`: exit 0（初回 `duplicated_attributes` を
  fixture の inner attribute 重複で検出し修正後 green）。
- `mise run test`: exit 0（workspace 全件。library git 28 /
  import 17 / webdav 42 / rt_cache 1 を含む）。
- `mise run test-rt`: exit 0。
- `mise run test-msrv`（rust 1.89）: exit 0。
- `doc_maintenance.js apply`: managed index 5 件再生成、
  findings 519→502。残存する alib-07 findings は alib-06 と
  同一 category（ID 形式・Markdown link 未到達・相互 relation）
  のみで新規 category の増加なし。

## Risks

- **fixture 抽出による既存 test 破壊**: `tests/webdav.rs` 内の
  mock server を `tests/common/webdav_fixture.rs` へ移動する
  際、既存 test を壊すリスクがあった。`cargo test --test
  webdav` で全件 green を確認してから他 case を追加する順序で
  対処し、41 passed / 0 failed を確認済み（plan の risk
  register どおり）。
- **offline git case の error kind 不確定性**: offline remote
  の error 分類は transport 依存のため、確定的なもの
  （LFS `NotFound`）は exact match、分類が揺れるものは
  `Offline|TransientIo|NotFound` の typed 集合で assert して
  いる。環境差で typed 集合の境界が変わる可能性は残る。
- **doc graph の既存 blocking findings**: repo 全体に
  pre-existing の broken relation があり、route_graph は
  blocked を返す。本 slice の docs は新規 finding を増やさない
  ことを `doc_status.js` で確認し、既存不具合の修復は
  スコープ外とした（plan の risk register どおり）。
- **Mock/headless 証拠の非昇格**: in-process mock・subprocess
  fixture の成功を実 provider・実 durability・実 OS crash
  保証と読み替えない境界を維持する。`NotRun`/
  `Unsupported`/`NotImplemented`/`Unknown` を Pass に昇格
  させないことが spec の非昇格条件。

## Follow-ups

- 実 network / 実 WebDAV server / 実 Git remote（SSH・HTTP
  transport・LFS batch API）: NotRun。
- OS credential store 実機: NotRun。
- 実 device / 実 GUI / Linux・macOS 実機: NotRun。
- publish/commit 相 cancel の注入点: slice-2
  （`docs/impl/ir/alib-07-import-cancel.md`・PR #256 merge 済み）
  で解消済み。
- v5→SHA-256 移行本体（CRC 昇格）: #86（PR #253 merge 済み）の
  責務。本 slice は namespace 共存のみ Pass（吸収しない）。
- LIB-PIN-02 remote LFS（batch API/HTTP download endpoint）:
  capability 宣言上 `Unsupported`/`NotRun` のまま。
- SVN/P4/SMB/S3 provider: NotImplemented（matrix に行を設けず
  暗黙 PASS にしない）。

上記の残存受入行が Issue #91 を open 維持する根拠である
（headless slice は PR #236・PR #256 で merge 済みだが、
real-env lane・cross-gate・deferred・#89/#114 所有行は未消化）。
LIB-IMPORT-07（複数 window）・LIB-QOS-04（実 device/GUI）は
#89/#114 の責務であり、本 slice の証拠では閉じない。

詳細は `docs/test-specs/alib-07-fault-matrix.md` の
Case ID 別 matrix を参照。
