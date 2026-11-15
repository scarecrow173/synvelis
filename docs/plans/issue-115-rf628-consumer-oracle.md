---
id: "PLAN-xBa7qG97w6tfXvQfyWEYkM"
type: "plan"
status: "approved"
title: "Issue #115 rf#628 consumer oracle Plan"
created: "2026-11-15"
updated: "2026-11-15"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/115"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/issue-115-rf628-consumer-oracle.md"
    - "docs/designs/issue-115-rf628-consumer-oracle.md"
  implemented-by:
    - "docs/tasks/issue-115-rf628-consumer-oracle.md"
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/plans/rcd-01-virtual-l1-oracle.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/issue-115-rf628-consumer-oracle.md"
    - "docs/designs/issue-115-rf628-consumer-oracle.md"
  derived-by: []
  verifies:
    - "docs/test-specs/issue-115-rf628-consumer-oracle.md"
  verified-by:
    - "docs/test-specs/issue-115-rf628-consumer-oracle.md"
  references:
    - "https://github.com/scarecrow173/synvelis/issues/115"
  defers: []
  deferred-by: []
---

# Issue #115 rf#628 consumer oracle Plan

## Scope split

単一 slice（`rf#628-fixture` lane の consumer oracle 実走）。Issue #115
の残り gate（rf#629 仮想 endpoint EXT-L3・physical・GUI Option B・
macOS/Linux L1 CI）は本 slice 対象外で、Issue は open 継続。

## File map

| File | 変更 | 内容 |
| --- | --- | --- |
| `Cargo.toml` | modified | `[workspace.dependencies]` に `ruviform-audio-test` 1 行追加（`rev` なし） |
| `Cargo.lock` | generated | `cargo update -p ruviform-*` で 28 package を `abb9f372` に再解決 |
| `crates/synvelis-recording/Cargo.toml` | modified | `[dev-dependencies]` に `ruviform-audio-test.workspace = true` |
| `crates/synvelis-recording/tests/rf628_oracle.rs` | added | SV-1〜SV-5 consumer oracle（9 test） |
| `crates/synvelis-recording/tests/rf628_oracle_rt_alloc.rs` | added | SV-6 RT no-alloc 計測（1 test・`rt_` prefix） |
| `evidence/sv115/rcd-01-virtual-l1-oracle-case-matrix.md` | modified | `rf#628-fixture` 列を実測 verdict で更新 |
| `evidence/sv115/rcd-01-virtual-l1-oracle-ledger.md` | modified | rf#628 行を採用 SHA・結果へ整合 |
| `evidence/sv115/` | added | 本 slice の検証記録 |
| `docs/{specs,designs,plans,tasks,test-specs,impl/ir,impl/exp}/issue-115-rf628-consumer-oracle.md` | added | doc chain（slug `issue-115-rf628-consumer-oracle`） |
| `docs/*/README.md` | generated | `doc-maintenance` index 再生成（手編集禁止） |
| `ui/vendor/ruviform/` | generated | `mise run sync-ui-deps` 再生成（gitignore・コミット対象外） |

## Dependency graph

```
pin bump (Cargo.toml/Cargo.lock) → dep 追加 → sync-ui-deps/check-ruviform-rev
  → tests/rf628_oracle*.rs → mise 正規検証 → evidence 更新
  → doc chain → review-protocol → commit/push/PR
```

## Risk register

| Risk | 対応 |
| --- | --- |
| pin bump が他 crate の compile/test を壊す | `mise run check/lint/test-rt` で検出。範囲外の破壊は修正せず API diff・症状を記録して報告 |
| `frames`/`position`/`gap_frames` の期待構成ミス | `PumpReport` 実測値と journal prefix を双方向に照合（report 側断言 + media oracle） |
| RT 計測への他テスト混入 | 別 binary・単一テスト・warm-up 済み `Region`（`input_adapter_rt_alloc.rs` 先例） |
| `check-headless-deps` が新規推移的依存で赤 | `ruviform-audio-test` の依存は全て既存 ruviform workspace member（dev-dep の `assert_no_alloc` は非伝播）。赤なら正直に記録 |

## Rollback strategy

新規 test file + doc/evidence + manifest/lock の pin bump。問題時は
pin bump commit と test commit を個別 revert 可能（論理単位で分割）。

## Verification matrix

| 検証 | 種別 | 期待 |
| --- | --- | --- |
| `cargo test -p synvelis-recording --test rf628_oracle --test rf628_oracle_rt_alloc` | 補助証拠 | 10 test pass |
| `mise run fmt-check` | 正規 | pass |
| `mise run check synvelis-recording` | 正規 | pass |
| `mise run lint synvelis-recording` | 正規 | pass（-D warnings） |
| `mise run test synvelis-recording` | 正規 | pass（非回帰含む・新規 test 名で確認） |
| `mise run test-rt` | 正規 | pass（`rt_scripted_input_callback_is_alloc_and_drop_free` 収集確認） |
| `mise run check-headless-deps` | 正規 | pass（GUI/GPU/WebView 非混入） |
| `mise run check-ruviform-rev` + `node tools/check-ruviform-rev.mjs --require-vendor` | 正規 | `abb9f372` 単一 SHA・REV 一致 |
| `mise exec -- prek run --all-files` | 正規 | pass |
| `test-recording-real-input`/`test-msrv`/`*-full` | — | 実行しない（opt-in/MSRV なし/Full CI 管轄）→ NotRun 記録 |
