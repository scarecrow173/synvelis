---
id: "TSPEC-TDeaZLlpVBAaTB6kuxZzZ"
type: "test-spec"
status: "approved"
title: "VCS-06 evidence completion tests"
created: "2026-11-11"
updated: "2026-11-11"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/97"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on:
    - "DESIGN-TR6hK0f243aEk3CmFQlfp"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "SPEC-034VNNGsOaycnOBjCAH3m1"
    - "SPEC-034V2wnQPWFJdKz68KlwJG"
  refines: []
  refined-by: []
  derives-from:
    - "SPEC-034VNNGsOaycnOBjCAH3m1"
  derived-by: []
  verifies:
    - "SPEC-034VNNGsOaycnOBjCAH3m1"
  verified-by: []
  references:
    - "crates/synvelis-project/tests/vcs06_plugin_instance_matrix.rs"
    - "evidence/reports/vcs-06-compat-evidence.md"
  defers: []
  deferred-by: []
---

# VCS-06 evidence completion tests

## Feature: VCS-RESTORE-08 — plugin_instances verbatim restore-as-copy

### Rule RESTORE-08-R1: fork は plugin_instances を verbatim 保持する

`restore_as_copy` が publish する `SnapshotEnvelope` の
`project.plugin_instances` は、source project のものと
`PartialEq` で一致しなければならない。`watermark`・全 entry の
`PluginInstanceId`・pin された `PluginRef` が不変であること。

#### Example

resolved processor 2 件（`proc-a`/`proc-b`、異なる `PluginRef`）
+ unresolved processor 1 件の project を構築し、identity を
発行（watermark=3、id=1,2）した上で `restore_as_copy` を実行する。
readback した envelope で `plugin_instances` が source と一致
し、fork `project_id` のみが新規採番されることを確認する。

### Rule RESTORE-08-R2: unresolved processor は identity を持たない

identity 未発行の processor は `plugin_instances.instances` に
entry を持たず、verbatim clone 後もその状態が保たれること
（negative control）。

#### Example

`proc-unresolved` が `identities.instances` に存在しないことを
pre-condition で assert し、restore 後も entry が増えていない
ことを `PartialEq` 一致で担保する。

### Rule RESTORE-08-R3: 保持された identity は fork で resolve 可能

clone された `plugin_instances` の entry は fork project の
`PluginRef` に対して `resolve()` で機能し、id の再採番・衝突が
起きないこと。

#### Example

fork の `plugin_instances` から proc-a の identity を引き、
fork の processor の `PluginRef` と一致することを確認する。

## Feature: VCS-COMPAT-03/04 — CI leg 実測記録

### Rule COMPAT-R1: 実測値のみ記録する

`evidence/reports/vcs-06-compat-evidence.md` は `gh pr checks`・
`gh run list`・`gh run view` で実際に取得した run URL・
head_sha・leg outcome のみを記録する。推測値・将来の再実行
結果を含めない。

#### Example

PR #268/#306/#323 について、ci.yml の `workflow_dispatch`
専用であるため PR check が存在しないこと、および merge commit
を含む main 向け run の URL と leg outcome を記録する。

### Rule COMPAT-R2: required/observed を区別する

`tools/ci-summary.mjs` の定義に従い、`verify (ubuntu-latest)`・
`verify (windows-latest)` を required、`verify-macos` を
observed（pr-mode）/required（full-mode）として区別して記録
する。

#### Example

run 42938180676 では ubuntu verify=success、windows verify=
failure（webdav TransientIo、VCS-06 非関連）、verify-macos=
success と leg 単位で記録し、overall は `failure` と正直に記す。

### Rule COMPAT-R3: CI leg を実機受入に換算しない

CI leg の pass/fail は workflow 実行の証拠であり、実 OS
durability・plugin host・consumer・RT の受入を意味しない。
#83 Close に伴い COMPAT-03 の錨が現行 ci.yml verify/full
lane である旨を明記する。

#### Example

レポートに「CI 実行 ≠ 実機受入」「#83 は Closed、現行の錨は
ci.yml verify/full lane」を明示する。
