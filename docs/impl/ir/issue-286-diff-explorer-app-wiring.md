---
id: "IMPL-034ZeMql6ocIS9vFcTCzaV"
type: "impl"
status: "completed"
title: "issue-286-diff-explorer-app-wiring"
created: "2026-11-17"
updated: "2026-11-18"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/286"
    - "docs/tasks/issue-286-diff-explorer-app-wiring.md"
    - "docs/test-specs/issue-286-diff-explorer-app-wiring.md"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-app/tests/cb09_variant_diff.rs"
      - type: "file"
        path: "docs/discovery/issue-286-diff-explorer-app-wiring.md"
      - type: "file"
        path: "docs/specs/issue-286-diff-explorer-app-wiring.md"
      - type: "file"
        path: "docs/designs/issue-286-diff-explorer-app-wiring.md"
      - type: "file"
        path: "docs/plans/issue-286-diff-explorer-app-wiring.md"
      - type: "file"
        path: "docs/adr/issue-286-diff-explorer-app-wiring.md"
      - type: "file"
        path: "docs/tasks/issue-286-diff-explorer-app-wiring.md"
      - type: "file"
        path: "docs/test-specs/issue-286-diff-explorer-app-wiring.md"
      - type: "file"
        path: "docs/impl/ir/issue-286-diff-explorer-app-wiring.md"
      - type: "file"
        path: "evidence/sv286/app-wiring-verification.md"
    modified:
      - type: "file"
        path: "crates/synvelis-workspace/src/variant_diff.rs"
      - type: "file"
        path: "crates/synvelis-workspace/tests/cb09_diff_explorer.rs"
      - type: "file"
        path: "crates/synvelis-app/src/ui_ops.rs"
      - type: "file"
        path: "crates/synvelis-app/src/app.rs"
      - type: "file"
        path: "crates/synvelis-app/src/service.rs"
      - type: "file"
        path: "crates/synvelis-app/src/screens.rs"
      - type: "file"
        path: "crates/synvelis-app/src/lib.rs"
      - type: "file"
        path: "docs/impl/ir/cb-09-diff-explorer.md"
      - type: "file"
        path: "docs/test-specs/cb-09-diff-explorer.md"
      - type: "file"
        path: "crates/synvelis-render/src/render_job/orchestrator.rs"
      - type: "file"
        path: "crates/synvelis-workspace/src/render_dispatch.rs"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/tasks/issue-286-diff-explorer-app-wiring.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/ir/cb-09-diff-explorer.md"
    - "docs/impl/ir/cb-08-compare-orchestrator.md"
  refines:
    - "docs/impl/ir/cb-09-diff-explorer.md"
  refined-by: []
  derives-from:
    - "docs/tasks/issue-286-diff-explorer-app-wiring.md"
  derived-by: []
  verifies:
    - "docs/test-specs/issue-286-diff-explorer-app-wiring.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted:
      - "docs/impl/exp/issue-286-diff-explorer-app-wiring.md"
    rejected: []
---

# issue-286-diff-explorer-app-wiring

## Summary

CB-09 Difference Explorer の headless view-model
（`cb-09-diff-explorer`）を `synvelis-app` へ配線し、
`VariantDiffEngine` facade 経由で CB-08 orchestrator
（compare）と gateway CAS（adopt/adopt-preview）へ typed
intent dispatch する経路を完成させた。detached
`ViewType::VariantDiff` view の open/close・target seed・
`ViewSpec` projection/mount・`syn.variant-diff.*` op・
per-view session・runtime publish observation を実装し、
semantic/signal/perceptual evidence の分離と honest
state 語彙を app 層まで維持した。

## Context

`docs/tasks/issue-286-diff-explorer-app-wiring.md` /
`docs/test-specs/issue-286-diff-explorer-app-wiring.md`
に基づく synvelis#286 の「後始末」slice。先行 slice
`cb-09-diff-explorer`（headless view-model + ViewSpec）と
`cb-08-compare-orchestrator`（#285、provider seam）・
`cb-07-arrangement-adapter`（#284）の成果を接続する。
app は gateway 型を名指しできない（AC-05）ため、workspace
側 facade が `VariantPorts`/`simulate`/`execute` を opaque
に所有する設計とした。

## Implementation

### Changes

- `crates/synvelis-workspace/src/variant_diff.rs`:
  `VariantDiffEngine` facade を追加。`VariantPorts`
  （orchestrator + adopt gateway + asset blob probe）を保持
  し、`compare`/`adopt_preview`/`adopt` を
  `VariantDiffIntent` 検証付きで dispatch。`VariantDiffDispatchError`
  で typed reject/engine/commit 失敗を保持。standard ports
  が CB-07 `ArrangementAdapter` を含む全 merged adapter を
  登録する（synvelis#539 merge 後は facade 側の手動 install
  を除去 — stale `DomainAdapterUnmerged` verdict は
  supported へ修正済み）。
  signal provider 出力を provenance 保持で
  `AuxLayerEvidence` へ map。`adopt` は `Adopt` intent のみ
  受理（preview/compare intent は malformed reject）。
  `apply_variant_diff_view_local` を追加し、view-local
  action を authority 非依存で適用可能にした。
- `crates/synvelis-workspace/src/lib.rs`: 当初 facade 型と
  view-local helper の root re-export を追加したが、全 import が
  `synvelis_workspace::variant_diff::{...}` module path 形式で
  compile 上不要と判明したため revert（sv#518 占有範囲・ask
  必須の違反解消）。root export 差分は残さない。
