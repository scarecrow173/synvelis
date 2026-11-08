---
id: "DESIGN-034U2uVeVKqECLpCn8kbSa"
type: "design"
status: "approved"
title: "Issue #184 CI verify/evidence 失敗 — canonical temp fixture 設計"
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
  implements:
    - "docs/specs/issue-184-ci-verify-symlink-fixture.md"
  implemented-by:
    - "docs/plans/issue-184-ci-verify-evidence-canonical-temp-fixture.md"
  depends-on:
    - "docs/specs/issue-184-ci-verify-symlink-fixture.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/adr/alib-01-synvelis-library-crate-boundary.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/issue-184-ci-verify-symlink-fixture.md"
  derived-by:
    - "docs/plans/issue-184-ci-verify-evidence-canonical-temp-fixture.md"
    - "docs/test-specs/issue-184-canonical-temp-fixture.md"
    - "docs/tasks/canonical-temp-fixture-synvelis-library-symlink.md"
  verifies: []
  verified-by:
    - "docs/test-specs/issue-184-canonical-temp-fixture.md"
  references:
    - "crates/synvelis-library/tests/common/mod.rs"
    - "crates/synvelis-library/tests/local.rs"
  defers: []
  deferred-by: []
---

# Issue #184 CI verify/evidence 失敗 — canonical temp fixture 設計

## Context

`LocalFolderProvider`（PR #180 で新設）は全ファイルアクセスを
ruviform `ruviform-io-file` の `RootedDir` capability 経由で行う。
`WithinRoot` resolve は absolute symlink target を **canonical root
component 列との textual prefix 比較**で判定し、綴りの異なる同一
ディレクトリ（alias）を `EscapesRoot` として保守的に拒否する
（`RootedDir::canonical_root()` の doc comment が "non-canonical
alias — out-of-contract" と明記）。

一方 `synvelis-library` の `TempTree` fixture は
`std::env::temp_dir()` の綴りをそのままルートに使う。temp_dir の
綴りはプラットフォームで canonical とは限らない:

- macOS: `$TMPDIR` = `/var/folders/…` だが `/var` は `/private/var`
  への symlink。canonical root は `/private/var/folders/…`。
- Windows: GitHub-hosted runner の TEMP は 8.3 短縮名
  （`C:\Users\RUNNER~1\…`）になりうる。canonical root は長い実名。
- いずれも junction・bind mount・`subst` でも同じクラスの差異が起きる。

テストが `tree.join("sub")` 等で作る absolute symlink target は
非 canonical 綴りのままなので、root 内を指していても resolve 層が
`EscapesRoot` と判定し、4 テストが失敗する。ubuntu lane は `/tmp`
が実ディレクトリで canonical 綴りと一致するため pass していた。

ローカル再現（Windows）: `TEMP`/`TMP` を junction 経由の非 canonical
綴りに差し替えて `cargo test -p synvelis-library --test local` を
実行し、CI と同一の 4 件・同一の panic 箇所を確認済み。

## Scope

### In Scope

- `crates/synvelis-library/tests/common/mod.rs` の `TempTree::new`
  が保持するルートパスを canonical spelling に正規化する。

### Out of Scope

- ruviform resolve 層の契約・実装（文書化済みの意図的動作。
  実ユーザー環境の false positive が問題化した場合は ruviform 側の
  別 Issue で扱う）。
- プロダクトコード・他クレートの fixture・CI workflow。

## Components and Boundaries

- `TempTree`（tests/common/mod.rs）: 「実在する一時ディレクトリ」を
  供給する fixture。canonical spelling を返す責務をここに置くのが
  正しい境界 — symlink target を作る各テストが個別に canonicalize
  する重複を避け、将来の absolute path 利用も同じ契約で守る。
- `try_symlink`（同ファイル）: 変更しない。link 生成時に target を
  canonicalize する案は、fixture が非 canonical パスを配り続ける
  根本を残すため採用しない。

## Data and Control Flow

1. `TempTree::new(tag)` が `temp_dir()/synvelis-lib-{tag}-{pid}-{seq}`
   を作成（従来どおり）。
2. `fs::canonicalize` で canonical spelling を取得し、それを保持。
   失敗時は `expect`（作成直後の実在 dir なので実質到達不能）。
3. `path()`/`join()`/`write()`/`mkdir()` は canonical root から導出
   されるため、テストが作る absolute symlink target は常に
   canonical 綴りになり、ruviform の canonical component 比較と
   一致する。
4. `Drop` は canonical root に対して `remove_dir_all` — 同一
   ディレクトリを指すため問題ない。

Windows では `fs::canonicalize` が `\\?\` verbatim 形を返す。実測で
verbatim 形の link path / target 双方を受け付け、`read_link` は
DOS 形を返し、`is_dir`/`symlink_metadata`/`remove_dir_all` も正常に
動くことを確認済み。`provider.root()`（`GetFinalPathNameByHandleW`
由来の canonical_root）と同じ verbatim 形になるため、文字列表現の
一貫性も保たれる。verbatim→DOS 変換（dunce 相当）は追加しない —
不要な変換層を増やさない（KISS）。

## Non-Functional Constraints

- 新規依存を追加しない（`tempfile`/`dunce` 等は使わない）。
- テストの実行時間・並列性に影響しない（canonicalize は
  TempTree 生成ごとに 1 回の syscall）。

## Risks and Trade-offs

- verbatim `\\?\` 形の path が一部 API で扱いにくい可能性 →
  std::fs 系および本テスト群の使用範囲では実測済みで問題なし。
  万一の問題は PR CI で検出可能。
- canonical 化で「非 canonical 綴りの link target」ケースを
  テストしなくなる → それは ruviform の out-of-contract 動作であり、
  synvelis 側の契約テスト対象外（spec の Deferred Design Concerns
  に記録）。

## Deferred Design Concerns

- ruviform absolute link target の alias 正規化（canonicalize して
  から prefix 比較する等）は ruviform 側の設計判断。synvelis 側で
  暫定対応しない（恒久的な重複実装で隠さない、の運用契約どおり
  spec に deferred として記録）。

## Verification Notes

- [x] `TEMP`/`TMP` を junction 経由の非 canonical 綴りにして
  `cargo test -p synvelis-library --test local` → 修正前は CI と
  同一 4 件が FAILED（再現確認済み）。
- [ ] 同条件で修正後は全件 pass。
- [ ] `mise run test`（canonical temp_dir の通常環境）で全件 pass。
- [ ] `mise run fmt` / `fmt-check` / `check` / `lint` / `build`。
