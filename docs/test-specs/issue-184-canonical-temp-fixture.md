---
id: "TSPEC-034U2HrQ3Myiuakuy9dVPa"
type: "test-spec"
status: "approved"
title: "Issue #184 canonical temp fixture 検証仕様"
created: "2026-11-08"
updated: "2026-11-08"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/184"
    - "https://github.com/scarecrow173/synvelis/actions/runs/42431461645"
  changes:
    added: []
    modified:
      - type: "file"
        path: "crates/synvelis-library/tests/common/mod.rs"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on:
    - "docs/specs/issue-184-ci-verify-symlink-fixture.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/issue-184-ci-verify-symlink-fixture.md"
    - "docs/designs/issue-184-ci-verify-evidence-canonical-temp-fixture.md"
    - "docs/plans/issue-184-ci-verify-evidence-canonical-temp-fixture.md"
  derived-by: []
  verifies:
    - "docs/specs/issue-184-ci-verify-symlink-fixture.md"
    - "docs/designs/issue-184-ci-verify-evidence-canonical-temp-fixture.md"
  verified-by: []
  references:
    - "crates/synvelis-library/tests/common/mod.rs"
    - "crates/synvelis-library/tests/local.rs"
  defers: []
  deferred-by: []
---

# Issue #184 canonical temp fixture 検証仕様

## Purpose

`TempTree` が canonical spelling のルートを返す契約を pin する。
`fs::canonicalize` を外す・迂回する変更が入った場合に、非 canonical
な temp 綴り環境で symlink 契約テストが再び `EscapesRoot` で壊れる
ことを検出できるようにする。本 spec は fixture 契約の回帰防止であり、
個別テストケースの網羅ではない。

## Feature

TempTree canonical spelling — `synvelis-library` のテストが作る
absolute symlink target は、ruviform `ruviform-io-file` の canonical
component 契約と一致する綴りで生成される。

## Rules

- Rule: `TempTree::new` が保持するルートは `fs::canonicalize` 相当の
  canonical spelling である。`std::env::temp_dir()` の綴りをそのまま
  保持してはならない。
- Rule: fixture が canonical root を返す限り、root 内を指す absolute
  symlink は `WithinRoot` resolve で辿れ、root 外・dangling・loop の
  各契約がプラットフォーム非依存に成立する。
- Rule: 非 canonical 綴りの link target が `EscapesRoot` になるのは
  ruviform の文書化済み契約であり、synvelis 側で alias 正規化の
  workaround を追加してはならない。

## Examples

- Given `TEMP`/`TMP` が junction 経由の非 canonical 綴りを指す環境
  When `cargo test -p synvelis-library --test local` を実行する
  Then `stat_reports_link_kind_not_target_kind`・
  `symlink_into_root_is_browsable_and_marked`・
  `symlink_loop_does_not_recurse_forever`・
  `dangling_symlink_is_missing_not_fatal` を含む全件が pass する。
- Given 通常環境（canonical な temp_dir）
  When `mise run test` を実行する
  Then `synvelis-library` を含む workspace 全テストが pass する。
- Given fixture が canonical 化されている
  When `ancestor_symlink_cannot_leak_out_of_locator_scope` /
  `symlink_escaping_root_is_listed_but_never_opened` を実行する
  Then 「真の逸脱」（祖先・root 外）による Unavailable が維持され、
  綴り違い由来の偶然の `EscapesRoot` に依存しない。

## Guarantees

- `TempTree::path()`/`join()`/`write()`/`mkdir()` が返すパスは全て
  canonical root から導出され、テストが作るあらゆる absolute path
  （symlink target を含む）が canonical 綴りになる。
- `Drop` の `remove_dir_all` は canonical root に対して同一
  ディレクトリを削除する。
- 新規の外部依存を必要としない。

## Non-goals

- 非 canonical 綴りの absolute link target を受理する ruviform 側の
  挙動変更（out-of-contract、別途 ruviform 検討事項）。
- `synvelis-library` 以外のクレートの fixture 同等修正
  （同パターンが問題化した場合は別 Issue）。
- 各 symlink テストケース自体の追加・変更（既存テストが契約を
  そのまま検証する）。

## Risk

canonicalize 呼び出しを除去する退行が入ると、macOS `/var`・
Windows 8.3 名・junction 環境でのみ再発する CI 限定の失敗となり、
ubuntu/self-hosted lane では検出できない。上記 Examples の
非 canonical TEMP 再現が唯一のローカル検出手段である。
