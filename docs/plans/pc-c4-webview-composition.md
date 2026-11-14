---
id: "PLAN-034XHDyswoYugrRstusA6p"
type: "plan"
status: "approved"
title: "PC-C4 webview composition plan"
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
    - "docs/designs/pc-c4-webview-composition.md"
  implemented-by: []
  depends-on:
    - "docs/specs/pc-c4-webview-composition.md"
    - "docs/designs/pc-c4-webview-composition.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/plans/pc-c4-drain-composition-plan.md"
    - "docs/plans/pc-c4-observe-engine-wiring-plan.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/pc-c4-webview-composition.md"
    - "docs/designs/pc-c4-webview-composition.md"
  derived-by: []
  verifies:
    - "docs/test-specs/pc-c4-webview-composition.md"
  verified-by:
    - "docs/test-specs/pc-c4-webview-composition.md"
  references:
    - "docs/specs/pc-c4-webview-composition.md"
    - "docs/designs/pc-c4-webview-composition.md"
    - "docs/plans/pc-c4-drain-composition-plan.md"
  defers: []
  deferred-by: []
---

# PC-C4 webview composition plan Implementation Plan

## Goal

`synvelis`（webview）bin に native と同じ条件の routing engine
composition を配線し、`syn.control.*` op surface を非 stub 化
する。あわせて merge 済み PC-C4 slice の stale status 簿記を
行う。

## Scope

単一実装ストリーム — `synvelis-app` crate 内の helper 追加・
bin 配線・fixture・簿記で完結し、分割しない。

## Upstream Documents

- `docs/specs/pc-c4-webview-composition.md`
- `docs/designs/pc-c4-webview-composition.md`
- `docs/specs/pc-c4-drain-composition.md`（deferred 項目の親）

## Design Inputs

- `docs/designs/pc-c4-webview-composition.md`

## File Map

- `crates/synvelis-app/src/composition.rs` —
  `compose_product_routing(app, Option<PathBuf>)`（pub）と
  `product_engine_config`（private）を追加。
- `crates/synvelis-app/src/lib.rs` — `compose_product_routing`
  を re-export。
- `crates/synvelis-app/src/bin/synvelis.rs` — `SYNVELIS_PROJECT_DIR`
  env gate + `compose_product_routing` 呼出を `attach_logging`
  直後に追加。
- `crates/synvelis-app/tests/pc_c4_webview_composition.rs` —
  新規 fixture（gate 閉/開・attach・非 stub status・recreate
  detach の 3 test）。
- `docs/tasks/pc-c4-02-task1-plugin-instance-binding.md` —
  status `in-progress` → `done`（PR #250 merge 済み）。
- `docs/tasks/pc-c4-plugin-binding-task.md` — status +
  checkboxes を merge 済み事実に更新（PR #393 merge 済み）。
- `docs/impl/ir/pc-c4-observe-engine-wiring.md` — status を
  慣例確認のうえ `completed` へ（PR #452 merge 済み・evidence
  記録済み）。
- `docs/{specs,designs,plans,tasks,test-specs,impl/ir,impl/exp}/
  pc-c4-webview-composition.md` — 本 doc chain。
- `evidence/sv101/` — 検証記録（新規ファイル）。
- `docs/*/README.md` — doc-maintenance `index apply` の
  再生成物（手編集しない）。

## Implementation Sequence

- [x] T1: `composition.rs` に `compose_product_routing` +
  `product_engine_config` を追加し `lib.rs` で re-export。
- [x] T2: `bin/synvelis.rs` に env gate + 呼出を配線。
- [x] T3: `tests/pc_c4_webview_composition.rs` を作成し
  fixture 3 test を green にする。
- [ ] T4: stale status 簿記（task 2 件 + IR 1 件）。
- [ ] T5: canonical 検証 lane 実行 + evidence 記録 + docs
  index 再生成。
- [ ] T6: review-protocol 1 cycle → commit → push → PR。

## Dependencies and Risks

- `synvelis-native.rs` は read-only — 変更しない。helper の
  native 移行は deferred（design の Deferred Design Concerns）。
- `webview` feature gate: helper は feature 非依存 lib、bin
  呼出は `webview` feature の `synvelis.rs` のみ — headless
  build を壊さないことを `check`/`check-app-webview` で確認。
- 空 project での publish: `SetProjectName` revision で
  generation 1 の surface が attach されることを fixture で
  実測済み（`cargo test --test pc_c4_webview_composition`
  補助証拠、3/3 pass）。

## Task Breakdown

- task1（本 plan に 1:1）: `docs/tasks/pc-c4-webview-composition.md`

## Verification

- [ ] `mise run fmt-check` → exit 0
- [ ] `mise run check synvelis-app` → exit 0
- [ ] `mise run lint synvelis-app` → exit 0
- [ ] `mise run test synvelis-app` → exit 0（新規 3 test 含む）
- [ ] `mise run check-app-webview` / `lint-app-webview` /
  `test-app-webview` → exit 0
- [ ] `mise exec -- prek run --all-files` → exit 0
- [ ] `git diff` で `synvelis-native.rs`・範囲外ファイルの
  無変更を確認
- [ ] NotRun: `*-msrv`/`*-full`/`test-rt`（player 変更なし・
  Daily Full CI 委譲）、実 plugin factory（ruviform#253）・
  実 GUI（#114）・wire-schema L1・3OS — evidence に記録

## Open Questions / Gaps

- なし — gate 形状（`Option<PathBuf>`）と helper 配置は
  design で確定済み（exp EXP-1 に代替案の却下根拠）。

## Self-Review

- [x] Every spec requirement maps to a task.
- [x] No placeholders remain.
- [x] File paths, relations, status values, and terminology are consistent.

## Implementation Handoff

- Recommended mode: inline execution（Orca worker として
  dispatch 済み）
- Independent work streams: none
- User approval required before dispatch: no（coordinator
  dispatch が承認済み）
- If approved: discover available implementation and delegation capabilities in the current environment, then execute task-by-task with review checkpoints.
- If declined or unavailable: execute inline using the same dependency graph and verification matrix.

## Review Handoff

- reviewer focus: `compose_product_routing` の `Option<PathBuf>`
  契約が native の env gate 意味と一致するか、`synvelis.rs` の
  呼出位置（`attach_logging` 後・`NativeShell` 前）が正しいか、
  fixture が「webview 経路の composition」を実際に pin しているか
  （bin の env read 自体は process boundary のため compile +
  review で担保）。
