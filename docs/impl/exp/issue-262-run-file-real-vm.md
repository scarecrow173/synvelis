---
id: "EXP-034YBzJ2mW8sVf4XtPc"
type: "experiment"
status: "completed"
title: "issue-262-run-file-real-vm slice の試行 — seam 解決・surface 規則・envelope 写像"
created: "2026-11-17"
updated: "2026-11-17"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/262"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by:
    - "docs/impl/ir/issue-262-run-file-real-vm.md"
  depends-on:
    - "docs/tasks/issue-262-run-file-real-vm.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/exp/issue-262-run-file-real-vm-exp.jsonl"
    - "docs/discovery/issue-262-run-file-real-vm.md"
    - "evidence/sv262/issue-262-run-file-real-vm-ledger.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/discovery/issue-262-run-file-real-vm.md"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "crates/synvelis-cli/src/script.rs"
    - "crates/synvelis-cli/src/host.rs"
    - "crates/synvelis-automation/src/surface.rs"
    - "crates/synvelis-gateway/src/headless.rs"
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# issue-262-run-file-real-vm slice の試行

Event log: `docs/impl/exp/issue-262-run-file-real-vm-exp.jsonl`
（構造化記録）。

## 試行の概要

sv273 で defer された `.luau`→plan compile 以降を、AUTX-06 の
parity 正本に沿って実配線する。焦点は (a) `HeadlessSessionHost`
の共有 handle seam を非破壊で実現する方法、(b) canonical
`ScriptSurfaceSpec` の命名規則をどこに・どう固定するか、
(c) run 終端 state を envelope/exit code へ正直に写像する境界。

## 採用した判断

- **composition-time 注入**: `HeadlessSessionHost` の field を
  `Arc` 化し getter で clone を返す。`new`/
  `without_commit_boundary` の signature 不変で既存呼出 3 箇所は
  無変更。getter 案（暗黙共有）より composition root での明示
  を優先。
- **surface 生成器は automation**: `synvelis-script` は命名無
  関知の汎用 crate、cli は #274 と lease 共有不可 → automation
  `surface.rs`。命名規則は deterministic camelCase、衝突は
  `DuplicateApiName` fail-closed（同 id 別 version も曖昧と
  して reject — version 違いが script 名を shadow するため）。
- **envelope 写像**: run state をそのまま status/exit へ。
  lane の最後の `ErrorEnvelope` を `RecordingStepLane` で保持
  し、Failed 時に gateway exit code（denied→3 等）を verbatim
  で返す。非終端は `accepted`/7 で成功を捏造しない。
- **`--inputs` は workflow のみ**: `InputDecl` は全必須・
  default 無し。action recipe は request-level fields を
  `RecipeSpec` が持つため inputs flag の対象外。

## 却下/修正した点

- **surface 衝突チェックの初版**: 同 id の別 version を「同一
  id だから非衝突」としていたが、script 名は version を区別
  できず shadow になる → 全ての再導出を `DuplicateApiName`
  に変更（unit test で固定）。
- **e2e fixture**: 旧 test は空 dir の `--project` で deferred
  経路を検証していた。実実行は `boot_project` が saved bundle
  を要求するため `fixture_project`/`project_dir` へ変更。
- **`outputs: []` flow 記法**: YAML frontend は flow collection
  非対応 → block 記法/省略へ修正。

## 残課題（外部 gate）

- 実 job step・named macro 実解決・approval→apply・durable
  journal・製品 E2E/3OS 実機は NotRun/Deferred（#267/#274/
  AUTX-04 系が所有）。
