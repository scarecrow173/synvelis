---
id: "IMPL-034XHDQ6rzGXUkov3cBJgE"
type: "impl"
status: "completed"
title: "PC-C4 webview composition — synvelis.rs engine attach 実装記録"
created: "2026-11-15"
updated: "2026-11-15"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/101"
  changes:
    added:
      - type: "file"
        path: "docs/specs/pc-c4-webview-composition.md"
      - type: "file"
        path: "docs/designs/pc-c4-webview-composition.md"
      - type: "file"
        path: "docs/plans/pc-c4-webview-composition.md"
      - type: "file"
        path: "docs/tasks/pc-c4-webview-composition.md"
      - type: "file"
        path: "docs/test-specs/pc-c4-webview-composition.md"
      - type: "file"
        path: "docs/impl/ir/pc-c4-webview-composition-synvelis-rs-engine-attach.md"
      - type: "file"
        path: "docs/impl/exp/pc-c4-webview-composition.md"
      - type: "file"
        path: "crates/synvelis-app/tests/pc_c4_webview_composition.rs"
      - type: "file"
        path: "evidence/sv101/pc-c4-webview-composition-verification.md"
    modified:
      - type: "file"
        path: "crates/synvelis-app/src/composition.rs"
      - type: "file"
        path: "crates/synvelis-app/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-app/src/bin/synvelis.rs"
      - type: "file"
        path: "docs/tasks/pc-c4-02-task1-plugin-instance-binding.md"
      - type: "file"
        path: "docs/tasks/pc-c4-plugin-binding-task.md"
      - type: "file"
        path: "docs/impl/ir/pc-c4-observe-engine-wiring.md"
  implements:
    - "docs/tasks/pc-c4-webview-composition.md"
  implemented-by: []
  depends-on:
    - "docs/tasks/pc-c4-webview-composition.md"
    - "docs/specs/pc-c4-webview-composition.md"
    - "docs/designs/pc-c4-webview-composition.md"
    - "docs/plans/pc-c4-webview-composition.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/ir/pc-c4-drain-composition.md"
    - "docs/impl/ir/pc-c4-observe-engine-wiring.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies:
    - "docs/test-specs/pc-c4-webview-composition.md"
  verified-by:
    - "docs/test-specs/pc-c4-webview-composition.md"
  references:
    - "docs/impl/exp/pc-c4-webview-composition.md"
    - "evidence/sv101/pc-c4-webview-composition-verification.md"
  defers:
    - "実 plugin factory（ruviform#253 gate）・実 GUI（synvelis#114）・wire-schema L1 結合検証（synvelis#101 残件）"
    - "synvelis-native bin の共有 helper への呼出側移行（後続 slice）"
    - "project.new/open 後の engine 再 attach（synvelis#106 の責務）"
  deferred-by: []
metadata:
  experiments:
    adopted:
      - "docs/impl/exp/pc-c4-webview-composition.md"
    rejected: []
---

# PC-C4 webview composition — synvelis.rs engine attach 実装記録

## Summary

`crates/synvelis-app/src/bin/synvelis.rs`（webview product
bin）に routing engine composition を配線した。共有 helper
`compose_product_routing(app, project_dir: Option<PathBuf>)`
を `composition.rs` に新設し、`synvelis-native.rs` の
`compose_product_routing`/`product_engine_config` と同じ
条件（`SYNVELIS_PROJECT_DIR` gate・empty plugin registry・
`<dir>/media` media root・48 kHz/stereo/256 Realtime
geometry）で `compose_routing_engine` を呼ぶ。webview bin は
`std::env::var_os("SYNVELIS_PROJECT_DIR").map(PathBuf::from)`
を process boundary で読み、`attach_logging` 直後・
`NativeShell::new_with_data` 前に呼出を行う。

