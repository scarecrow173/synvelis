---
id: "SPEC-034U2u4gD9peI0yjoqllEA"
type: "spec"
status: "proposed"
title: "Issue #184 CI verify/evidence 失敗 — synvelis-library テスト fixture の canonical path 修正"
created: "2026-11-08"
updated: "2026-11-08"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/184"
    - "https://github.com/scarecrow173/synvelis/actions/runs/42431461645"
  changes:
    added:
      - type: "file"
        path: "docs/specs/issue-184-ci-verify-symlink-fixture.md"
      - type: "file"
        path: "docs/discovery/issue-184-ci-verify-symlink-fixture.md"
      - type: "file"
        path: "docs/designs/issue-184-ci-verify-evidence-canonical-temp-fixture.md"
      - type: "file"
        path: "docs/plans/issue-184-ci-verify-evidence-canonical-temp-fixture.md"
      - type: "file"
        path: "docs/tasks/canonical-temp-fixture-synvelis-library-symlink.md"
      - type: "file"
        path: "docs/test-specs/issue-184-canonical-temp-fixture.md"
    modified:
      - type: "file"
        path: "crates/synvelis-library/tests/common/mod.rs"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by:
    - "docs/designs/issue-184-ci-verify-evidence-canonical-temp-fixture.md"
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/alib-01-asset-provider-contract.md"
    - "docs/adr/alib-01-synvelis-library-crate-boundary.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/discovery/issue-184-ci-verify-symlink-fixture.md"
  derived-by:
    - "docs/designs/issue-184-ci-verify-evidence-canonical-temp-fixture.md"
    - "docs/plans/issue-184-ci-verify-evidence-canonical-temp-fixture.md"
    - "docs/test-specs/issue-184-canonical-temp-fixture.md"
    - "docs/tasks/canonical-temp-fixture-synvelis-library-symlink.md"
  verifies: []
  verified-by:
    - "docs/test-specs/issue-184-canonical-temp-fixture.md"
  references:
    - "crates/synvelis-library/tests/common/mod.rs"
    - "crates/synvelis-library/tests/local.rs"
    - ".github/workflows/ci.yml"
  defers: []
  deferred-by: []
---

# Issue #184 CI verify/evidence 失敗 — synvelis-library テスト fixture の canonical path 修正

## What Should Be Built

