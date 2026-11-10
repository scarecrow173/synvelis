---
id: "IMPL-AnRU3dvHgi5XEuxJxubame"
type: "impl"
status: "completed"
title: "PC-C4-02 plugin instance binding — stage-1 authority consumption・plugin identity pin"
created: "2026-11-10"
updated: "2026-11-10"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/101"
  changes:
    added:
      - type: "file"
        path: "docs/specs/pc-c4-02-plugin-instance-binding.md"
      - type: "file"
        path: "docs/designs/pc-c4-02-plugin-instance-binding-design.md"
      - type: "file"
        path: "docs/plans/pc-c4-02-plugin-instance-binding.md"
      - type: "file"
        path: "docs/tasks/pc-c4-02-task1-plugin-instance-binding.md"
      - type: "file"
        path: "docs/test-specs/pc-c4-02-e2e-plugin-instance-binding.md"
    modified:
      - type: "file"
        path: "crates/synvelis-project/src/host_param.rs"
      - type: "file"
        path: "crates/synvelis-project/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-player/src/binding.rs"
      - type: "file"
        path: "crates/synvelis-player/src/host.rs"
      - type: "file"
        path: "crates/synvelis-player/src/routing_control.rs"
      - type: "file"
        path: "crates/synvelis-player/src/routing_engine.rs"
      - type: "file"
        path: "crates/synvelis-player/tests/pc_c4_binding.rs"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/tasks/pc-c4-02-task1-plugin-instance-binding.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies:
    - "docs/test-specs/pc-c4-02-e2e-plugin-instance-binding.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# PC-C4-02 plugin instance binding — implementation record

## What landed

Issue #101 第二 slice。第一 slice（PR #221）の binding 層を #122
（CTL-02）の stage-1 semantic authority へ接続し、plugin instance
の authored identity を connect-time 証拠として pin した。

- `synvelis-project::host_param::plugin_instance_binding` —
  `ProcessorId → &PluginRef` の canonical lookup（`NotFound`/
  `Validation` fail-closed）。`semantic_plugin_binding` と対になる
  processor-level 正本。
- `ProcessorControl.plugin: Option<PluginRef>` — engine が
  `stage_build` で `ProcessorConnectRequest.processor.binding` の
  connect-time `Resolved` を `RetainedConnection` に捕捉し、
  `take_processor_control` で一度だけ vend。live project の現在値
  ではなく instance 自身の identity 証拠。
- `PluginBindingSession::attach` が `plugin: PluginRef` を必須 pin
  化。`PluginParameterBinding.plugin` に記録。
- `bind` が `semantic_plugin_binding` を必ず解決し、
  `semantic.plugin != session.plugin` を `BindingError::PluginMismatch`
  で拒否。`ParamId` は semantic binding 由来のみ。
- `dispatch` が record の `plugin` pin を record 比較前に検査
  （`PluginMismatch`）。
- `RoutingControlDispatch::vend_nodes` は `control.plugin: None` を
  `without_control` evidence として fail-closed 処理。

## Key decisions

- **identity の情報源は connect-time binding に限定**: live project
  を attach 時に読むと、rebind 済み project の主張を旧 generation
  の instance へ誤pin する経路が残る。instance 真実（connect 時
  `ProcessorBinding`）と現在の主張（semantic binding）の一致だけが
  bind を許す設計で、wrong-plugin delivery を構造的に排除。
- **last-good 維持**: project rebind は旧 generation surface の
  既存 binding を無効化しない（その instance に対して pin は正しい
  まま）。新規 bind のみ `PluginMismatch` で止まる。世代交代は
  routing engine の generation 機構が担う。
- **`PluginRef` 全体比較**: `capability_revision` 衝突下の
  descriptor/version 差分も `PluginMismatch` で捕捉
  （`port_capability_generation` が拾えない drift を identity pin
  が補完）。

## Verification

- `mise run check` / `lint` / `test`（`CARGO_BUILD_JOBS=4` —
  page-file 対策の env のみ、コマンドは canonical のまま）/
  `test-rt` / `check-headless-deps`: all Passed。
- `cargo test -p synvelis-player --test pc_c4_binding`: 7/7
  （新規 `pc_c4_02_stage1_authority_and_plugin_identity`・
  `pc_c4_02_dispatch_plugin_pin_and_last_good` 含む）。
- `mise run fmt-check`: `crates/synvelis-library/tests/
  vcs03_git_provider.rs` の **main 既存の rustfmt drift** で Failed
  （本 branch 未変更・VCS 領域は scope 外 — 報告のみ）。
- `mise run test-msrv`: 結果は PR 本文に記録。
- `mise run check-ruviform-rev`: worktree の `ui/vendor/ruviform`
  stale（synvelis#230 既知、lock rev 未変更のため本 slice 無関係）。

## NotRun

- 実 VST3/OS plugin backend・実 GUI・AudioLens f64（#294）・
  Ruviform cross-consumer CI — 別 gate。
