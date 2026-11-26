---
id: "PLAN-e8bHN4ZJCCap49Tw896dG3"
type: "plan"
status: "approved"
title: "CTL-02 canonical persistence plan"
created: "2026-11-27"
updated: "2026-11-27"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/122"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/ctl-02-canonical-persistence.md"
    - "docs/designs/ctl-02-canonical-persistence.md"
  implemented-by:
    - "docs/tasks/ctl-02-canonical-persistence.md"
    - "docs/impl/ctl-02-canonical-persistence.md"
  depends-on:
    - "docs/designs/ctl-02-canonical-persistence.md"
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

# CTL-02 canonical persistence plan

1. `control.rs` に canonical deviation oracle と migration normalization
   helper を追加する。
2. `migration.rs` に v15→v16 step を追加し、`store.rs` の schema version
   と version history を v16 へ更新する。
3. `project.rs` の `validate_references` に canonical oracle を追加し、
   error message に target id/owner/role/deviated fields を含める。
4. `ctl_02.rs` に v15 normalization fixture、v16 fail-closed fixture、
   fresh/v9 oracle pass fixture を追加する。
5. `cb01_variant.rs`、`tim_02_smf.rs`、`md_a.rs` の schema version 固定
   期待値を v16 に合わせる。
6. package scoped fmt を適用した後、指定された canonical verification lane
   を実行する。失敗は scope 内/環境起因を区別して記録する。
7. `evidence/sv122/` に command、OS、対象 SHA、exit code、fixture 観測を
   記録し、implementation record と PR 本文へ反映する。
8. 最新 main を取り込んで最終検証し、branch push と PR 作成を行う。
   `docs/**/README.md` は更新せず、Coordinator 統合時整備に残す。

## 検証計画

正規コマンドは mise task。直接 cargo は補助証拠扱い。package 引数は
`synvelis-project`。`fmt-check` は workspace-wide lane として実行するが、
Windows `os error 206` の既知制約で起動不能な場合は環境 failure として
記録し、package scoped fmt を補助証拠に留める。
