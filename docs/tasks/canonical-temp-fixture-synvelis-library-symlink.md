---
id: "TASK-034U2vruJGfkEYZHCqNfwE"
type: "task"
status: "in-progress"
title: "canonical temp fixture による synvelis-library symlink テスト修正"
created: "2026-11-08"
updated: "2026-11-08"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/184"
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
    - "docs/plans/issue-184-ci-verify-evidence-canonical-temp-fixture.md"
  implemented-by: []
  depends-on:
    - "docs/plans/issue-184-ci-verify-evidence-canonical-temp-fixture.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/issue-184-ci-verify-symlink-fixture.md"
    - "docs/designs/issue-184-ci-verify-evidence-canonical-temp-fixture.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/issue-184-ci-verify-symlink-fixture.md"
    - "docs/designs/issue-184-ci-verify-evidence-canonical-temp-fixture.md"
    - "docs/plans/issue-184-ci-verify-evidence-canonical-temp-fixture.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/issue-184-canonical-temp-fixture.md"
  references:
    - "crates/synvelis-library/tests/common/mod.rs"
    - "crates/synvelis-library/tests/local.rs"
  defers: []
  deferred-by: []
---

# canonical temp fixture による synvelis-library symlink テスト修正

## Work

`crates/synvelis-library/tests/common/mod.rs` の `TempTree::new` で、
`create_dir_all` 後に `fs::canonicalize` したパスを保持するよう変更
する。`std::env::temp_dir()` の非 canonical 綴り（macOS `/var`、
Windows 8.3 名・junction）を吸収し、テストが作る absolute symlink
target が ruviform `ruviform-io-file` の canonical component 契約と
一致する綴りになるようにする。変更には根拠を示すコメントを付ける。
新規依存は追加しない。

## Classification

- `normal-plan-task`

## Upstream Documents

- `docs/plans/issue-184-ci-verify-evidence-canonical-temp-fixture.md`
- `docs/specs/issue-184-ci-verify-symlink-fixture.md`
- `docs/designs/issue-184-ci-verify-evidence-canonical-temp-fixture.md`

## Dependencies

- なし（ブロッカーなし）

## Affected Files

- `crates/synvelis-library/tests/common/mod.rs` — `TempTree::new` の
  ルート保持値を canonical spelling に変更
- `docs/specs|designs|plans|tasks/issue-184-*` — 本タスクの文書チェーン

## Done When

- [ ] `TempTree::new` が canonical spelling のルートを返す
- [ ] 非 canonical temp 綴り環境で `--test local` が全件 pass
- [ ] `mise run test` 他の canonical 検証タスクが pass

## Verification

- [ ] `TEMP`/`TMP` を junction 経由の非 canonical 綴りにして
  `cargo test -p synvelis-library --test local` → 全件 pass
  （修正前の同一 4 件 FAILED は対照実験として記録済み）
- [ ] `mise run fmt` / `fmt-check` / `check` / `lint` / `test` /
  `build` が全て pass
- [ ] review-protocol によるレビューで blocker が残っていない