`synvelis-library` の統合テストが使う `TempTree` fixture が、全プラットフォームで
canonical spelling のルートパスを返すようにする。具体的には
`crates/synvelis-library/tests/common/mod.rs` の `TempTree::new` が、
`std::env::temp_dir()` 由来の非 canonical なパス綴り
（macOS の `/var` → `/private/var`、Windows の 8.3 短縮名・junction・
`\\?\` verbatim 差異）を `fs::canonicalize` で正規化してから保持する。

## Why Now

main `8c11e311`（PR #180, ALIB-01 `synvelis-library` 新設）で導入された
`LocalFolderProvider` の symlink テスト 4 件が、GitHub-hosted
`verify-macos` と `verify (windows-latest)` でのみ失敗し、fail-closed の
`evidence` / `ci-summary` を連鎖的に failure にしている（run 42431461645）。
ubuntu lane と self-hosted `gw10-*` lane は success で、main の Full CI が
赤いままでは後続作業の検証信号が得られない。

## Users and Value

- Synvelis 開発者・CI 運用者: main の Full CI が再び緑になり、
  各 lane の合否が実際の回帰を反映する状態に戻る。
- `synvelis-library` の契約テスト: 「root 内を指す absolute symlink は
  browsable」が全プラットフォームで同じ契約として検証される。

## Scope

### In Scope

- `crates/synvelis-library/tests/common/mod.rs` の `TempTree` が返す
  ルートパスを canonical spelling に揃える修正と、その根拠を示す
  コメント。
- 修正が対象とする失敗の再現・検証（非 canonical な temp 綴りを
  模した環境でのローカル再現を含む）。

### Out of Scope

- ruviform `ruviform-io-file` rooted resolve 層の契約変更
  （canonical spelling 強制は文書化済みの意図的な保守的動作であり、
  本タスクでは変更しない。緩和が必要なら別途 ruviform 側の検討事項）。
- `synvelis-library` 以外のクレートのテスト helper や、
  プロダクトコード（`src/`）の変更。
- `.github/workflows/` の lane 構成変更。

## Requirements

- `TempTree::new` が返す `path()` は、そのディレクトリの canonical
  spelling であること（`fs::canonicalize` 相当。Windows では
  `\\?\` verbatim 形を含みうるが、std::fs 系 API と ruviform の
  canonical root 比較の双方で一貫して扱える形）。
- 既存テストの意味を変えないこと。canonical 化によって
  「root 内 symlink は辿れる」「root 外 symlink は Unavailable」
  「dangling symlink は Missing」等の検証意図が全プラットフォームで
  同一に保たれること。
- 新規の外部依存（`tempfile` / `dunce` 等）を追加しないこと。

## Acceptance Criteria

- [ ] `TEMP`/`TMP` が非 canonical な綴り（junction・8.3 名相当）を
  指す環境で `cargo test -p synvelis-library --test local` が
  4 件の symlink テストを含め全件 pass する。
- [ ] 通常環境（canonical な temp_dir）でも `mise run test` が
  pass する。
- [ ] `ancestor_symlink_cannot_leak_out_of_locator_scope` 等、
  「外へ出る link は Unavailable」の否定系テストが引き続き pass し、
  canonical spelling 化で検証経路が「綴り違い由来の偶然の
  EscapesRoot」から「真の祖先逸脱」へ改善される。
- [ ] PR の CI で `verify-macos`・`verify (windows-latest)`・
  `evidence`・`ci-summary` が pass する見通しが得られる
  （CI 限定 lane のため PR 上の実測で確認）。

## Deferred Design Concerns

<!-- Intentionally deferred future work. Link the deferred draft doc via relations.defers. -->

- ruviform `ruviform-io-file` の absolute link target 解決は
  canonical component 列との textual prefix 比較のみで、
  同一ディレクトリの非 canonical alias 綴り（bind mount・`subst`・
  symlinked ancestor 経由）を `EscapesRoot` として保守的に拒否する。
  実ユーザー環境で「root が symlinked prefix 配下にあり、内部 link が
  同じ非 canonical 綴りを指す」ケースでは false positive になりうるが、
  ruviform 側はこれを out-of-contract と文書化している。
  再エンゲージ条件: 実運用で当該 false positive が観測された場合、
  または ruviform 側で alias 正規化の設計検討が起きた場合。
  無視した場合のリスク: macOS で `/tmp`・`/var` 配下を root にした
  provider が内部 absolute symlink を Unavailable と報告し続ける。

## Sources and Relations

- Issue #184（CI failure tracker、managed コメントは編集しない）。
- 失敗 run: <https://github.com/scarecrow173/synvelis/actions/runs/42431461645>
  — `verify-macos` / `verify (windows-latest)` の
  `Test workspace (mise run test)` step で
  `crates/synvelis-library/tests/local.rs` の 4 テストが FAILED、
  `evidence` / `ci-summary` は fail-closed gate として連鎖 failure。
- 根因: `TempTree` が `std::env::temp_dir()` の綴りをそのまま使い、
  macOS（`/var`→`/private/var`）と Windows（8.3 短縮名等）で
  canonical root と異なる綴りの absolute symlink target を作る。
  ruviform `ruviform-io-file` の `WithinRoot` resolve は absolute
  target を canonical component 列と textual 比較し、綴り違いを
  `EscapesRoot` とする（`canonical_root()` の doc comment に
  out-of-contract と明記）。ローカルでは `TEMP` を junction 経由の
  非 canonical 綴りに差し替えて同一 4 件の失敗を再現済み。
