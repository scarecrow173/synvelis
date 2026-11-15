---
id: "TASK-034XHDJqlIjGWpY91oE0nE"
type: "task"
status: "done"
title: "PC-C4 webview composition task1 — synvelis.rs engine attach + stale status 簿記"
created: "2026-11-15"
updated: "2026-11-15"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/101"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/pc-c4-webview-composition.md"
  implemented-by: []
  depends-on:
    - "docs/plans/pc-c4-webview-composition.md"
    - "docs/specs/pc-c4-webview-composition.md"
    - "docs/designs/pc-c4-webview-composition.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/plans/pc-c4-webview-composition.md"
  derived-by: []
  verifies:
    - "docs/test-specs/pc-c4-webview-composition.md"
  verified-by:
    - "docs/test-specs/pc-c4-webview-composition.md"
  references: []
  defers: []
  deferred-by: []
---

# PC-C4 webview composition task1 — synvelis.rs engine attach + stale status 簿記

## Work

- T1 — `composition.rs`: `compose_product_routing(app,
  Option<PathBuf>)`（pub）+ `product_engine_config`（private）
  追加、`lib.rs` re-export。
- T2 — `bin/synvelis.rs`: `SYNVELIS_PROJECT_DIR` env gate +
  `compose_product_routing` 呼出（`attach_logging` 直後）。
- T3 — `tests/pc_c4_webview_composition.rs`: gate 閉/開・
  attach・非 stub `syn.control.status`・recreate detach の
  3 test。
- T4 — 簿記: `pc-c4-02-task1-plugin-instance-binding`・
  `pc-c4-plugin-binding-task` の status 更新、IR
  `pc-c4-observe-engine-wiring` の status 慣例確認・更新。
- T5 — canonical 検証 lane + `evidence/sv101/` 記録 + docs
  index 再生成。
- T6 — review-protocol 1 cycle → commit → push → PR
  （`Refs #101`）。

## Classification

- feature / synvelis-app composition slice（Issue #101
  webview-bin engine composition + bookkeeping）

## Upstream Documents

- plan: `docs/plans/pc-c4-webview-composition.md`
- spec: `docs/specs/pc-c4-webview-composition.md`
- design: `docs/designs/pc-c4-webview-composition.md`

## Dependencies

- drain slice の `compose_routing_engine`/`SessionEngineConfig`
  （merge 済み）に依存。native bin は read-only 参照。

## Affected Files

- `crates/synvelis-app/src/composition.rs`
- `crates/synvelis-app/src/lib.rs`
- `crates/synvelis-app/src/bin/synvelis.rs`
- `crates/synvelis-app/tests/pc_c4_webview_composition.rs`（新規）
- `docs/tasks/pc-c4-02-task1-plugin-instance-binding.md`
- `docs/tasks/pc-c4-plugin-binding-task.md`
- `docs/impl/ir/pc-c4-observe-engine-wiring.md`
- `docs/{specs,designs,plans,tasks,test-specs,impl/ir,impl/exp}/
  pc-c4-webview-composition.md`（新規 7 件）
- `evidence/sv101/`（新規ファイル）
- `docs/*/README.md`（index 再生成物）

## Done When

- [x] webview bin が `SYNVELIS_PROJECT_DIR` 存在時に engine を
  attach し `syn.control.*` が非 stub evidence を返す
- [x] `composition.rs` は helper 追加のみ、`synvelis-native.rs`
  無変更
- [x] fixture 3 test green
- [x] 簿記 3 件更新
- [x] canonical 検証 lane exit 0
- [x] doc chain + index + evidence 完成

## Verification

- [ ] `mise run fmt-check` / `check synvelis-app` /
  `lint synvelis-app` / `test synvelis-app` /
  `check-app-webview` / `lint-app-webview` /
  `test-app-webview` / `prek run --all-files` — 結果は
  `evidence/sv101/` と PR 本文に記録