- `crates/synvelis-workspace/tests/cb09_diff_explorer.rs`:
  stale arrangement verdict の test 更新 + engine facade
  test 7 件追加（計 40 test）。
- `crates/synvelis-app/src/ui_ops.rs`: `VariantDiffUi`
  （engine + per-view `VariantDiffSession` + bundle root +
  `RuntimePublishState` 観測 map）を `UiOpsState` に追加。
  `syn.variant-diff.visual`（open/close/target seed）・
  `syn.variant-diff.view`（persisted state + deterministic
  `ViewSpec`）・`syn.variant-diff.action`（decode →
  view-local early path → authority input 解決 → apply →
  intent dispatch）を実装。projection は live
  authority/context を毎回再導出し、persisted target の
  pin が一致する場合のみ session evidence を付与。
  `variant_diff_run_intent`/`_compare`/`_adopt` に dispatch
  を分離。adopt は `params.approval` 必須・CAS commit・
  revision 前進時のみ `notify_committed_change`・receipt
  `/runtime/state` decode 成功時のみ publish 観測を記録。
- `crates/synvelis-app/src/app.rs`: `bundle_path` を
  `pub(crate)` 化（engine root の live 追跡用）。
- `crates/synvelis-app/src/service.rs`: 3 op の registry
  登録と dispatch arm。
- `crates/synvelis-app/src/screens.rs` + `lib.rs`:
  `mount_variant_diff_view` と re-export。
- `crates/synvelis-app/tests/cb09_variant_diff.rs`: app
  結合 test 11 件（lifecycle/kind gate・target seed・
  view-local authority 非接触・compare pin-scope・
  stale/implicit reject・preview 非 commit・approval 必須・
  CAS commit・stale CAS reject・ViewSpec mount smoke）。

### Decisions During Implementation

- **View-local early path**: persisted target が無い fresh
  view でも select/audition/layer visibility/set-target が
  動く必要があるため、`apply_variant_diff_view_local`
  （workspace 側の view-local subset）を authority 解決の
  前に適用する二段構成にした。authority action は
  persisted target、無ければ action 自身の explicit pin
  で context を解決し、apply 時に再検証する。
- **Engine root の live 追跡**: `VariantDiffEngine` は
  bundle root に紐づく `VariantPorts` を持つため、
  `bundle_path` 変更時に engine・全 session・publish
  observation を破棄して再構築する（evidence が project
  を跨がない）。
- **Adopt は target を mint しない**: adopt は mutation で
  あり compare target を捏造しない。target 未設定 view は
  adopt 成功後も `UNAVAILABLE` を返すのが正直な挙動で
  あり、test で固定した。
- **Preview/commit intent の排他**: `VariantDiffEngine::
  adopt` は `Adopt` intent のみ受理し、`AdoptPreview`/
  `Compare` intent を malformed reject にすることで、
  preview が commit 経路へ混入する経路を型で閉じた。
- **Runtime publish は receipt decode 成功時のみ**:
  adopt receipt JSON の `/runtime/state` が
  `RuntimePublishState` に decode できた時だけ branch 別
  観測へ記録し、欠落・失敗時は観測を触らない。
- **Exact audition は Unavailable のまま**: plugin/render
  probe が bind されていないため capability evidence の
  exact audition は `Unavailable` を維持し、editability
  のみ CAS adopt 経路の存在で available とした（捏造
  しない）。

## Related Experiments

### Adopted

なし — 既知解の実行タスクであり、exploratory な分岐を
要しなかったため Experiment Log は作成していない
（impl-doc skill の granularity rule に従う）。

### Rejected

なし。

## Validation

`evidence/sv286/app-wiring-verification.md` に全記録。
概略:

- `mise run check-app-native` / `lint-app-native` /
  `test-app-gpui` — pass（final head `e40b15f`。GPUI
  driver 組立・clippy・GPUI-simulated test）
- `mise run fmt-check` — 本 worktree で cargo-fmt の
  `os error 206`（環境起因・diff 無関係）により fail。
  全 15 member の `cargo fmt -p -- --check` は個別 pass
  （詳細は evidence 参照）
- `mise run check synvelis-workspace` / `check synvelis-app`
  — pass
- `mise run lint synvelis-workspace` / `lint synvelis-app`
  / `lint-app-gpui` — pass（`-D warnings`）
- `mise run test synvelis-workspace` — pass
  （`cb09_diff_explorer` 40/40 を含む全 suite）
- `mise run test synvelis-app` — pass
  （`cb09_variant_diff` 11/11 を含む全 suite）
- `mise run check-headless-deps` — pass
- `mise run check-app-gpui` — pass

## Risks

- 実 GPUI window・実レンダリング・focus/keyboard・実
  plugin/RT 受入は未実施（#288 系 gate）。mount smoke は
  ViewHost への component mount 確認までであり、実画面
  受入を主張しない。
- signal/perceptual evidence は provider seam 経由の
  provenance 付き投影であり、実 PCM/RT 測定・perceptual
  評価は #288 系 gate。
- `op_variant_diff_action` の authority 解決は persisted
  target 優先・action pin fallback の順であり、両者が
  乖離した場合の UX 意味論は CB-11 cross-gate で最終確認
  される余地がある。

## Follow-ups

- synvelis#288 系 gate での実 GPUI/plugin/RT 受入。
- CB-10 gateway typed contract slice・CB-11 cross-gate
  での end-to-end 契約確認。
- Issue #286 の close 判定は coordinator に委譲
  （`Refs #286` のみ使用）。
