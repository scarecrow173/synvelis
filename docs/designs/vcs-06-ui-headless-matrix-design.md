---
id: "DESIGN-034VpC0m5WnwXqJkI2nNXw"
type: "design"
status: "approved"
title: "VCS-06 UI headless matrix — fixture 設計"
created: "2026-11-10"
updated: "2026-11-10"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/97"
    - "docs/specs/vcs-06-ui-headless-matrix.md"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/vcs-06-ui-headless-matrix.md"
  implemented-by: []
  depends-on:
    - "docs/specs/vcs-06-ui-headless-matrix.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/vcs-06-ui-headless-matrix.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/vcs-06-ui-headless-matrix.md"
  references: []
  defers: []
  deferred-by: []
---

# VCS-06 UI headless matrix — fixture 設計

## Context

`vcs04_history.rs`（app 層）と `synvelis-workspace/tests/vcs04_history.rs`
（workspace 層）は既に `SynvelisApp` + `VcsHistoryAdapter` +
in-memory `FakeProvider` で `syn.vcs.view`/`syn.vcs.visual`/
`vcs_history_view`/`history_screen_spec` の headless lane を成立
させている。本 slice はその lane 上で、既存 fixture が踏んでいない
欠損行 — 異 `project_id`、asset bytes `Absent` の degraded
snapshot、staged freshness の Unknown 経路 — を固定する。
`src/` は変更せず、公開 API の外部観測契約のみを oracle とする。

## Scope

### In Scope

- `crates/synvelis-app/tests/vcs06_ui_matrix.rs` 1 file・4 tests
- ledger `evidence/reports/vcs-06-fixture-matrix-ledger.md` の
  VCS-UI-01..05 行分割

### Out of Scope

- `crates/*/src/` 変更、既存 vcs04 fixture の重複再実装、実
  GPUI/OS/IME/keyboard/a11y（#114）

## Components and Boundaries

- `FakeProvider`（`VcsReadProvider` + `VcsBytesPort`）:
  `vcs04_history.rs` と同型の in-memory provider。差分は
  `index_probe_fail: Arc<AtomicBool>` の追加と、`resolve_fixed_revision`
  が `"cross-project"`→`'f'`、`"degraded"`→`'g'` の expr を解決する
  点のみ。
- `fixture()`: live project を "vcs-c" + `RegisterAsset` まで進め、
  committed `'a'..'g'`・staged（asset 未登録の env_c）を
  `put_committed`/`put_staged` で投入して `VcsHistoryAdapter` 経由で
  `set_vcs_backend`、`adopt_view(VcsHistory)` まで行う。
- 観測点: `op_vcs_visual`（pin）→ `op_vcs_view`（JSON 投影）と、
  `vcs_history_view` + `history_screen_spec`（screen-spec 投影）。

## Data and Control Flow

1. `fixture()` が envelope 群を生成:
   - `env_a`/`env_b`/`env_c`: 名前変更のみの live capture 連鎖。
   - `env_g`: `RegisterAsset` 適用後の capture（asset 保持）。
   - `env_f`: `env_c` を deserialize → envelope/project 双方の
     `project_id` を `0x00C0_FFEE` に差し替えて serialize
     （cross-project read が返す姿）。
2. 各 test が `pin(before/after, endpoint)` で endpoint を解決・
   pin し、`history()` の JSON を assert。
3. degraded 行のみ `vcs_history_view` を `VcsSession{backend:
   app.vcs_backend(), …}` で再投影し `history_screen_spec` の
   `vcs.issues` table 行数を `summary.unresolved` と照合。
4. no-backend 行のみ `VcsSession{backend: None, …}` で同じ
   `vcs_history_view` を投影（`op_vcs_view` は常に attach 済み
   backend を持つため、Unknown 経路はこの境界で観測する）。

## Non-Functional Constraints

- deterministic: 全 fixture は in-memory・temp 不使用・時刻非依存。
- read-only: pin/view/screen-spec は project revision・engine・
  transport・backend journal を変更しない（下層は vcs04 の
  `vcs_ops_never_touch_project_engine_or_transport` が保証）。

## Risks and Trade-offs

| risk | 対応 |
| --- | --- |
| staged fixture が asset 入り snapshot を指すと staged resolve が `PartialStage` で失敗 | staged は asset 未登録の env_c を指す |
| live 側 asset probe が `Present` になり `UnresolvedSource` が出ない | `op_vcs_view` が `blobs: None` で `VcsSession` を構築する既存契約を利用（live 側は常に `Unknown`） |
| probe 失敗を pin 時に起こすと resolve 自体が失敗 | `Arc<AtomicBool>` を pin 解決後に立て、freshness probe のみ失敗させる |
| 既存 vcs04 との重複 | A/B・merge・stale・dock・lifecycle は ledger 参照記録のみ |

## Deferred Design Concerns

- 実 GPUI two-window / IME / keyboard / a11y — #114 実機 lane。

## Verification Notes

- [x] `cargo test -p synvelis-app --test vcs06_ui_matrix`（補助）で
  4 tests pass、fmt/clippy clean（canonical `mise` 結果は impl-ir）。
