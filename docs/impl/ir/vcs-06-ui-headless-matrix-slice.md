---
id: "IMPL-034VpEJmewmUasabGgBzbM"
type: "impl"
status: "completed"
title: "VCS-06 UI headless matrix (synvelis#97 slice)"
created: "2026-11-10"
updated: "2026-11-11"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/97"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-app/tests/vcs06_ui_matrix.rs"
      - type: "file"
        path: "docs/specs/vcs-06-ui-headless-matrix.md"
      - type: "file"
        path: "docs/designs/vcs-06-ui-headless-matrix-design.md"
      - type: "file"
        path: "docs/plans/vcs-06-ui-headless-matrix-plan.md"
      - type: "file"
        path: "docs/tasks/vcs-06-ui-headless-matrix.md"
      - type: "file"
        path: "docs/test-specs/vcs-06-ui-headless-matrix.md"
      - type: "file"
        path: "docs/impl/ir/vcs-06-ui-headless-matrix-slice.md"
    modified:
      - type: "file"
        path: "evidence/reports/vcs-06-fixture-matrix-ledger.md"
      - type: "file"
        path: "docs/specs/README.md"
      - type: "file"
        path: "docs/designs/README.md"
      - type: "file"
        path: "docs/plans/README.md"
      - type: "file"
        path: "docs/tasks/README.md"
      - type: "file"
        path: "docs/test-specs/README.md"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/tasks/vcs-06-ui-headless-matrix.md"
    - "docs/specs/vcs-06-ui-headless-matrix.md"
    - "docs/designs/vcs-06-ui-headless-matrix-design.md"
    - "docs/plans/vcs-06-ui-headless-matrix-plan.md"
  implemented-by: []
  depends-on:
    - "docs/designs/vcs-06-ui-headless-matrix-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/ir/vcs-05-safe-apply-selected-change-restore-as-copy-headless.md"
    - "docs/specs/vcs-06-fixture-fault-matrix.md"
    - "docs/specs/vcs-04-history-visual-diff-ui.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/plans/vcs-06-ui-headless-matrix-plan.md"
  derived-by: []
  verifies:
    - "docs/test-specs/vcs-06-ui-headless-matrix.md"
  verified-by: []
  references: []
  defers:
    - "実 GPUI two-window/IME/keyboard/a11y 証拠（synvelis#114）"
    - "実 OS window / native dock（OS matrix lane）"
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# VCS-06 UI headless matrix (synvelis#97 slice)

## Summary

`crates/synvelis-app/tests/vcs06_ui_matrix.rs` を新規追加し、
`syn.vcs.view`/`syn.vcs.visual`/`vcs_history_view`/
`history_screen_spec` の外部観測契約で VCS-UI 欠損行 3 系統を固定
した: 異 `project_id` の committed 端点が `overall:"unknown"` +
issues で `equal`/0-change に潰れないこと、asset bytes `Absent`
の degraded snapshot が `completeness:"Partial"` +
`missing-asset`/`unresolved-source` issue 行を screen-spec
`vcs.issues` table へそのまま写すこと、staged pin の
`index_generation` probe 失敗 / backend 不在が
`freshness:"unknown"` + `freshness_reason` になること。
`src/` 変更なし。ledger の VCS-UI-01..05 行を headless `Pass*` と
実機 `NotRun` に分割記録した。

## Context

Implements `docs/tasks/vcs-06-ui-headless-matrix.md` per
`docs/specs/vcs-06-ui-headless-matrix.md` and
`docs/designs/vcs-06-ui-headless-matrix-design.md`, verified by
`docs/test-specs/vcs-06-ui-headless-matrix.md`（synvelis#97 の
headless 観測 slice。実機 GUI lane は #114、Issue #97 は Open の
まま）。

## Implementation

### Changes