新規 fixture `tests/pc_c4_webview_composition.rs` は
`compose_product_routing` を直接呼び、gate 閉（`None` →
stub 維持）・gate 開（`Some(dir)` → composition 時点で
attach → publish → 非 stub `syn.control.status`）・
`project.new` recreate detach の 3 test を検証する。
headless in-process fixture であり実 WebView backend・実
plugin factory は不要。

## Context

- Issue: synvelis#101（長寿 tracker、`Refs #101`）
- task: `docs/tasks/pc-c4-webview-composition.md`
- spec/design/plan/test-spec: `docs/{specs,designs,plans,
  test-specs}/pc-c4-webview-composition.md`
- 前提 slice: `docs/impl/ir/pc-c4-drain-composition.md`
  （`compose_routing_engine` composition root）と
  `docs/impl/ir/pc-c4-observe-engine-wiring.md`
  （`ui_pump` observe drain 配線）
- mirror precedent: `crates/synvelis-app/src/bin/
  synvelis-native.rs` の private `compose_product_routing`/
  `product_engine_config`（本 slice では変更しない — webview
  側のみ新 helper を利用）
- stale status 簿記: `docs/tasks/pc-c4-02-task1-plugin-
  instance-binding.md`（PR #250 merge 済み）、
  `docs/tasks/pc-c4-plugin-binding-task.md`（PR #393 merge
  済み）、`docs/impl/ir/pc-c4-observe-engine-wiring.md`
  （PR #452 merge 済み）

## Implementation Notes

- `composition.rs`: `pub fn compose_product_routing(app:
  &mut SynvelisApp, project_dir: Option<PathBuf>)` を追加。
  `None` は即 return（gate 閉 — engine stub 維持の意図
  された degrade）。`Some(dir)` は
  `compose_routing_engine(app, product_engine_config(dir
  .join("media")), Vec::new())`。private
  `product_engine_config(media_root)` は native の同名
  private fn と同一 geometry を返す。
- `lib.rs`: `compose_product_routing` を `composition` から
  re-export に追加（`compose_routing_engine` と同じ経路）。
- `bin/synvelis.rs`: env var 名知識は process boundary に
  留め、library は env を読まない（既存契約）。呼出位置は
  `attach_logging` 直後・`NativeShell::new_with_data` 前 —
  shell 構築前に session/engine が composition 済みになる。
- fixture は `PluginHost`/`PluginHostProcessors` を名指さず
  `compose_product_routing` のみを呼ぶ — production
  wiring そのものを検証する。
- `synvelis-native.rs`・`service.rs`・他 crate・`Cargo.toml`・
  `mise.toml` は無変更。

## Evidence

- `evidence/sv101/pc-c4-webview-composition-verification.md`
  — canonical 検証 lane のコマンド・OS・toolchain・exit code・
  Passed/NotRun 記録。
- `evidence/sv101/external-gates.md` — 外部 gate
  （ruviform#253 実 plugin factory・#114 実 GUI・wire-schema
  L1・3OS）の NotRun/Deferred 追記。
- doc-driven-dev-graph route は pre-existing の broken
  `defers` relation で `blocked` を返す（drain slice と同じ
  既知状態）— canonical doc chain は scaffold script + 手動
  記入で構築し、index は doc-maintenance `index apply` で
  再生成。
- 実験: `docs/impl/exp/pc-c4-webview-composition.md`
  （gate 形状・native 移行範囲・fixture 粒度の代替案検討を
  EXP-1〜3 に記録）。

## External Gates / Deferred

- 実 plugin factory registry（VST3/AU/CLAP・OS backend）—
  ruviform#253 gate。本 slice は empty registry のままで、
  plugin-bound processor は typed reject で fail する
  （捏造しない）。
- 実 GUI/実 Window での webview boot — synvelis#114 系の
  GUI-L2 gate。
- wire-schema L1 結合検証・AudioLens/Ruviform consumer
  照合 — synvelis#101 残件。
- `synvelis-native.rs` の共有 helper 移行 — 後続 slice。
- project.new/open 後の engine 再 attach — synvelis#106。