- `tests/vcs06_ui_matrix.rs` — `vcs04_history.rs` と同型の
  `FakeProvider`（`VcsReadProvider` + `VcsBytesPort`）を
  `VcsHistoryAdapter` 経由で `set_vcs_backend` に注入し、実
  `SynvelisApp` + `adopt_view(VcsHistory)` 上で pin/view/screen-spec
  を駆動する 4 tests。
  - `cross_project_committed_endpoints_report_unknown_not_equal`
  - `absent_asset_bytes_mark_degraded_partial_with_issue_rows`
  - `staged_pin_freshness_is_unknown_when_index_probe_fails`
  - `staged_pin_freshness_is_unknown_without_backend`
- `evidence/reports/vcs-06-fixture-matrix-ledger.md` — VCS-UI-01..05
  を個別行に分割（UI-01/02/03/04 → `Pass*`、UI-05 → `NotRun`）、
  lifecycle/read-only 行を追加、fixture 一覧に本 file を追記。
- docs チェーン 6 file + `docs/*/README.md` index 再生成。

### Decisions During Implementation

- `env_f`（cross-project fixture）は `env_c` の `SnapshotEnvelope`
  を deserialize → envelope/project 双方の `project_id` を
  `0x00C0_FFEE` に差し替えて serialize して生成 — wire 上の
  cross-project read が返す姿をそのまま再現。
- `env_g`（degraded fixture）は live project に
  `ProjectOp::RegisterAsset` 適用後の capture。committed 側の
  blob read は常に `Absent`（`Missing`）、live 側は
  `op_vcs_view` が `blobs: None` で構築する既存契約により常に
  `Unknown` — 同一 project 内容でも equality は fail closed。
- staged index は asset 未登録の env_c を指す（asset 入りだと
  staged resolve が `PartialStage` で失敗し pin 自体が立たない）。
- `index_probe_fail` は `Arc<AtomicBool>` を test と provider で
  共有し、pin 解決**後**に立てて freshness probe のみを失敗させる
  （adapter が provider を所有するため test からは flag 経由でのみ
  到達可能）。
- `op_vcs_view` は常に attach 済み backend を持つため、
  no-backend の `EndpointFreshness::Unknown` 経路は同じ
  `vcs_history_view` 投影境界を `VcsSession{backend: None}` で
  直接観測した。
- 既存 `vcs04_history.rs`（app）と `synvelis-workspace/tests/
  vcs04_history.rs`（workspace）が担保する A/B pin・merge
  parent・stale・dock・lifecycle・read-only 不変条件は重複実装
  せず ledger の参照記録に留めた。

## Related Experiments

### Adopted

<!-- none -->

### Rejected

<!-- none -->

## Validation

`origin/main` @ `099e7ea` 取り込み・`mise run sync-ui-deps` 後の
最終状態で実測（Windows、pinned rustc 1.99.0、MSRV lane は
rust 1.89.0）。

- `mise run fmt-check` — exit 0。
- `mise run check` — exit 0。
- `mise run lint` — exit 0。
- `mise run test` — exit 0（workspace 全体、`vcs06_ui_matrix.rs`
  4 tests を含む）。
- `mise run test-rt` — exit 0。
- `mise run test-msrv` — exit 0（rust 1.89.0 toolchain）。
- `mise run check-app-native` — exit 0。
- `mise run lint-app-native` — exit 0。
- `mise run sync-ui-deps` — exit 0（ruviform pin `b02942c2` へ
  vendor 同期）。
- 補助証拠（canonical = mise）: `cargo test -p synvelis-app
  --test vcs06_ui_matrix` 4 pass、`cargo clippy -p
  synvelis-app --all-targets -- -D warnings` clean、
  `cargo fmt --all -- --check` clean — Windows/rustc 1.99.0。

## Risks

- 実 GPUI two-window / IME / keyboard / a11y・実 OS window は
  未検証（`NotRun`、#114 / OS matrix lane）。
- VCS Writer / 3-way merge・非 git provider・`*-REAL` 行・
  COMPAT-02..04 は引き続き `NotRun`/`Unsupported`/`Blocked`。

## Follow-ups

- synvelis#114: 実機 GUI lane で VCS-UI-05 を計測。
- synvelis#97: 残る `*-REAL` 行・COMPAT 行の統合受入（Issue は
  Open のまま、Close は Coordinator 判断）。
